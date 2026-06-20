// SPDX-License-Identifier: MIT OR Apache-2.0
// This is based on https://github.com/smol-rs/async-io
// with adaptations by:
//
// Copyright (C) 2021-2022 François Laignel <fengalin@free.fr>

use concurrent_queue::ConcurrentQueue;
use futures::ready;
use polling::{Event, Events, Poller};
use slab::Slab;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::panic;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

// Choose the proper implementation of `Registration` based on the target platform.
cfg_if::cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use windows::Registration;
    } else if #[cfg(any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
    ))] {
        mod kqueue;
        pub use kqueue::Registration;
    } else if #[cfg(unix)] {
        mod unix;
        pub use unix::Registration;
    } else {
        compile_error!("unsupported platform");
    }
}

use crate::runtime::{Async, RUNTIME_CAT};

const READ: usize = 0;
const WRITE: usize = 1;

thread_local! {
    static CURRENT_REACTOR: RefCell<Option<Reactor>> = const { RefCell::new(None) };
}

#[derive(Debug)]
pub(super) struct Reactor {
    /// Portable bindings to epoll/kqueue/event ports/wepoll.
    ///
    /// This is where I/O is polled, producing I/O events.
    poller: Poller,

    /// Ticker bumped before polling.
    ///
    /// This is useful for checking what is the current "round" of `ReactorLock::react()` when
    /// synchronizing things in `Source::readable()` and `Source::writable()`. Both of those
    /// methods must make sure they don't receive stale I/O events - they only accept events from a
    /// fresh "round" of `ReactorLock::react()`.
    ticker: AtomicUsize,

    /// Time when timers have been checked in current time slice.
    timers_check_instant: Instant,

    /// Time limit when timers are being fired in current time slice.
    time_slice_end: Instant,

    /// Half max throttling duration, needed to fire timers.
    half_max_throttling: Duration,

    /// List of wakers to wake when reacting.
    wakers: Vec<Waker>,

    /// Registered sources.
    sources: Slab<Arc<Source>>,

    /// Temporary storage for I/O events when polling the reactor.
    ///
    /// Holding a lock on this event list implies the exclusive right to poll I/O.
    events: Events,

    /// An ordered map of registered regular timers.
    ///
    /// Timers are in the order in which they fire. The `RegularTimerId` distinguishes
    /// timers that fire at the same time. The `Waker` represents the task awaiting the
    /// timer.
    timers: BTreeMap<(Instant, RegularTimerId), Waker>,

    /// An ordered map of registered after timers.
    ///
    /// These timers are guaranteed to fire no sooner than their expected time.
    ///
    /// Timers are in the order in which they fire. The `AfterTimerId` distinguishes
    /// timers that fire at the same time. The `Waker` represents the task awaiting the
    /// timer.
    after_timers: BTreeMap<(Instant, AfterTimerId), Waker>,

    /// A queue of timer operations (insert and remove).
    ///
    /// When inserting or removing a timer, we don't process it immediately - we just push it into
    /// this queue. Timers actually get processed when the queue fills up or the reactor is polled.
    timer_ops: ConcurrentQueue<TimerOp>,
}

impl Reactor {
    fn new(max_throttling: Duration) -> Self {
        Reactor {
            poller: Poller::new().expect("cannot initialize I/O event notification"),
            ticker: AtomicUsize::new(0),
            timers_check_instant: Instant::now(),
            time_slice_end: Instant::now(),
            half_max_throttling: max_throttling / 2,
            wakers: Vec::new(),
            sources: Slab::new(),
            events: Events::new(),
            timers: BTreeMap::new(),
            after_timers: BTreeMap::new(),
            timer_ops: ConcurrentQueue::bounded(1000),
        }
    }

    /// Initializes the reactor for current thread.
    pub fn init(max_throttling: Duration) {
        CURRENT_REACTOR.with(|cur| {
            let mut cur = cur.borrow_mut();
            if cur.is_none() {
                *cur = Some(Reactor::new(max_throttling));
            }
        })
    }

