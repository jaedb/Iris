// Copyright (C) 2018-2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::future::poll_fn;
use futures::pin_mut;

use std::cell::OnceCell;
use std::future::Future;
use std::panic;
#[cfg(feature = "tuning")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as sync_mpsc;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::task::Poll;
use std::thread;
use std::time::{Duration, Instant};

use waker_fn::waker_fn;

use super::task::{SubTaskOutput, TaskId, TaskQueue};
use super::{CallOnDrop, JoinHandle, Reactor};
use crate::runtime::RUNTIME_CAT;

thread_local! {
    static CURRENT_SCHEDULER: OnceCell<Scheduler> = const { OnceCell::new() };
}

#[derive(Debug)]
pub(super) enum Scheduler {
    Blocking(Blocking),
    Throttling(ThrottlingHandleWeak),
}

impl Scheduler {
    pub fn is_blocking(&self) -> bool {
        matches!(*self, Scheduler::Blocking(..))
    }
}

#[derive(Debug, Default)]
pub(super) struct Blocking {
    tasks_queue: TaskQueue,
}

impl Blocking {
    const MAX_SUCCESSIVE_TASKS: usize = 64;

    pub fn block_on<F>(future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        CURRENT_SCHEDULER.with(|cur_sched| {
            let cur_sched = cur_sched.get_or_init(|| {
                Reactor::init(Duration::ZERO);
                Scheduler::Blocking(Blocking::default())
            });

            match cur_sched {
                Scheduler::Throttling(hdl_weak) => {
                    let msg = if let Some(hdl) = hdl_weak.upgrade() {
                        format!(
                            "Attempt to block on existing Context {}",
                            hdl.context_name()
                        )
                    } else {
                        "Attempt to block on terminated Context".to_string()
                    };
                    gst::error!(RUNTIME_CAT, "{msg}");
                    panic!("{msg}");
                }
                Scheduler::Blocking(sched) => {
                    let (task_id, task) = sched.tasks_queue.add(future);

                    gst::trace!(RUNTIME_CAT, "Blocking on current thread with {task_id:?}");

                    let _guard = CallOnDrop::new(|| {
                        gst::trace!(
                            RUNTIME_CAT,
                            "Done blocking on current thread with {task_id:?}",
                        );
                    });

                    // Blocking on `task` which is cheap to `poll`.
                    let waker = waker_fn(|| ());
                    let cx = &mut std::task::Context::from_waker(&waker);
                    pin_mut!(task);

                    let mut now;
                    let mut tasks_checked;
                    'main: loop {
                        now = Instant::now();
                        Reactor::with_mut(|reactor| reactor.react(now).ok());

                        if let Poll::Ready(t) = task.as_mut().poll(cx) {
                            return t;
                        }

                        tasks_checked = 0;
                        while tasks_checked < Self::MAX_SUCCESSIVE_TASKS {
                            let Ok(runnable) = sched.tasks_queue.pop_runnable() else {
                                continue 'main;
                            };

                            if let Err(err) = panic::catch_unwind(|| runnable.run()) {
                                gst::error!(
                                    RUNTIME_CAT,
                                    "A task panicked blocking on current thread"
                                );

                                panic::resume_unwind(err);
                            }

                            tasks_checked += 1;
                        }
                    }
                }
            }
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct Throttling {
    context_name: Arc<str>,
    max_throttling: Duration,
    task_queue: TaskQueue,
    must_unpark: Mutex<bool>,
    must_unpark_cvar: Condvar,
    #[cfg(feature = "tuning")]
    parked_duration: AtomicU64,
}

impl Throttling {
    const MAX_SUCCESSIVE_TASKS: usize = 64;

    pub fn start(context_name: &str, max_throttling: Duration) -> ThrottlingHandle {
        // Name the thread so that it appears in panic messages.
        let thread = thread::Builder::new().name(context_name.to_string());

        let (handle_sender, handle_receiver) = sync_mpsc::channel();
        let context_name: Arc<str> = Arc::from(context_name);
        let join = thread
            .spawn(move || {
                gst::debug!(
                    RUNTIME_CAT,
                    "Started Scheduler thread for Context {context_name}",
                );

                let handle = CURRENT_SCHEDULER.with(|cur_sched| {
                    Reactor::init(max_throttling);
                    let handle = ThrottlingHandle::new(Arc::new(Throttling {
                        context_name: context_name.clone(),
                        max_throttling,
                        ..Default::default()
                    }));

                    cur_sched.set(Scheduler::Throttling(handle.downgrade())).expect("new thread");

                    handle
                });

                let this = Arc::clone(&handle.0.scheduler);
                let must_shutdown = handle.0.must_shutdown.clone();
                let handle_weak = handle.downgrade();
                handle_sender.send(handle).unwrap();

                let shutdown_fut = poll_fn(move |_| {
                    if must_shutdown.load(Ordering::SeqCst) {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                });

                let _guard = CallOnDrop::new(|| {
                    gst::trace!(RUNTIME_CAT, "Closing Scheduler for Context {context_name}");

                    Reactor::clear();
                });

                // Blocking on `shutdown_fut` which is cheap to `poll`.
                match this.block_on_priv(shutdown_fut) {
                    Ok(_) => {
                        gst::debug!(
                            RUNTIME_CAT,
                            "Scheduler thread shut down for Context {context_name}",
                        );
                    }
                    Err(e) => {
                        gst::error!(
                            RUNTIME_CAT,
                            "Scheduler thread shut down due to an error within Context {context_name}",
                        );

                        if let Some(handle) = handle_weak.upgrade() {
                            handle.self_shutdown();
                        }

                        panic::resume_unwind(e);
                    }
                }
            })
            .expect("Failed to spawn Scheduler thread");

        let handle = handle_receiver.recv().expect("Context thread init failed");
        handle.set_join_handle(join);

        handle
    }

    // Important: the `termination_future` MUST be cheap to poll.
    //
    // Examples of appropriate `termination_future` are:
    //
    // - an `executor::Task` returned by `self.tasks.add(..)`.
    // - a `JoinHandle` returned by `Handle::spawn`.
    // - a custom future with few cycles (ex. checking an `AtomicBool`).
    fn block_on_priv<F>(&self, termination_future: F) -> std::thread::Result<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let waker = waker_fn(|| ());
        let cx = &mut std::task::Context::from_waker(&waker);
        pin_mut!(termination_future);

        let mut now;
        // This is to ensure reactor invocation on the first iteration.
        let mut last_react = Instant::now().checked_sub(self.max_throttling).unwrap();
        let mut tasks_checked;
        'main: loop {
            // Only check I/O and timers every `max_throttling`.
            now = Instant::now();
            if now - last_react >= self.max_throttling {
                last_react = now;
                Reactor::with_mut(|reactor| reactor.react(now).ok());
            }

            if let Poll::Ready(t) = termination_future.as_mut().poll(cx) {
                return Ok(t);
            }

            tasks_checked = 0;
            while tasks_checked < Self::MAX_SUCCESSIVE_TASKS {
                if let Ok(runnable) = self.task_queue.pop_runnable() {
                    panic::catch_unwind(|| runnable.run()).inspect_err(|_err| {
                        gst::error!(RUNTIME_CAT, "A task panicked {}", self.context_name);
                    })?;

                    tasks_checked += 1;
                } else {
                    let mut must_unpark = self.must_unpark.lock().unwrap();
                    loop {
                        if *must_unpark {
                            *must_unpark = false;
                            continue 'main;
                        }

                        if let Some(parking_duration) =
                            self.max_throttling.checked_sub(last_react.elapsed())
                        {
                            #[cfg(feature = "tuning")]
                            self.parked_duration.fetch_add(
                                parking_duration.subsec_nanos() as u64,
                                Ordering::Relaxed,
                            );

                            let result = self
                                .must_unpark_cvar
                                .wait_timeout(must_unpark, parking_duration)
                                .unwrap();

                            must_unpark = result.0;
                        } else {
                            *must_unpark = false;
                            continue 'main;
                        }
                    }
                }
            }
        }
    }

    fn unpark(&self) {
        let mut must_unpark = self.must_unpark.lock().unwrap();
        *must_unpark = true;
        self.must_unpark_cvar.notify_one();
    }

    pub fn is_throttling_thread() -> bool {
        CURRENT_SCHEDULER.with(|cur_scheduler| {
            let Some(sched) = cur_scheduler.get() else {
                return false;
            };
            !sched.is_blocking()
        })
    }

    pub fn current() -> Option<ThrottlingHandle> {
        CURRENT_SCHEDULER.with(|cur_scheduler| match cur_scheduler.get()? {
            Scheduler::Blocking(_) => None,
            Scheduler::Throttling(hdl_weak) => hdl_weak.upgrade(),
        })
    }

    fn is_current(&self) -> bool {
        CURRENT_SCHEDULER.with(|cur_sched| match cur_sched.get() {
            None | Some(Scheduler::Blocking(_)) => false,
            Some(Scheduler::Throttling(hdl_weak)) => {
                if let Some(hdl) = hdl_weak.upgrade() {
                    std::ptr::eq(self, Arc::as_ptr(&hdl.0.scheduler))
                } else {
                    false
                }
            }
        })
    }
}

#[derive(Debug)]
struct ThrottlingHandleInner {
    scheduler: Arc<Throttling>,
    must_shutdown: Arc<AtomicBool>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}

impl ThrottlingHandleInner {
    fn new(scheduler: Arc<Throttling>) -> Self {
        ThrottlingHandleInner {
            scheduler,
            must_shutdown: Default::default(),
            join: Default::default(),
        }
    }

    #[track_caller]
    fn context_name(&self) -> &str {
        self.scheduler.context_name.as_ref()
    }
}

impl Drop for ThrottlingHandleInner {
    fn drop(&mut self) {
        if !self.must_shutdown.fetch_or(true, Ordering::SeqCst) {
            // Was not already shutting down.
            self.scheduler.unpark();

            gst::trace!(
                RUNTIME_CAT,
                "Shutting down Scheduler thread for Context {}",
                self.context_name(),
            );

            // Don't block shutting down itself
            if !self.scheduler.is_current() {
                if let Some(join_handler) = self.join.lock().unwrap().take() {
                    gst::trace!(
                        RUNTIME_CAT,
                        "Waiting for Scheduler thread to shutdown for Context {}",
                        self.scheduler.context_name
                    );

                    let _ = join_handler.join();
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ThrottlingHandleWeak(Weak<ThrottlingHandleInner>);

impl ThrottlingHandleWeak {
    pub(super) fn upgrade(&self) -> Option<ThrottlingHandle> {
        self.0.upgrade().map(ThrottlingHandle)
    }
}

#[derive(Clone, Debug)]
pub(super) struct ThrottlingHandle(Arc<ThrottlingHandleInner>);

impl ThrottlingHandle {
    fn new(scheduler: Arc<Throttling>) -> Self {
        ThrottlingHandle(Arc::new(ThrottlingHandleInner::new(scheduler)))
    }

    fn set_join_handle(&self, join: thread::JoinHandle<()>) {
        *self.0.join.lock().unwrap() = Some(join);
    }

    fn self_shutdown(self) {
        self.0.must_shutdown.store(true, Ordering::SeqCst);
        *self.0.join.lock().unwrap() = None;
    }

    pub fn context_name(&self) -> &str {
        self.0.context_name()
    }

    pub fn max_throttling(&self) -> Duration {
        self.0.scheduler.max_throttling
    }

    #[cfg(feature = "tuning")]
    pub fn parked_duration(&self) -> Duration {
        Duration::from_nanos(self.0.scheduler.parked_duration.load(Ordering::Relaxed))
    }

    /// Executes the provided function relatively to this [`Scheduler`]'s [`Reactor`].
    ///
    /// Useful to initialize i/o sources and timers from outside
    /// of a [`Scheduler`].
    ///
    /// # Panic
    ///
    /// This will block current thread and would panic if run
    /// from the [`Scheduler`].
    pub fn enter<'a, F, O>(&'a self, f: F) -> O
    where
        F: FnOnce() -> O + Send + 'a,
        O: Send + 'a,
    {
        assert!(
            !self.0.scheduler.is_current(),
            "Attempting to `enter()` current `Context` from itself"
        );

        // Safety: bounding `self` to `'a` and blocking on the task
        // ensures that the lifetime bounds satisfy the safety
        // requirements for `TaskQueue::add_sync`.
        let task = unsafe { self.0.scheduler.task_queue.add_sync(f) };
        self.0.scheduler.unpark();
        futures::executor::block_on(task)
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (task_id, task) = self.0.scheduler.task_queue.add(future);
        JoinHandle::new(task_id, task, self)
    }

    pub fn spawn_and_unpark<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (task_id, task) = self.0.scheduler.task_queue.add(future);
        self.0.scheduler.unpark();
        JoinHandle::new(task_id, task, self)
    }

    pub(super) fn unpark(&self) {
        self.0.scheduler.unpark();
    }

    pub fn add_sub_task<T>(&self, task_id: TaskId, sub_task: T) -> Result<(), T>
    where
        T: Future<Output = SubTaskOutput> + Send + 'static,
    {
        self.0.scheduler.task_queue.add_sub_task(task_id, sub_task)
    }

    pub fn downgrade(&self) -> ThrottlingHandleWeak {
        ThrottlingHandleWeak(Arc::downgrade(&self.0))
    }

    pub async fn drain_sub_tasks(&self, task_id: TaskId) -> SubTaskOutput {
        let sub_tasks_fut = self.0.scheduler.task_queue.drain_sub_tasks(task_id);
        sub_tasks_fut.await
    }
}

impl PartialEq for ThrottlingHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::{Blocking, Reactor, Throttling, ThrottlingHandle};
    use crate::runtime::timer;

    #[test]
    fn block_on_task_join_handle() {
        use futures::channel::oneshot;
        use std::sync::mpsc;

        let (join_sender, join_receiver) = mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        std::thread::spawn(move || {
            let handle = ThrottlingHandle::new(Arc::new(Throttling {
                context_name: "block_on_task_join_handle".into(),
                max_throttling: Duration::from_millis(2),
                ..Default::default()
            }));

            Reactor::init(Duration::from_millis(2));

            let join_handle = handle.spawn(async {
                timer::delay_for(Duration::from_millis(5)).await;
                42
            });

            let _ = join_sender.send(join_handle);
            let _ = handle.0.scheduler.block_on_priv(shutdown_receiver);
        });

        let task_join_handle = join_receiver.recv().unwrap();
        let res = Blocking::block_on(task_join_handle).unwrap();

        let _ = shutdown_sender.send(());

        assert_eq!(res, 42);
    }

    #[test]
    fn block_on_timer() {
        let res = Blocking::block_on(async {
            timer::delay_for(Duration::from_millis(5)).await;
            42
        });

        assert_eq!(res, 42);
    }

    #[test]
    fn enter_non_static() {
        let handle = Throttling::start("enter_non_static", Duration::from_millis(2));

        let mut flag = false;
        handle.enter(|| flag = true);
        assert!(flag);
    }
}
