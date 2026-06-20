// Copyright (C) 2018-2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;

use std::sync::LazyLock;

use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{self, Poll};
use std::time::Duration;

use super::{scheduler, JoinHandle, SubTaskOutput, TaskId};
use crate::runtime::RUNTIME_CAT;

// We are bound to using `sync` for the `runtime` `Mutex`es. Attempts to use `async` `Mutex`es
// lead to the following issues:
//
// * `CONTEXTS`: can't `spawn` a `Future` when called from a `Context` thread via `ffi`.
// * `timers`: can't automatically `remove` the timer from `BinaryHeap` because `async drop`
//    is not available.
// * `task_queues`: can't `add` a pending task when called from a `Context` thread via `ffi`.
//
// Also, we want to be able to `acquire` a `Context` outside of an `async` context.
// These `Mutex`es must be `lock`ed for a short period.
static CONTEXTS: LazyLock<Mutex<HashMap<Arc<str>, ContextWeak>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Blocks on `future` in one way or another if possible.
///
/// IO & time related `Future`s must be handled within their own [`Context`].
/// Wait for the result using a [`JoinHandle`] or a `channel`.
///
/// If there's currently an active `Context` with a task, then the future is only queued up as a
/// pending sub task for that task.
///
/// Otherwise the current thread is blocking and the passed in future is executed.
///
/// Note that you must not pass any futures here that wait for the currently active task in one way
/// or another as this would deadlock!
#[track_caller]
pub fn block_on_or_add_subtask<Fut>(future: Fut) -> Option<Fut::Output>
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    if let Some((cur_context, cur_task_id)) = Context::current_task() {
        gst::debug!(
            RUNTIME_CAT,
            "Adding subtask to task {:?} on context {}",
            cur_task_id,
            cur_context.name()
        );
        let _ = cur_context.add_sub_task(cur_task_id, async move {
            future.await;
            Ok(())
        });
        return None;
    }

    // Not running in a Context thread so we can block
    Some(block_on(future))
}

/// Blocks on `future`.
///
/// IO & time related `Future`s must be handled within their own [`Context`].
/// Wait for the result using a [`JoinHandle`] or a `channel`.
///
/// The current thread is blocking and the passed in future is executed.
///
/// # Panics
///
/// This function panics if called within a [`Context`] thread.
#[track_caller]
pub fn block_on<Fut>(future: Fut) -> Fut::Output
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    gst::log!(RUNTIME_CAT, "Blocking on local thread");
    scheduler::Blocking::block_on(future)
}

/// Yields execution back to the runtime.
#[inline]
pub fn yield_now() -> YieldNow {
    YieldNow::default()
}

#[derive(Debug, Default)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        if !self.0 {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContextWeak(scheduler::ThrottlingHandleWeak);

impl ContextWeak {
    pub fn upgrade(&self) -> Option<Context> {
        self.0.upgrade().map(Context)
    }
}

/// A `threadshare` `runtime` `Context`.
///
/// The `Context` provides low-level asynchronous processing features to
/// multiplex task execution on a single thread.
///
/// `Element` implementations should use [`PadSrc`] and [`PadSink`] which
///  provide high-level features.
///
/// [`PadSrc`]: ../pad/struct.PadSrc.html
/// [`PadSink`]: ../pad/struct.PadSink.html
#[derive(Clone, Debug)]
pub struct Context(scheduler::ThrottlingHandle);

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for Context {}

impl Context {
    pub fn acquire(context_name: &str, wait: Duration) -> Result<Self, io::Error> {
        let mut contexts = CONTEXTS.lock().unwrap();

        if let Some(context_weak) = contexts.get(context_name) {
            if let Some(context) = context_weak.upgrade() {
                gst::debug!(RUNTIME_CAT, "Joining Context '{}'", context.name());
                return Ok(context);
            }
        }

        let context = Context(scheduler::Throttling::start(context_name, wait));
        contexts.insert(context_name.into(), context.downgrade());

        gst::debug!(
            RUNTIME_CAT,
            "New Context '{}' throttling {wait:?}",
            context.name(),
        );
        Ok(context)
    }

    pub fn downgrade(&self) -> ContextWeak {
        ContextWeak(self.0.downgrade())
    }

    pub fn name(&self) -> &str {
        self.0.context_name()
    }

    // FIXME this could be renamed as max_throttling
    // but then, all elements should also change their
    // wait variables and properties to max_throttling.
    pub fn wait_duration(&self) -> Duration {
        self.0.max_throttling()
    }

    /// Total duration the scheduler spent parked.
    ///
    /// This is only useful for performance evaluation.
    #[cfg(feature = "tuning")]
    pub fn parked_duration(&self) -> Duration {
        self.0.parked_duration()
    }

    /// Returns `true` if a `Context` is running on current thread.
    pub fn is_context_thread() -> bool {
        scheduler::Throttling::is_throttling_thread()
    }

    /// Returns the `Context` running on current thread, if any.
    pub fn current() -> Option<Context> {
        scheduler::Throttling::current().map(Context)
    }