    /// Clears the `Reactor`.
    ///
    /// It will be ready for reuse on current thread without reallocating.
    pub fn clear() {
        let _ = CURRENT_REACTOR.try_with(|cur_reactor| {
            cur_reactor.borrow_mut().as_mut().map(|reactor| {
                reactor.ticker = AtomicUsize::new(0);
                reactor.wakers.clear();
                reactor.sources.clear();
                reactor.events.clear();
                reactor.timers.clear();
                reactor.after_timers.clear();
                while !reactor.timer_ops.is_empty() {
                    let _ = reactor.timer_ops.pop();
                }
            })
        });
    }

    /// Executes the function with current thread's reactor as ref.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - The Reactor is not initialized, i.e. if
    ///   current thread is not a [`Context`] thread.
    /// - The Reactor is already mutably borrowed.
    ///
    /// Use [`Context::enter`] to register i/o sources
    /// or timers from a different thread.
    #[track_caller]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Reactor) -> R,
    {
        CURRENT_REACTOR.with(|reactor| {
            f(reactor
                .borrow()
                .as_ref()
                .expect("Reactor initialized at this point"))
        })
    }

    /// Executes the function with current thread's reactor as mutable.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - The Reactor is not initialized, i.e. if
    ///   current thread is not a [`Context`] thread.
    /// - The Reactor is already mutably borrowed.
    ///
    /// Use [`Context::enter`] to register i/o sources
    /// or timers from a different thread.
    #[track_caller]
    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Reactor) -> R,
    {
        CURRENT_REACTOR.with(|reactor| {
            f(reactor
                .borrow_mut()
                .as_mut()
                .expect("Reactor initialized at this point"))
        })
    }

    /// Returns the current ticker.
    pub fn ticker(&self) -> usize {
        self.ticker.load(Ordering::SeqCst)
    }

    pub fn half_max_throttling(&self) -> Duration {
        self.half_max_throttling
    }

    pub fn timers_check_instant(&self) -> Instant {
        self.timers_check_instant
    }

    pub fn time_slice_end(&self) -> Instant {
        self.time_slice_end
    }

    /// Registers an I/O source in the reactor.
    pub fn insert_io(&mut self, raw: Registration) -> io::Result<Arc<Source>> {
        // Create an I/O source for this file descriptor.
        let source = {
            let key = self.sources.vacant_entry().key();
            let source = Arc::new(Source {
                registration: raw,
                key,
                state: Default::default(),
            });
            self.sources.insert(source.clone());
            source
        };

        // Register the file descriptor.
        if let Err(err) = source.registration.add(&self.poller, source.key) {
            gst::error!(
                crate::runtime::RUNTIME_CAT,
                "Failed to register fd {:?}: {}",
                source.registration,
                err,
            );
            self.sources.remove(source.key);
            return Err(err);
        }

        Ok(source)
    }

    /// Deregisters an I/O source from the reactor.
    pub fn remove_io(&mut self, source: &Source) -> io::Result<()> {
        self.sources.remove(source.key);
        source.registration.delete(&self.poller)
    }

    /// Registers a regular timer in the reactor.
    ///
    /// Returns the inserted timer's ID.
    pub fn insert_regular_timer(&mut self, when: Instant, waker: &Waker) -> RegularTimerId {
        // Generate a new timer ID.
        static REGULAR_ID_GENERATOR: AtomicUsize = AtomicUsize::new(1);
        let id = RegularTimerId(REGULAR_ID_GENERATOR.fetch_add(1, Ordering::Relaxed));

        // Push an insert operation.
        while self
            .timer_ops
            .push(TimerOp::Insert(when, id.into(), waker.clone()))
            .is_err()
        {
            // If the queue is full, drain it and try again.
            gst::warning!(RUNTIME_CAT, "react: timer_ops is full");
            self.process_timer_ops();
        }

        id
    }

    /// Registers an after timer in the reactor.
    ///
    /// Returns the inserted timer's ID.
    pub fn insert_after_timer(&mut self, when: Instant, waker: &Waker) -> AfterTimerId {
        // Generate a new timer ID.
        static AFTER_ID_GENERATOR: AtomicUsize = AtomicUsize::new(1);
        let id = AfterTimerId(AFTER_ID_GENERATOR.fetch_add(1, Ordering::Relaxed));

        // Push an insert operation.
        while self
            .timer_ops
            .push(TimerOp::Insert(when, id.into(), waker.clone()))
            .is_err()
        {
            // If the queue is full, drain it and try again.
            gst::warning!(RUNTIME_CAT, "react: timer_ops is full");
            self.process_timer_ops();
        }

        id
    }

    /// Deregisters a timer from the reactor.
    pub fn remove_timer(&mut self, when: Instant, id: impl Into<TimerId>) {
        // Push a remove operation.
        let id = id.into();
        while self.timer_ops.push(TimerOp::Remove(when, id)).is_err() {
            gst::warning!(RUNTIME_CAT, "react: timer_ops is full");
            // If the queue is full, drain it and try again.
            self.process_timer_ops();
        }
    }

    /// Processes ready timers and extends the list of wakers to wake.
    fn process_timers(&mut self, now: Instant) {
        self.process_timer_ops();

        self.timers_check_instant = now;
        self.time_slice_end = now + self.half_max_throttling;

        // Split regular timers into ready and pending timers.
        //
        // Careful to split just *after* current time slice end,
        // so that a timer set to fire in current time slice is
        // considered ready.
        let pending = self
            .timers
            .split_off(&(self.time_slice_end, RegularTimerId::NONE));
        let ready = mem::replace(&mut self.timers, pending);

        // Add wakers to the list.
        if !ready.is_empty() {
            gst::trace!(
                RUNTIME_CAT,
                "process_timers (regular): {} ready wakers",
                ready.len()
            );

            for (_, waker) in ready {
                self.wakers.push(waker);
            }
        }

        // Split "at least" timers into ready and pending timers.
        //
        // Careful to split just *after* `now`,
        // so that a timer set for exactly `now` is considered ready.
        let pending = self
            .after_timers
            .split_off(&(self.timers_check_instant, AfterTimerId::NONE));
        let ready = mem::replace(&mut self.after_timers, pending);

        // Add wakers to the list.
        if !ready.is_empty() {
            gst::trace!(
                RUNTIME_CAT,
                "process_timers (after): {} ready wakers",
                ready.len()
            );

            for (_, waker) in ready {
                self.wakers.push(waker);
            }
        }
    }

    /// Processes queued timer operations.
    fn process_timer_ops(&mut self) {
        // Process only as much as fits into the queue, or else this loop could in theory run
        // forever.
        for _ in 0..self.timer_ops.capacity().unwrap() {
            match self.timer_ops.pop() {
                Ok(TimerOp::Insert(when, TimerId::Regular(id), waker)) => {
                    self.timers.insert((when, id), waker);
                }
                Ok(TimerOp::Insert(when, TimerId::After(id), waker)) => {
                    self.after_timers.insert((when, id), waker);
                }
                Ok(TimerOp::Remove(when, TimerId::Regular(id))) => {
                    self.timers.remove(&(when, id));
                }
                Ok(TimerOp::Remove(when, TimerId::After(id))) => {
                    self.after_timers.remove(&(when, id));
                }
                Err(_) => break,
            }
        }
    }

    /// Processes new events.
    pub fn react(&mut self, now: Instant) -> io::Result<()> {
        debug_assert!(self.wakers.is_empty());

        // Process ready timers.
        self.process_timers(now);

        // Bump the ticker before polling I/O.
        let tick = self.ticker.fetch_add(1, Ordering::SeqCst).wrapping_add(1);

        self.events.clear();

        // Block on I/O events.
        let res = match self.poller.wait(&mut self.events, Some(Duration::ZERO)) {
            // No I/O events occurred.
            Ok(0) => Ok(()),
            // At least one I/O event occurred.
            Ok(_) => {
                for ev in self.events.iter() {
                    // Check if there is a source in the table with this key.
                    if let Some(source) = self.sources.get(ev.key) {
                        let mut state = source.state.lock().unwrap();

                        // Collect wakers if any event was emitted.
                        for &(dir, emitted) in &[(WRITE, ev.writable), (READ, ev.readable)] {
                            if emitted {
                                state[dir].tick = tick;
                                state[dir].drain_into(&mut self.wakers);
                            }
                        }

                        // Re-register if there are still writers or readers. The can happen if
                        // e.g. we were previously interested in both readability and writability,
                        // but only one of them was emitted.
                        if !state[READ].is_empty() || !state[WRITE].is_empty() {
                            // Create the event that we are interested in.
                            let event = {
                                let mut event = Event::none(source.key);
                                event.readable = !state[READ].is_empty();
                                event.writable = !state[WRITE].is_empty();
                                event
                            };

                            // Register interest in this event.
                            source.registration.modify(&self.poller, event)?;
                        }
                    }
                }

                Ok(())
            }

            // The syscall was interrupted.
            Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(()),

            // An actual error occureed.
            Err(err) => Err(err),
        };

        // Wake up ready tasks.
        if !self.wakers.is_empty() {
            gst::trace!(RUNTIME_CAT, "react: {} ready wakers", self.wakers.len());

            for waker in self.wakers.drain(..) {
                // Don't let a panicking waker blow everything up.
                panic::catch_unwind(|| waker.wake()).ok();
            }
        }

        res
    }
}

/// Timer will fire in its time slice.
/// This can happen before of after the expected time.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RegularTimerId(usize);
impl RegularTimerId {
    const NONE: RegularTimerId = RegularTimerId(0);
}

/// Timer is guaranteed to fire after the expected time.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AfterTimerId(usize);
impl AfterTimerId {
    const NONE: AfterTimerId = AfterTimerId(0);
}

/// Any Timer Ids.
#[derive(Copy, Clone, Debug)]
pub(crate) enum TimerId {
    Regular(RegularTimerId),
    After(AfterTimerId),
}

impl From<RegularTimerId> for TimerId {
    fn from(id: RegularTimerId) -> Self {
        TimerId::Regular(id)
    }
}

impl From<AfterTimerId> for TimerId {
    fn from(id: AfterTimerId) -> Self {
        TimerId::After(id)
    }
}

/// A single timer operation.
enum TimerOp {
    Insert(Instant, TimerId, Waker),
    Remove(Instant, TimerId),
}

/// A registered source of I/O events.
#[derive(Debug)]
pub(super) struct Source {
    /// This source's registration into the reactor.
    pub(super) registration: Registration,

    /// The key of this source obtained during registration.
    key: usize,

    /// Inner state with registered wakers.
    state: Mutex<[Direction; 2]>,
}