    /// Returns the `TaskId` running on current thread, if any.
    pub fn current_task() -> Option<(Context, TaskId)> {
        Option::zip(
            scheduler::Throttling::current().map(Context),
            TaskId::current(),
        )
    }

    /// Executes the provided function relatively to this [`Context`].
    ///
    /// Useful to initialize i/o sources and timers from outside
    /// of a [`Context`].
    ///
    /// # Panic
    ///
    /// This will block current thread and would panic if run
    /// from the [`Context`].
    #[track_caller]
    pub fn enter<'a, F, O>(&'a self, f: F) -> O
    where
        F: FnOnce() -> O + Send + 'a,
        O: Send + 'a,
    {
        if let Some(cur) = Context::current().as_ref() {
            if cur == self {
                panic!(
                    "Attempt to enter Context {} within itself, this would deadlock",
                    self.name()
                );
            } else {
                gst::warning!(
                    RUNTIME_CAT,
                    "Entering Context {} within {}",
                    self.name(),
                    cur.name()
                );
            }
        } else {
            gst::debug!(RUNTIME_CAT, "Entering Context {}", self.name());
        }

        self.0.enter(f)
    }

    pub fn spawn<Fut>(&self, future: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        self.0.spawn(future)
    }

    pub fn spawn_and_unpark<Fut>(&self, future: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        self.0.spawn_and_unpark(future)
    }

    /// Forces the scheduler to unpark.
    ///
    /// This is not needed by elements implementors as they are
    /// supposed to call [`Self::spawn_and_unpark`] when needed.
    /// However, it's useful for lower level implementations such as
    /// `runtime::Task` so as to make sure the iteration loop yields
    /// as soon as possible when a transition is requested.
    pub(in crate::runtime) fn unpark(&self) {
        self.0.unpark();
    }

    pub fn add_sub_task<T>(&self, task_id: TaskId, sub_task: T) -> Result<(), T>
    where
        T: Future<Output = SubTaskOutput> + Send + 'static,
    {
        self.0.add_sub_task(task_id, sub_task)
    }

    pub async fn drain_sub_tasks() -> SubTaskOutput {
        let (ctx, task_id) = match Context::current_task() {
            Some(task) => task,
            None => return Ok(()),
        };

        ctx.0.drain_sub_tasks(task_id).await
    }
}

impl From<scheduler::ThrottlingHandle> for Context {
    fn from(handle: scheduler::ThrottlingHandle) -> Self {
        Context(handle)
    }
}

#[cfg(test)]
mod tests {
    use futures::channel::mpsc;
    use futures::lock::Mutex;
    use futures::prelude::*;

    use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::Context;
    use crate::runtime::Async;

    type Item = i32;

    const SLEEP_DURATION_MS: u64 = 2;
    const SLEEP_DURATION: Duration = Duration::from_millis(SLEEP_DURATION_MS);
    const DELAY: Duration = Duration::from_millis(SLEEP_DURATION_MS * 10);

    #[test]
    fn block_on_timer() {
        gst::init().unwrap();

        let elapsed = crate::runtime::executor::block_on(async {
            let now = Instant::now();
            crate::runtime::timer::delay_for(DELAY).await;
            now.elapsed()
        });

        assert!(elapsed >= DELAY);
    }

    #[test]
    fn context_task_id() {
        use super::TaskId;

        gst::init().unwrap();

        let context = Context::acquire("context_task_id", SLEEP_DURATION).unwrap();
        let join_handle = context.spawn(async {
            let (ctx, task_id) = Context::current_task().unwrap();
            assert_eq!(ctx.name(), "context_task_id");
            assert_eq!(task_id, TaskId(0));
        });
        futures::executor::block_on(join_handle).unwrap();
        // TaskId(0) is vacant again

        let ctx_weak = context.downgrade();
        let join_handle = context.spawn(async move {
            let (ctx, task_id) = Context::current_task().unwrap();
            assert_eq!(task_id, TaskId(0));

            let res = ctx.add_sub_task(task_id, async move {
                let (_ctx, task_id) = Context::current_task().unwrap();
                assert_eq!(task_id, TaskId(0));
                Ok(())
            });
            assert!(res.is_ok());

            ctx_weak
                .upgrade()
                .unwrap()
                .spawn(async {
                    let (ctx, task_id) = Context::current_task().unwrap();
                    assert_eq!(task_id, TaskId(1));

                    let res = ctx.add_sub_task(task_id, async move {
                        let (_ctx, task_id) = Context::current_task().unwrap();
                        assert_eq!(task_id, TaskId(1));
                        Ok(())
                    });
                    assert!(res.is_ok());
                    assert!(Context::drain_sub_tasks().await.is_ok());

                    let (_ctx, task_id) = Context::current_task().unwrap();
                    assert_eq!(task_id, TaskId(1));
                })
                .await
                .unwrap();

            assert!(Context::drain_sub_tasks().await.is_ok());

            let (_ctx, task_id) = Context::current_task().unwrap();
            assert_eq!(task_id, TaskId(0));
        });
        futures::executor::block_on(join_handle).unwrap();
    }