/// A read or write direction.
#[derive(Debug, Default)]
struct Direction {
    /// Last reactor tick that delivered an event.
    tick: usize,

    /// Ticks remembered by `Async::poll_readable()` or `Async::poll_writable()`.
    ticks: Option<(usize, usize)>,

    /// Waker stored by `Async::poll_readable()` or `Async::poll_writable()`.
    waker: Option<Waker>,

    /// Wakers of tasks waiting for the next event.
    ///
    /// Registered by `Async::readable()` and `Async::writable()`.
    wakers: Slab<Option<Waker>>,
}

impl Direction {
    /// Returns `true` if there are no wakers interested in this direction.
    fn is_empty(&self) -> bool {
        self.waker.is_none() && self.wakers.iter().all(|(_, opt)| opt.is_none())
    }

    /// Moves all wakers into a `Vec`.
    fn drain_into(&mut self, dst: &mut Vec<Waker>) {
        if let Some(w) = self.waker.take() {
            dst.push(w);
        }
        for (_, opt) in self.wakers.iter_mut() {
            if let Some(w) = opt.take() {
                dst.push(w);
            }
        }
    }
}

impl Source {
    /// Polls the I/O source for readability.
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_ready(READ, cx)
    }

    /// Polls the I/O source for writability.
    pub fn poll_writable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_ready(WRITE, cx)
    }

    /// Registers a waker from `poll_readable()` or `poll_writable()`.
    ///
    /// If a different waker is already registered, it gets replaced and woken.
    fn poll_ready(&self, dir: usize, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.state.lock().unwrap();

        // Check if the reactor has delivered an event.
        if let Some((a, b)) = state[dir].ticks {
            // If `state[dir].tick` has changed to a value other than the old reactor tick,
            // that means a newer reactor tick has delivered an event.
            if state[dir].tick != a && state[dir].tick != b {
                state[dir].ticks = None;
                return Poll::Ready(Ok(()));
            }
        }

        let was_empty = state[dir].is_empty();

        // Register the current task's waker.
        if let Some(w) = state[dir].waker.take() {
            if w.will_wake(cx.waker()) {
                state[dir].waker = Some(w);
                return Poll::Pending;
            }
            // Wake the previous waker because it's going to get replaced.
            panic::catch_unwind(|| w.wake()).ok();
        }

        Reactor::with(|reactor| {
            state[dir].waker = Some(cx.waker().clone());
            state[dir].ticks = Some((reactor.ticker(), state[dir].tick));

            // Update interest in this I/O handle.
            if was_empty {
                let event = {
                    let mut event = Event::none(self.key);
                    event.readable = !state[READ].is_empty();
                    event.writable = !state[WRITE].is_empty();
                    event
                };

                // Register interest in it.
                self.registration.modify(&reactor.poller, event)?;
            }

            Poll::Pending
        })
    }

    /// Waits until the I/O source is readable.
    pub fn readable<T: Send + 'static>(handle: &Async<T>) -> Readable<'_, T> {
        Readable(Self::ready(handle, READ))
    }

    /// Waits until the I/O source is readable.
    pub fn readable_owned<T: Send + 'static>(handle: Arc<Async<T>>) -> ReadableOwned<T> {
        ReadableOwned(Self::ready(handle, READ))
    }

    /// Waits until the I/O source is writable.
    pub fn writable<T: Send + 'static>(handle: &Async<T>) -> Writable<'_, T> {
        Writable(Self::ready(handle, WRITE))
    }

    /// Waits until the I/O source is writable.
    pub fn writable_owned<T: Send + 'static>(handle: Arc<Async<T>>) -> WritableOwned<T> {
        WritableOwned(Self::ready(handle, WRITE))
    }

    /// Waits until the I/O source is readable or writable.
    fn ready<H: Borrow<Async<T>> + Clone, T: Send + 'static>(handle: H, dir: usize) -> Ready<H, T> {
        Ready {
            handle,
            dir,
            ticks: None,
            index: None,
            _guard: None,
        }
    }
}

/// Future for [`Async::readable`](crate::runtime::Async::readable).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Readable<'a, T: Send + 'static>(Ready<&'a Async<T>, T>);

impl<T: Send + 'static> Future for Readable<'_, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.0).poll(cx))?;
        gst::trace!(
            RUNTIME_CAT,
            "readable: fd={:?}",
            self.0.handle.source.registration
        );
        Poll::Ready(Ok(()))
    }
}

impl<T: Send + 'static> fmt::Debug for Readable<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Readable").finish()
    }
}

/// Future for [`Async::readable_owned`](crate::runtime::Async::readable_owned).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadableOwned<T: Send + 'static>(Ready<Arc<Async<T>>, T>);

impl<T: Send + 'static> Future for ReadableOwned<T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.0).poll(cx))?;
        gst::trace!(
            RUNTIME_CAT,
            "readable_owned: fd={:?}",
            self.0.handle.source.registration
        );
        Poll::Ready(Ok(()))
    }
}

impl<T: Send + 'static> fmt::Debug for ReadableOwned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadableOwned").finish()
    }
}

/// Future for [`Async::writable`](crate::runtime::Async::writable).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Writable<'a, T: Send + 'static>(Ready<&'a Async<T>, T>);

impl<T: Send + 'static> Future for Writable<'_, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.0).poll(cx))?;
        gst::trace!(
            RUNTIME_CAT,
            "writable: fd={:?}",
            self.0.handle.source.registration
        );
        Poll::Ready(Ok(()))
    }
}