    #[test]
    fn drain_sub_tasks() {
        // Setup
        gst::init().unwrap();

        let context = Context::acquire("drain_sub_tasks", SLEEP_DURATION).unwrap();

        let join_handle = context.spawn(async {
            let (sender, mut receiver) = mpsc::channel(1);
            let sender: Arc<Mutex<mpsc::Sender<Item>>> = Arc::new(Mutex::new(sender));

            let add_sub_task = move |item| {
                let sender = sender.clone();
                Context::current_task()
                    .ok_or(())
                    .and_then(|(ctx, task_id)| {
                        ctx.add_sub_task(task_id, async move {
                            sender
                                .lock()
                                .await
                                .send(item)
                                .await
                                .map_err(|_| gst::FlowError::Error)
                        })
                        .map_err(drop)
                    })
            };

            // Tests

            // Drain empty queue
            let drain_fut = Context::drain_sub_tasks();
            drain_fut.await.unwrap();

            // Add a subtask
            add_sub_task(0).unwrap();

            // Check that it was not executed yet
            receiver.try_next().unwrap_err();

            // Drain it now and check that it was executed
            let drain_fut = Context::drain_sub_tasks();
            drain_fut.await.unwrap();
            assert_eq!(receiver.try_next().unwrap(), Some(0));

            // Add another task and check that it's not executed yet
            add_sub_task(1).unwrap();
            receiver.try_next().unwrap_err();

            // Return the receiver
            receiver
        });

        let mut receiver = futures::executor::block_on(join_handle).unwrap();

        // The last sub task should be simply dropped at this point
        match receiver.try_next() {
            Ok(None) | Err(_) => (),
            other => panic!("Unexpected {other:?}"),
        }
    }

    #[test]
    fn block_on_from_sync() {
        gst::init().unwrap();

        let context = Context::acquire("block_on_from_sync", SLEEP_DURATION).unwrap();

        let bytes_sent = crate::runtime::executor::block_on(context.spawn(async {
            let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5001);
            let socket = Async::<UdpSocket>::bind(saddr).unwrap();
            let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4001);
            socket.send_to(&[0; 10], saddr).await.unwrap()
        }))
        .unwrap();
        assert_eq!(bytes_sent, 10);

        let elapsed = crate::runtime::executor::block_on(context.spawn(async {
            let start = Instant::now();
            crate::runtime::timer::delay_for(DELAY).await;
            start.elapsed()
        }))
        .unwrap();
        // Due to throttling, `Delay` may be fired earlier
        assert!(elapsed + SLEEP_DURATION / 2 >= DELAY);
    }

    #[test]
    #[should_panic]
    fn block_on_from_context() {
        gst::init().unwrap();

        let context = Context::acquire("block_on_from_context", SLEEP_DURATION).unwrap();

        // Panic: attempt to `runtime::executor::block_on` within a `Context` thread
        let join_handle = context.spawn(async {
            crate::runtime::executor::block_on(crate::runtime::timer::delay_for(DELAY));
        });

        // Panic: task has failed
        // (enforced by `async-task`, see comment in `Future` impl for `JoinHandle`).
        futures::executor::block_on(join_handle).unwrap_err();
    }

    #[test]
    fn enter_context_from_scheduler() {
        gst::init().unwrap();

        let elapsed = crate::runtime::executor::block_on(async {
            let context = Context::acquire("enter_context_from_executor", SLEEP_DURATION).unwrap();
            let socket = context
                .enter(|| {
                    let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5002);
                    Async::<UdpSocket>::bind(saddr)
                })
                .unwrap();

            let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4002);
            let bytes_sent = socket.send_to(&[0; 10], saddr).await.unwrap();
            assert_eq!(bytes_sent, 10);

            let (start, timer) =
                context.enter(|| (Instant::now(), crate::runtime::timer::delay_for(DELAY)));
            timer.await;
            start.elapsed()
        });

        // Due to throttling, `Delay` may be fired earlier
        assert!(elapsed + SLEEP_DURATION / 2 >= DELAY);
    }

    #[test]
    fn enter_context_from_sync() {
        gst::init().unwrap();

        let context = Context::acquire("enter_context_from_sync", SLEEP_DURATION).unwrap();
        let socket = context
            .enter(|| {
                let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5003);
                Async::<UdpSocket>::bind(saddr)
            })
            .unwrap();

        let saddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4003);
        let bytes_sent = futures::executor::block_on(socket.send_to(&[0; 10], saddr)).unwrap();
        assert_eq!(bytes_sent, 10);

        let (start, timer) =
            context.enter(|| (Instant::now(), crate::runtime::timer::delay_for(DELAY)));
        let elapsed = crate::runtime::executor::block_on(async move {
            timer.await;
            start.elapsed()
        });
        // Due to throttling, `Delay` may be fired earlier
        assert!(elapsed + SLEEP_DURATION / 2 >= DELAY);
    }
}