impl<T: Send + 'static> fmt::Debug for Writable<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Writable").finish()
    }
}

/// Future for [`Async::writable_owned`](crate::runtime::Async::writable_owned).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WritableOwned<T: Send + 'static>(Ready<Arc<Async<T>>, T>);

impl<T: Send + 'static> Future for WritableOwned<T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.0).poll(cx))?;
        gst::trace!(
            RUNTIME_CAT,
            "writable_owned: fd={:?}",
            self.0.handle.source.registration
        );
        Poll::Ready(Ok(()))
    }
}

impl<T: Send + 'static> fmt::Debug for WritableOwned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WritableOwned").finish()
    }
}

struct Ready<H: Borrow<Async<T>>, T: Send + 'static> {
    handle: H,
    dir: usize,
    ticks: Option<(usize, usize)>,
    index: Option<usize>,
    _guard: Option<RemoveOnDrop<H, T>>,
}

impl<H: Borrow<Async<T>>, T: Send + 'static> Unpin for Ready<H, T> {}

impl<H: Borrow<Async<T>> + Clone, T: Send + 'static> Future for Ready<H, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            ref handle,
            dir,
            ticks,
            index,
            _guard,
            ..
        } = &mut *self;

        let mut state = handle.borrow().source.state.lock().unwrap();

        // Check if the reactor has delivered an event.
        if let Some((a, b)) = *ticks {
            // If `state[dir].tick` has changed to a value other than the old reactor tick,
            // that means a newer reactor tick has delivered an event.
            if state[*dir].tick != a && state[*dir].tick != b {
                return Poll::Ready(Ok(()));
            }
        }

        let was_empty = state[*dir].is_empty();
        Reactor::with(|reactor| {
            // Register the current task's waker.
            let i = match *index {
                Some(i) => i,
                None => {
                    let i = state[*dir].wakers.insert(None);
                    *_guard = Some(RemoveOnDrop {
                        handle: handle.clone(),
                        dir: *dir,
                        key: i,
                        _marker: PhantomData,
                    });
                    *index = Some(i);
                    *ticks = Some((reactor.ticker(), state[*dir].tick));
                    i
                }
            };
            state[*dir].wakers[i] = Some(cx.waker().clone());

            // Update interest in this I/O handle.
            if was_empty {
                // Create the event that we are interested in.
                let event = {
                    let mut event = Event::none(handle.borrow().source.key);
                    event.readable = !state[READ].is_empty();
                    event.writable = !state[WRITE].is_empty();
                    event
                };

                handle
                    .borrow()
                    .source
                    .registration
                    .modify(&reactor.poller, event)?;
            }

            Poll::Pending
        })
    }
}

/// Remove waker when dropped.
struct RemoveOnDrop<H: Borrow<Async<T>>, T: Send + 'static> {
    handle: H,
    dir: usize,
    key: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<H: Borrow<Async<T>>, T: Send + 'static + 'static> Drop for RemoveOnDrop<H, T> {
    fn drop(&mut self) {
        let mut state = self.handle.borrow().source.state.lock().unwrap();
        let wakers = &mut state[self.dir].wakers;
        if wakers.contains(self.key) {
            wakers.remove(self.key);
        }
    }
}
