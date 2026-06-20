// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! An execution loop to run asynchronous processing.

use futures::channel::mpsc as async_mpsc;
use futures::channel::oneshot;
use futures::prelude::*;

use std::fmt;
use std::ops::Deref;
use std::pin::{pin, Pin};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Poll;

use gst::glib;
use gst::glib::prelude::*;

use super::{Context, JoinHandle, RUNTIME_CAT};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
pub enum TaskState {
    Error,
    Flushing,
    Paused,
    PausedFlushing,
    Prepared,
    Preparing,
    Started,
    Stopped,
    Unprepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trigger {
    Error,
    FlushStart,
    FlushStop,
    Pause,
    Prepare,
    Start,
    Stop,
    Unprepare,
}

/// Transition success details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionOk {
    /// Transition completed successfully.
    Complete {
        origin: TaskState,
        target: TaskState,
    },
    /// Not waiting for transition result.
    ///
    /// This is to prevent:
    /// - A deadlock when executing a transition action.
    /// - A potential infinite wait when pausing a running loop
    ///   which could be awaiting for an `nominal` to complete.
    NotWaiting { trigger: Trigger, origin: TaskState },
    /// Skipping triggering event due to current state.
    Skipped { trigger: Trigger, state: TaskState },
}

/// TriggeringEvent error details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionError {
    pub trigger: Trigger,
    pub state: TaskState,
    pub err_msg: gst::ErrorMessage,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?} from state {:?}: {:?}",
            self.trigger, self.state, self.err_msg
        )
    }
}

impl std::error::Error for TransitionError {}

impl From<TransitionError> for gst::ErrorMessage {
    fn from(err: TransitionError) -> Self {
        err.err_msg
    }
}

/// Transition status.
///
/// A state transition occurs as a result of a triggering event.
/// The triggering event is asynchronously handled by a state machine
/// running on a [`Context`].
#[must_use = "This `TransitionStatus` may be `Pending`. In most cases it should be awaited. See `await_maybe_on_context`"]
pub enum TransitionStatus {
    /// Transition result is ready.
    Ready(Result<TransitionOk, TransitionError>),
    /// Transition is pending.
    Pending {
        trigger: Trigger,
        origin: TaskState,
        res_fut: Pin<Box<dyn Future<Output = Result<TransitionOk, TransitionError>> + Send>>,
    },
}

impl TransitionStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, TransitionStatus::Ready { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, TransitionStatus::Pending { .. })
    }

    /// Converts the `TransitionStatus` into a `Result`.
    ///
    /// This function allows getting the `TransitionError` when
    /// the transition result is ready without `await`ing nor blocking.
    ///
    /// See also [`Self::await_maybe_on_context`].
    // FIXME once stabilized, this could use https://github.com/rust-lang/rust/issues/84277
    pub fn check(self) -> Result<TransitionStatus, TransitionError> {
        match self {
            TransitionStatus::Ready(Err(err)) => Err(err),
            other => Ok(other),
        }
    }

    /// Blocks on this state transition to complete, or adds a subtask if running on a [`Context`].
    ///
    /// Notes:
    ///
    /// - If you need to execute code after the transition succeeds or fails,
    ///   see [`block_on_or_add_subtask_then`].
    /// - When running in an `async` block within a running transition or
    ///   task iteration, don't await for the transition as it would deadlock.
    ///   Use [`Self::check`] to make sure the state transition is valid.
    /// - When running in an `async` block out of a running transition or
    ///   task iteration, just `.await` normally. E.g.:
    ///
    /// ```
    /// # use gstthreadshare::runtime::task::{Task, TransitionOk, TransitionError};
    /// # async fn async_fn() -> Result<TransitionOk, TransitionError> {
    /// # let task = Task::default();
    ///   let flush_ok = task.flush_start().await?;
    /// # Ok(flush_ok)
    /// # }
    /// ```
    ///
    /// This function makes sure the transition completes successfully or
    /// produces an error. It must be used in situations where we don't know
    /// whether we are running on a [`Context`] or not. This is the case for
    /// functions in [`PadSrc`] and [`PadSink`] as well as the synchronous
    /// functions transitively called from them.
    ///
    /// As an example, a `PadSrc::src_event` function which handles a
    /// `FlushStart` could call:
    ///
    /// ```
    /// # fn src_event() -> bool {
    /// # let task = gstthreadshare::runtime::Task::default();
    ///   return task
    ///       .flush_start()
    ///       .block_on_or_add_subtask()
    ///       .is_ok();
    /// # }
    /// ```
    ///
    /// If the transition is already complete, the result is returned immediately.
    ///
    /// If we are NOT running on a [`Context`], the transition result is awaited
    /// by blocking on current thread and the result is returned.
    ///
    /// If we are running on a [`Context`], the transition result is awaited
    /// in a sub task for current [`Context`]'s Scheduler task. As a consequence,
    /// the sub task will be awaited in usual [`Context::drain_sub_tasks`]
    /// rendezvous, ensuring some kind of synchronization. To avoid deadlocks,
    /// `Ok(TransitionOk::NotWaiting { .. })` is immediately returned.
    ///
    /// [`PadSrc`]: ../pad/struct.PadSrc.html
    /// [`PadSink`]: ../pad/struct.PadSink.html
    pub fn block_on_or_add_subtask<O>(self, obj: &O) -> Result<TransitionOk, TransitionError>
    where
        O: IsA<glib::Object> + Send,
    {
        use TransitionStatus::*;
        match self {
            Pending {
                trigger,
                origin,
                res_fut,
            } => {
                if let Some((ctx, task_id)) = Context::current_task() {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Awaiting for {trigger:?} ack in a subtask on context {}",
                        ctx.name()
                    );

                    let obj = obj.clone();
                    let _ = ctx.add_sub_task(task_id, async move {
                        let res = res_fut.await;
                        match res {
                            Ok(status) => {
                                gst::log!(
                                    RUNTIME_CAT,
                                    obj = obj,
                                    "Task {trigger:?} success: {status:?}",
                                );
                            }
                            Err(err) => {
                                gst::error!(
                                    RUNTIME_CAT,
                                    obj = obj,
                                    "Task {trigger:?} failure: {err}"
                                );
                            }
                        }

                        Ok(())
                    });

                    Ok(TransitionOk::NotWaiting { trigger, origin })
                } else {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Awaiting for {trigger:?} ack on current thread",
                    );
                    let res = futures::executor::block_on(res_fut);
                    match res {
                        Ok(ref status) => {
                            gst::log!(
                                RUNTIME_CAT,
                                obj = obj,
                                "Task {trigger:?} success: {status:?}",
                            );
                        }
                        Err(ref err) => {
                            gst::error!(RUNTIME_CAT, obj = obj, "Task {trigger:?} failure: {err}");
                        }
                    }

                    res
                }
            }
            Ready(res) => {
                match res {
                    Ok(ref status) => {
                        gst::log!(
                            RUNTIME_CAT,
                            obj = obj,
                            "Task transition immediate success: {status:?}",
                        );
                    }
                    Err(ref err) => {
                        gst::error!(
                            RUNTIME_CAT,
                            obj = obj,
                            "Task transition immediate failure: {err}",
                        );
                    }
                }

                res
            }
        }
    }

    /// Blocks on this state transition to complete, or adds a subtask if running on a [`Context`]
    /// executing the provided function after the transition succeeds or fails.
    ///
    /// Compared to [`block_on_or_addsubtask`], this function also executes the provieded
    /// `func` after the transition succeeded or failed. Code following [`block_on_or_addsubtask`]
    /// can actually be executed before the transition if a subtask was added and the returned
    /// `Result` might not reflect the actual transition result.
    ///
    /// If the transition is already complete, `func` is executed immediately.
    ///
    /// If we are NOT running on a [`Context`], the transition result is awaited
    /// by blocking on current thread and `func` is executed.
    ///
    /// If we are running on a [`Context`], the transition result is awaited
    /// in a sub task for current [`Context`]'s Scheduler task and `func` is executed with
    /// the transition result. In this case, `block_on_or_add_subtask_then` always
    /// returns `Ok(())` since the actual processing is handled asynchronously.
    ///
    /// ## Example
    ///
    /// ```
    /// # use gstthreadshare::runtime::task::{Task, TransitionOk, TransitionError};
    /// # async fn async_fn() -> Result<TransitionOk, TransitionError> {
    /// # let task = Task::default();
    ///   task
    ///       .stop()
    ///       .block_on_or_add_subtask_then(self.obj(), |elem, res| {
    ///           // Add specific stop code here,
    ///           // it will be executed after the transition succeeds or fails
    ///
    ///           if res.is_ok() {
    ///               gst::debug!(CAT, obj = elem, "Stopped");
    ///           }
    ///       })
    /// # Ok(flush_ok)
    /// # }
    /// ```
    pub fn block_on_or_add_subtask_then<T, F>(
        self,
        obj: glib::BorrowedObject<'_, T>,
        func: F,
    ) -> Result<(), gst::ErrorMessage>
    where
        T: IsA<glib::Object> + Send,
        F: FnOnce(&T, &Result<TransitionOk, TransitionError>) + Send + 'static,
    {
        use TransitionStatus::*;
        match self {
            Pending {
                trigger, res_fut, ..
            } => {
                if let Some((ctx, task_id)) = Context::current_task() {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Awaiting for {trigger:?} ack in a subtask on context {}",
                        ctx.name()
                    );
                    let obj = obj.clone();
                    let _ = ctx.add_sub_task(task_id, async move {
                        let res = res_fut.await;
                        match res {
                            Ok(ref status) => {
                                gst::log!(
                                    RUNTIME_CAT,
                                    obj = obj,
                                    "Task {trigger:?} success: {status:?}",
                                );
                                func(&obj, &res);
                                Ok(())
                            }
                            Err(ref err) => {
                                gst::error!(
                                    RUNTIME_CAT,
                                    obj = obj,
                                    "Task {trigger:?} failure: {err}",
                                );
                                func(&obj, &res);
                                Err(gst::FlowError::Error)
                            }
                        }
                    });

                    Ok(())
                } else {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Awaiting for {trigger:?} ack on current thread",
                    );
                    let res = futures::executor::block_on(res_fut);
                    match res {
                        Ok(ref status) => {
                            gst::log!(
                                RUNTIME_CAT,
                                obj = obj,
                                "Task {trigger:?} success: {status:?}",
                            );
                            func(&obj, &res);
                            Ok(())
                        }
                        Err(ref err) => {
                            gst::error!(RUNTIME_CAT, obj = obj, "Task {trigger:?} failure: {err}",);
                            func(&obj, &res);
                            res.map(|_| ()).map_err(|err| err.into())
                        }
                    }
                }
            }
            Ready(res) => match res {
                Ok(ref status) => {
                    gst::log!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Task transition immediate success: {status:?}",
                    );
                    func(&obj, &res);
                    Ok(())
                }
                Err(ref err) => {
                    gst::error!(
                        RUNTIME_CAT,
                        obj = obj,
                        "Task transition immediate failure: {err}",
                    );
                    func(&obj, &res);
                    res.map(|_| ()).map_err(|err| err.into())
                }
            },
        }
    }
}

impl Future for TransitionStatus {
    type Output = Result<TransitionOk, TransitionError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        use TransitionStatus::*;

        match &mut *self {
            Ready(res) => Poll::Ready(res.clone()),
            Pending { res_fut, .. } => match Pin::new(res_fut).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(res) => {
                    *self = Ready(res.clone());

                    Poll::Ready(res)
                }
            },
        }
    }
}

impl From<TransitionOk> for TransitionStatus {
    fn from(ok: TransitionOk) -> Self {
        Self::Ready(Ok(ok))
    }
}

impl From<TransitionError> for TransitionStatus {
    fn from(err: TransitionError) -> Self {
        Self::Ready(Err(err))
    }
}

// Explicit impl due to `res_fut` not implementing `Debug`.
impl fmt::Debug for TransitionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TransitionStatus::*;
        match self {
            Ready(res) => f.debug_tuple("Ready").field(res).finish(),
            Pending {
                trigger, origin, ..
            } => f
                .debug_struct("Pending")
                .field("trigger", trigger)
                .field("origin", origin)
                .finish(),
        }
    }
}

/// Implementation trait for `Task`s.
///
/// Defines implementations for state transition actions and error handlers.
pub trait TaskImpl: Send + 'static {
    type Item: Send + 'static;

    fn obj(&self) -> &impl IsA<glib::Object>;

    fn prepare(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ok(())
    }

    fn unprepare(&mut self) -> impl Future<Output = ()> + Send {
        future::ready(())
    }

    fn start(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ok(())
    }

    /// Tries to retrieve the next item to process.
    ///
    /// With [`Self::handle_item`], this is one of the two `Task` loop
    /// functions. They are executed in a loop in the `Started` state.
    ///
    /// Function `try_next` is awaited at the beginning of each iteration,
    /// and can be cancelled at `await` point if a state transition is requested.
    ///
    /// If `Ok(item)` is returned, the iteration calls [`Self::handle_item`]
    /// with said `Item`.
    ///
    /// If `Err(..)` is returned, the iteration calls [`Self::handle_loop_error`].
    fn try_next(&mut self) -> impl Future<Output = Result<Self::Item, gst::FlowError>> + Send;

    /// Does whatever needs to be done with the `item`.
    ///
    /// With [`Self::try_next`], this is one of the two `Task` loop
    /// functions. They are executed in a loop in the `Started` state.
    ///
    /// Function `handle_item` asynchronously processes an `item` previously
    /// retrieved by [`Self::try_next`]. Processing is guaranteed to run
    /// to completion even if a state transition is requested.
    ///
    /// If `Err(..)` is returned, the iteration calls [`Self::handle_loop_error`].
    fn handle_item(
        &mut self,
        _item: Self::Item,
    ) -> impl Future<Output = Result<(), gst::FlowError>> + Send;

    fn pause(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ok(())
    }

    fn flush_start(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ready(Ok(()))
    }

    fn flush_stop(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ready(Ok(()))
    }

    fn stop(&mut self) -> impl Future<Output = Result<(), gst::ErrorMessage>> + Send {
        future::ready(Ok(()))
    }

    /// Handles an error occurring during the execution of the `Task` loop.
    ///
    /// This include errors returned by [`Self::try_next`] & [`Self::handle_item`].
    ///
    /// If the error is unrecoverable, implementations might use
    /// `gst::Element::post_error_message` and return `Trigger::Error`.
    ///
    /// Otherwise, handle the error and return the requested `Transition` to recover.
    ///
    /// Default behaviour depends on the `err`:
    ///
    /// - `FlowError::Flushing` -> `Trigger::FlushStart`.
    /// - `FlowError::Eos` -> `Trigger::Stop`.
    /// - Other `FlowError` -> `Trigger::Error`.
    fn handle_loop_error(&mut self, err: gst::FlowError) -> impl Future<Output = Trigger> + Send {
        async move {
            match err {
                gst::FlowError::Flushing => {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = self.obj(),
                        "Task loop returned Flushing. Posting FlushStart"
                    );
                    Trigger::FlushStart
                }
                gst::FlowError::Eos => {
                    gst::debug!(
                        RUNTIME_CAT,
                        obj = self.obj(),
                        "Task loop returned Eos. Posting Stop"
                    );
                    Trigger::Stop
                }
                other => {
                    gst::error!(
                        RUNTIME_CAT,
                        obj = self.obj(),
                        "Task loop returned {other:?}. Posting Error",
                    );
                    Trigger::Error
                }
            }
        }
    }

    /// Handles an error occurring during the execution of a transition action.
    ///
    /// This handler also catches errors returned by subtasks spawned by the transition action.
    ///
    /// If the error is unrecoverable, implementations might use `gst::Element::post_error_message`
    /// and return `Trigger::Error`.
    ///
    /// Otherwise, handle the error and return the recovering `Trigger`.
    ///
    /// Default is to `gst::error` log and return `Trigger::Error`.
    fn handle_action_error(
        &mut self,
        trigger: Trigger,
        state: TaskState,
        err: gst::ErrorMessage,
    ) -> impl Future<Output = Trigger> + Send {
        async move {
            gst::error!(
                RUNTIME_CAT,
                obj = self.obj(),
                "TaskImpl transition action error during {trigger:?} from {state:?}: {err:?}. Posting Trigger::Error",
            );

            Trigger::Error
        }
    }
}

type AckSender = oneshot::Sender<Result<TransitionOk, TransitionError>>;
type AckReceiver = oneshot::Receiver<Result<TransitionOk, TransitionError>>;

struct TriggeringEvent {
    trigger: Trigger,
    ack_tx: AckSender,
}

impl TriggeringEvent {
    fn new(trigger: Trigger) -> (Self, AckReceiver) {
        let (ack_tx, ack_rx) = oneshot::channel();
        let req = TriggeringEvent { trigger, ack_tx };

        (req, ack_rx)
    }

    fn send_ack(self, res: Result<TransitionOk, TransitionError>) {
        let _ = self.ack_tx.send(res);
    }

    fn send_err_ack(self) {
        let res = Err(TransitionError {
            trigger: self.trigger,
            state: TaskState::Error,
            err_msg: gst::error_msg!(
                gst::CoreError::StateChange,
                [
                    "Triggering Event {:?} rejected due to a previous unrecoverable error",
                    self.trigger,
                ]
            ),
        });

        self.send_ack(res);
    }
}

impl fmt::Debug for TriggeringEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TriggeringEvent")
            .field("trigger", &self.trigger)
            .finish()
    }
}

#[derive(Debug)]
struct StateMachineHandle {
    join_handle: JoinHandle<()>,
    triggering_evt_tx: async_mpsc::Sender<TriggeringEvent>,
    context: Context,
}

impl StateMachineHandle {
    fn trigger(&mut self, trigger: Trigger) -> AckReceiver {
        let (triggering_evt, ack_rx) = TriggeringEvent::new(trigger);

        gst::log!(RUNTIME_CAT, "Pushing {triggering_evt:?}");
        self.triggering_evt_tx.try_send(triggering_evt).unwrap();

        self.context.unpark();

        ack_rx
    }

    async fn join(self) {
        self.join_handle
            .await
            .expect("state machine shouldn't have been cancelled");
    }
}

#[derive(Debug)]
struct TaskInner {
    state: TaskState,
    state_machine_handle: Option<StateMachineHandle>,
}

impl Default for TaskInner {
    fn default() -> Self {
        TaskInner {
            state: TaskState::Unprepared,
            state_machine_handle: None,
        }
    }
}

impl TaskInner {
    fn switch_to_state(&mut self, target_state: TaskState, triggering_evt: TriggeringEvent) {
        let res = Ok(TransitionOk::Complete {
            origin: self.state,
            target: target_state,
        });

        self.state = target_state;
        triggering_evt.send_ack(res);
    }

    fn switch_to_err(&mut self, triggering_evt: TriggeringEvent) {
        let res = Err(TransitionError {
            trigger: triggering_evt.trigger,
            state: self.state,
            err_msg: gst::error_msg!(
                gst::CoreError::StateChange,
                [
                    "Unrecoverable error for {triggering_evt:?} from state {:?}",
                    self.state,
                ]
            ),
        });

        self.state = TaskState::Error;
        triggering_evt.send_ack(res);
    }

    fn skip_triggering_evt(&mut self, triggering_evt: TriggeringEvent) {
        let res = Ok(TransitionOk::Skipped {
            trigger: triggering_evt.trigger,
            state: self.state,
        });

        triggering_evt.send_ack(res);
    }

    fn trigger(&mut self, trigger: Trigger) -> Result<AckReceiver, TransitionError> {
        self.state_machine_handle
            .as_mut()
            .map(|state_machine| state_machine.trigger(trigger))
            .ok_or_else(|| {
                gst::warning!(RUNTIME_CAT, "Unable to send {trigger:?}: no state machine",);
                TransitionError {
                    trigger,
                    state: TaskState::Unprepared,
                    err_msg: gst::error_msg!(
                        gst::ResourceError::NotFound,
                        ["Unable to send {trigger:?}: no state machine"]
                    ),
                }
            })
    }
}

impl Drop for TaskInner {
    fn drop(&mut self) {
        if self.state != TaskState::Unprepared {
            // Don't panic here: in case another panic occurs, we would get
            // "panicked while panicking" which would prevents developers
            // from getting the initial panic message.
            gst::fixme!(RUNTIME_CAT, "Missing call to `Task::unprepare`");
        }
    }
}

/// An RAII implementation of scoped locked `TaskState`.
pub struct TaskStateGuard<'guard>(MutexGuard<'guard, TaskInner>);

impl Deref for TaskStateGuard<'_> {
    type Target = TaskState;

    fn deref(&self) -> &Self::Target {
        &(self.0).state
    }
}

/// A `Task` operating on a `threadshare` [`Context`].
///
/// [`Context`]: ../executor/struct.Context.html
#[derive(Debug, Clone)]
pub struct Task(Arc<Mutex<TaskInner>>);

impl Default for Task {
    fn default() -> Self {
        Task(Arc::new(Mutex::new(TaskInner::default())))
    }
}

impl Task {
    pub fn state(&self) -> TaskState {
        self.0.lock().unwrap().state
    }

    pub fn lock_state(&self) -> TaskStateGuard<'_> {
        TaskStateGuard(self.0.lock().unwrap())
    }

    pub fn prepare(&self, task_impl: impl TaskImpl, context: Context) -> TransitionStatus {
        let mut inner = self.0.lock().unwrap();

        let origin = inner.state;
        match origin {
            TaskState::Unprepared => (),
            TaskState::Prepared | TaskState::Preparing => {
                gst::debug!(
                    RUNTIME_CAT,
                    obj = task_impl.obj(),
                    "Task already {origin:?}",
                );
                return TransitionOk::Skipped {
                    trigger: Trigger::Prepare,
                    state: origin,
                }
                .into();
            }
            state => {
                gst::warning!(
                    RUNTIME_CAT,
                    obj = task_impl.obj(),
                    "Attempt to prepare Task in state {state:?}"
                );
                return TransitionError {
                    trigger: Trigger::Prepare,
                    state: inner.state,
                    err_msg: gst::error_msg!(
                        gst::CoreError::StateChange,
                        ["Attempt to prepare Task in state {state:?}"]
                    ),
                }
                .into();
            }
        }

        assert!(inner.state_machine_handle.is_none());

        inner.state = TaskState::Preparing;

        gst::log!(
            RUNTIME_CAT,
            obj = task_impl.obj(),
            "Spawning task state machine"
        );
        inner.state_machine_handle = Some(StateMachine::spawn(self.0.clone(), task_impl, context));

        let ack_rx = match inner.trigger(Trigger::Prepare) {
            Ok(ack_rx) => ack_rx,
            Err(err) => return err.into(),
        };
        drop(inner);

        TransitionStatus::Pending {
            trigger: Trigger::Prepare,
            origin: TaskState::Unprepared,
            res_fut: Box::pin(ack_rx.map(Result::unwrap)),
        }
    }

    pub fn unprepare(&self) -> TransitionStatus {
        let mut inner = self.0.lock().unwrap();

        let origin = inner.state;
        let mut state_machine_handle = match origin {
            TaskState::Stopped
            | TaskState::Error
            | TaskState::Prepared
            | TaskState::Preparing
            | TaskState::Unprepared => match inner.state_machine_handle.take() {
                Some(state_machine_handle) => {
                    gst::debug!(RUNTIME_CAT, "Unpreparing task");

                    state_machine_handle
                }
                None => {
                    gst::debug!(RUNTIME_CAT, "Task already unpreparing");
                    return TransitionOk::Skipped {
                        trigger: Trigger::Unprepare,
                        state: origin,
                    }
                    .into();
                }
            },
            state => {
                gst::warning!(RUNTIME_CAT, "Attempt to unprepare Task in state {state:?}");
                return TransitionError {
                    trigger: Trigger::Unprepare,
                    state: inner.state,
                    err_msg: gst::error_msg!(
                        gst::CoreError::StateChange,
                        ["Attempt to unprepare Task in state {state:?}"]
                    ),
                }
                .into();
            }
        };

        let ack_rx = state_machine_handle.trigger(Trigger::Unprepare);
        drop(inner);

        let state_machine_end_fut = async {
            state_machine_handle.join().await;
            ack_rx.await.unwrap()
        };

        TransitionStatus::Pending {
            trigger: Trigger::Unprepare,
            origin,
            res_fut: Box::pin(state_machine_end_fut),
        }
    }

    /// Starts the `Task`.
    ///
    /// The execution occurs on the `Task` context.
    pub fn start(&self) -> TransitionStatus {
        let mut inner = self.0.lock().unwrap();

        if let TaskState::Started = inner.state {
            return TransitionOk::Skipped {
                trigger: Trigger::Start,
                state: TaskState::Started,
            }
            .into();
        }

        let ack_rx = match inner.trigger(Trigger::Start) {
            Ok(ack_rx) => ack_rx,
            Err(err) => return err.into(),
        };

        let origin = inner.state;
        drop(inner);

        TransitionStatus::Pending {
            trigger: Trigger::Start,
            origin,
            res_fut: Box::pin(ack_rx.map(Result::unwrap)),
        }
    }

    /// Requests the `Task` loop to pause.
    ///
    /// If an item handling is in progress, it will run to completion,
    /// then no iterations will be executed before `start` is called again.
    pub fn pause(&self) -> TransitionStatus {
        self.push_pending(Trigger::Pause)
    }

    pub fn flush_start(&self) -> TransitionStatus {
        self.push_pending(Trigger::FlushStart)
    }

    pub fn flush_stop(&self) -> TransitionStatus {
        self.push_pending(Trigger::FlushStop)
    }

    /// Stops the `Started` `Task` and wait for it to finish.
    pub fn stop(&self) -> TransitionStatus {
        self.push_pending(Trigger::Stop)
    }

    /// Pushes a [`Trigger`] and returns TransitionStatus::Pending.
    fn push_pending(&self, trigger: Trigger) -> TransitionStatus {
        let mut inner = self.0.lock().unwrap();

        let ack_rx = match inner.trigger(trigger) {
            Ok(ack_rx) => ack_rx,
            Err(err) => return err.into(),
        };

        let origin = inner.state;
        drop(inner);

        TransitionStatus::Pending {
            trigger,
            origin,
            res_fut: Box::pin(ack_rx.map(Result::unwrap)),
        }
    }
}

struct StateMachine<Task: TaskImpl> {
    task_impl: Task,
    triggering_evt_rx: async_mpsc::Receiver<TriggeringEvent>,
    pending_triggering_evt: Option<TriggeringEvent>,
}

macro_rules! exec_action {
    ($self:ident, $action:ident, $triggering_evt:expr, $origin:expr, $task_inner:expr) => {{
        match $self.task_impl.$action().await {
            Ok(()) => Ok($triggering_evt),
            Err(err) => {
                // FIXME problem is that we loose the origin trigger in the
                //       final TransitionStatus.

                let next_trigger = $self
                    .task_impl
                    .handle_action_error($triggering_evt.trigger, $origin, err)
                    .await;

                // Convert triggering event according to the error handler's decision
                gst::trace!(
                    RUNTIME_CAT,
                    obj = $self.task_impl.obj(),
                    "TaskImpl transition action error: converting {:?} to {next_trigger:?}",
                    $triggering_evt.trigger,
                );

                $triggering_evt.trigger = next_trigger;
                $self.pending_triggering_evt = Some($triggering_evt);

                Err(())
            }
        }
    }};
}

impl<Task: TaskImpl> StateMachine<Task> {
    fn spawn(
        task_inner: Arc<Mutex<TaskInner>>,
        task_impl: Task,
        context: Context,
    ) -> StateMachineHandle {
        let (triggering_evt_tx, triggering_evt_rx) = async_mpsc::channel(4);

        let state_machine = StateMachine {
            task_impl,
            triggering_evt_rx,
            pending_triggering_evt: None,
        };

        StateMachineHandle {
            join_handle: context.spawn_and_unpark(state_machine.run(task_inner)),
            triggering_evt_tx,
            context,
        }
    }

    async fn run(mut self, task_inner: Arc<Mutex<TaskInner>>) {
        let mut triggering_evt = self
            .triggering_evt_rx
            .next()
            .await
            .expect("triggering_evt_rx dropped");

        if let Trigger::Prepare = triggering_evt.trigger {
            gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Preparing task");

            let res = exec_action!(
                self,
                prepare,
                triggering_evt,
                TaskState::Unprepared,
                &task_inner
            );
            if let Ok(triggering_evt) = res {
                let mut task_inner = task_inner.lock().unwrap();
                let res = Ok(TransitionOk::Complete {
                    origin: TaskState::Unprepared,
                    target: TaskState::Prepared,
                });

                task_inner.state = TaskState::Prepared;
                triggering_evt.send_ack(res);

                gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Task Prepared");
            }
        } else {
            panic!("Unexpected initial trigger {:?}", triggering_evt.trigger);
        }

        loop {
            triggering_evt = match self.pending_triggering_evt.take() {
                Some(pending_triggering_evt) => pending_triggering_evt,
                None => self
                    .triggering_evt_rx
                    .next()
                    .await
                    .expect("triggering_evt_rx dropped"),
            };
            gst::trace!(
                RUNTIME_CAT,
                obj = self.task_impl.obj(),
                "State machine popped {triggering_evt:?}"
            );

            match triggering_evt.trigger {
                Trigger::Error => {
                    let mut task_inner = task_inner.lock().unwrap();
                    task_inner.switch_to_err(triggering_evt);
                    gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Switched to Error");
                }
                Trigger::Start => {
                    let origin = {
                        let mut task_inner = task_inner.lock().unwrap();
                        let origin = task_inner.state;
                        match origin {
                            TaskState::Stopped | TaskState::Paused | TaskState::Prepared => (),
                            TaskState::PausedFlushing => {
                                task_inner.switch_to_state(TaskState::Flushing, triggering_evt);
                                gst::trace!(
                                    RUNTIME_CAT,
                                    obj = self.task_impl.obj(),
                                    "Switched from PausedFlushing to Flushing"
                                );
                                continue;
                            }
                            TaskState::Error => {
                                triggering_evt.send_err_ack();
                                continue;
                            }
                            state => {
                                task_inner.skip_triggering_evt(triggering_evt);
                                gst::trace!(
                                    RUNTIME_CAT,
                                    obj = self.task_impl.obj(),
                                    "Skipped Start in state {state:?}"
                                );
                                continue;
                            }
                        }

                        origin
                    };

                    self.start(triggering_evt, origin, &task_inner).await;
                    // next/pending triggering event handled in next iteration
                }
                Trigger::Pause => {
                    let (origin, target) = {
                        let mut task_inner = task_inner.lock().unwrap();
                        let origin = task_inner.state;
                        match origin {
                            TaskState::Started | TaskState::Stopped | TaskState::Prepared => {
                                (origin, TaskState::Paused)
                            }
                            TaskState::Flushing => (origin, TaskState::PausedFlushing),
                            TaskState::Error => (TaskState::Error, TaskState::Error),
                            state => {
                                task_inner.skip_triggering_evt(triggering_evt);
                                gst::trace!(
                                    RUNTIME_CAT,
                                    obj = self.task_impl.obj(),
                                    "Skipped Pause in state {state:?}"
                                );
                                continue;
                            }
                        }
                    };

                    let res = exec_action!(self, pause, triggering_evt, origin, &task_inner);
                    if let Ok(triggering_evt) = res {
                        task_inner
                            .lock()
                            .unwrap()
                            .switch_to_state(target, triggering_evt);
                        gst::trace!(
                            RUNTIME_CAT,
                            obj = self.task_impl.obj(),
                            "Task loop {target:?}"
                        );
                    }
                }
                Trigger::Stop => {
                    let (origin, target) = {
                        let mut task_inner = task_inner.lock().unwrap();
                        let origin = task_inner.state;
                        match origin {
                            TaskState::Started
                            | TaskState::Paused
                            | TaskState::PausedFlushing
                            | TaskState::Flushing => (origin, TaskState::Stopped),
                            TaskState::Error => (TaskState::Error, TaskState::Error),
                            state => {
                                task_inner.skip_triggering_evt(triggering_evt);
                                gst::trace!(
                                    RUNTIME_CAT,
                                    obj = self.task_impl.obj(),
                                    "Skipped Stop in state {state:?}"
                                );
                                continue;
                            }
                        }
                    };

                    let res = exec_action!(self, stop, triggering_evt, origin, &task_inner);
                    if let Ok(triggering_evt) = res {
                        task_inner
                            .lock()
                            .unwrap()
                            .switch_to_state(target, triggering_evt);
                        gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Task {target:?}");
                    }
                }
                Trigger::FlushStart => {
                    let (origin, target) = {
                        let mut task_inner = task_inner.lock().unwrap();
                        let origin = task_inner.state;
                        match origin {
                            TaskState::Started => (origin, TaskState::Flushing),
                            TaskState::Paused => (origin, TaskState::PausedFlushing),
                            TaskState::Error => (TaskState::Error, TaskState::Error),
                            state => {
                                task_inner.skip_triggering_evt(triggering_evt);
                                gst::trace!(
                                    RUNTIME_CAT,
                                    obj = self.task_impl.obj(),
                                    "Skipped FlushStart in state {state:?}"
                                );
                                continue;
                            }
                        }
                    };

                    let res = exec_action!(self, flush_start, triggering_evt, origin, &task_inner);
                    if let Ok(triggering_evt) = res {
                        task_inner
                            .lock()
                            .unwrap()
                            .switch_to_state(target, triggering_evt);
                        gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Task {target:?}");
                    }
                }
                Trigger::FlushStop => {
                    let origin = task_inner.lock().unwrap().state;
                    let is_paused = match origin {
                        TaskState::Flushing => false,
                        TaskState::PausedFlushing => true,
                        TaskState::Error => {
                            triggering_evt.send_err_ack();
                            continue;
                        }
                        state => {
                            task_inner
                                .lock()
                                .unwrap()
                                .skip_triggering_evt(triggering_evt);
                            gst::trace!(
                                RUNTIME_CAT,
                                obj = self.task_impl.obj(),
                                "Skipped FlushStop in state {state:?}"
                            );
                            continue;
                        }
                    };

                    let res = exec_action!(self, flush_stop, triggering_evt, origin, &task_inner);
                    if let Ok(triggering_evt) = res {
                        if is_paused {
                            task_inner
                                .lock()
                                .unwrap()
                                .switch_to_state(TaskState::Paused, triggering_evt);
                            gst::trace!(
                                RUNTIME_CAT,
                                obj = self.task_impl.obj(),
                                "Switched from PausedFlushing to Paused"
                            );
                        } else {
                            self.start(triggering_evt, origin, &task_inner).await;
                            // next/pending triggering event handled in next iteration
                        }
                    }
                }
                Trigger::Unprepare => {
                    // Unprepare is not joined by an ack_rx but by joining the state machine handle
                    self.task_impl.unprepare().await;

                    task_inner
                        .lock()
                        .unwrap()
                        .switch_to_state(TaskState::Unprepared, triggering_evt);

                    break;
                }
                _ => unreachable!("State machine handler {:?}", triggering_evt),
            }
        }

        gst::trace!(
            RUNTIME_CAT,
            obj = self.task_impl.obj(),
            "Task state machine terminated"
        );
    }

    async fn start(
        &mut self,
        mut triggering_evt: TriggeringEvent,
        origin: TaskState,
        task_inner: &Arc<Mutex<TaskInner>>,
    ) {
        match exec_action!(self, start, triggering_evt, origin, &task_inner) {
            Ok(triggering_evt) => {
                let mut task_inner = task_inner.lock().unwrap();
                task_inner.switch_to_state(TaskState::Started, triggering_evt);
            }
            Err(_) => {
                // error handled by exec_action
                return;
            }
        }

        match self.run_loop().await {
            Ok(()) => (),
            Err(err) => {
                let next_trigger = self.task_impl.handle_loop_error(err).await;
                let (triggering_evt, _) = TriggeringEvent::new(next_trigger);
                self.pending_triggering_evt = Some(triggering_evt);
            }
        }
    }

    async fn run_loop(&mut self) -> Result<(), gst::FlowError> {
        gst::trace!(RUNTIME_CAT, obj = self.task_impl.obj(), "Task loop started");

        let mut try_next_res;
        loop {
            try_next_res = {
                // select_biased requires the selected futures to implement
                // `FusedFuture`. Because async trait functions are not stable,
                // we use `BoxFuture` for the `TaskImpl` function, including
                // `try_next`. Since we need to get a new `BoxFuture` at
                // each iteration, we can guarantee that the future is
                // always valid for use in `select_biased`.
                let mut try_next_fut = pin!(self.task_impl.try_next().fuse());
                futures::select_biased! {
                    triggering_evt = self.triggering_evt_rx.next() => {
                        let triggering_evt = triggering_evt.expect("broken state machine channel");
                        gst::trace!(
                            RUNTIME_CAT,
                            "Task loop handing {:?} to state machine",
                            triggering_evt,
                        );
                        self.pending_triggering_evt = Some(triggering_evt);
                        return Ok(());
                    }
                    try_next_res = try_next_fut => try_next_res,
                }
            };

            let item = try_next_res.inspect_err(|err| {
                gst::debug!(
                    RUNTIME_CAT,
                    obj = self.task_impl.obj(),
                    "TaskImpl::try_next returned {err:?}"
                );
            })?;

            self.task_impl.handle_item(item).await.inspect_err(|&err| {
                gst::debug!(
                    RUNTIME_CAT,
                    obj = self.task_impl.obj(),
                    "TaskImpl::handle_item returned {err:?}"
                );
            })?;
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::channel::{mpsc, oneshot};
    use futures::executor::block_on;
    use futures::prelude::*;
    use gst::glib;
    use gst::glib::prelude::*;
    use std::future::pending;
    use std::time::Duration;

    use super::{
        Task, TaskImpl,
        TaskState::{self, *},
        TransitionError, TransitionOk,
        TransitionOk::*,
        TransitionStatus,
        TransitionStatus::*,
        Trigger::{self, *},
    };
    use crate::runtime::{Context, RUNTIME_CAT};

    impl TransitionStatus {
        // Only useful for unit tests, use `block_on_or_add_sub_task_and_then`
        // or `block_on_or_add_sub_task` in user code.
        fn block_on(self) -> Result<TransitionOk, TransitionError> {
            assert!(!Context::is_context_thread());
            match self {
                Pending {
                    trigger, res_fut, ..
                } => {
                    gst::debug!(
                        RUNTIME_CAT,
                        "Awaiting for {:?} ack on current thread",
                        trigger,
                    );
                    futures::executor::block_on(res_fut)
                }
                Ready(res) => res,
            }
        }
    }

    #[track_caller]
    fn stop_then_unprepare(task: Task) {
        task.stop().block_on().unwrap();
        task.unprepare().block_on().unwrap();
    }

    #[test]
    fn nominal() {
        gst::init().unwrap();

        struct TaskTest {
            obj: gst::Object,
            prepared_sender: mpsc::Sender<()>,
            started_sender: mpsc::Sender<()>,
            try_next_ready_sender: mpsc::Sender<()>,
            try_next_receiver: mpsc::Receiver<()>,
            handle_item_ready_sender: mpsc::Sender<()>,
            handle_item_sender: mpsc::Sender<()>,
            paused_sender: mpsc::Sender<()>,
            stopped_sender: mpsc::Sender<()>,
            unprepared_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "nominal: prepared");
                self.prepared_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "nominal: started");
                self.started_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "nominal: entering try_next");
                self.try_next_ready_sender.send(()).await.unwrap();
                gst::debug!(RUNTIME_CAT, "nominal: awaiting try_next");
                self.try_next_receiver.next().await.unwrap();
                Ok(())
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "nominal: entering handle_item");
                self.handle_item_ready_sender.send(()).await.unwrap();

                gst::debug!(RUNTIME_CAT, "nominal: locked in handle_item");
                self.handle_item_sender.send(()).await.unwrap();
                gst::debug!(RUNTIME_CAT, "nominal: leaving handle_item");

                Ok(())
            }

            async fn pause(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "nominal: paused");
                self.paused_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "nominal: stopped");
                self.stopped_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn unprepare(&mut self) {
                gst::debug!(RUNTIME_CAT, "nominal: unprepared");
                self.unprepared_sender.send(()).await.unwrap();
            }
        }

        let context = Context::acquire("nominal", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        assert_eq!(task.state(), Unprepared);

        gst::debug!(RUNTIME_CAT, "nominal: preparing");

        let (prepared_sender, mut prepared_receiver) = mpsc::channel(1);
        let (started_sender, mut started_receiver) = mpsc::channel(1);
        let (try_next_ready_sender, mut try_next_ready_receiver) = mpsc::channel(1);
        let (mut try_next_sender, try_next_receiver) = mpsc::channel(1);
        let (handle_item_ready_sender, mut handle_item_ready_receiver) = mpsc::channel(1);
        let (handle_item_sender, mut handle_item_receiver) = mpsc::channel(0);
        let (paused_sender, mut paused_receiver) = mpsc::channel(1);
        let (stopped_sender, mut stopped_receiver) = mpsc::channel(1);
        let (unprepared_sender, mut unprepared_receiver) = mpsc::channel(1);
        let obj = gst::Pad::builder(gst::PadDirection::Unknown)
            .name("runtime::Task::nominal")
            .build();
        let prepare_status = task.prepare(
            TaskTest {
                obj: obj.clone().into(),
                prepared_sender,
                started_sender,
                try_next_ready_sender,
                try_next_receiver,
                handle_item_ready_sender,
                handle_item_sender,
                paused_sender,
                stopped_sender,
                unprepared_sender,
            },
            context,
        );

        assert!(prepare_status.is_pending());
        match prepare_status {
            Pending {
                trigger: Prepare,
                origin: Unprepared,
                ..
            } => (),
            other => panic!("{other:?}"),
        };

        gst::debug!(RUNTIME_CAT, "nominal: starting (async prepare)");
        let start_status = task.start().check().unwrap();

        block_on(prepared_receiver.next()).unwrap();
        // also tests await_maybe_on_context
        assert_eq!(
            prepare_status.block_on_or_add_subtask(&obj).unwrap(),
            Complete {
                origin: Unprepared,
                target: Prepared,
            },
        );

        block_on(started_receiver.next()).unwrap();
        assert_eq!(
            start_status.block_on().unwrap(),
            Complete {
                origin: Prepared,
                target: Started,
            }
        );
        assert_eq!(task.state(), Started);

        // unlock task loop and keep looping
        block_on(try_next_ready_receiver.next()).unwrap();
        block_on(try_next_sender.send(())).unwrap();
        block_on(handle_item_ready_receiver.next()).unwrap();
        block_on(handle_item_receiver.next()).unwrap();

        gst::debug!(RUNTIME_CAT, "nominal: starting (redundant)");
        // already started
        assert_eq!(
            task.start().block_on().unwrap(),
            Skipped {
                trigger: Start,
                state: Started,
            },
        );
        assert_eq!(task.state(), Started);

        // Attempt to prepare Task in state Started (also tests check)
        match task.unprepare().check().unwrap_err() {
            TransitionError {
                trigger: Unprepare,
                state: Started,
                ..
            } => (),
            other => panic!("{other:?}"),
        }

        gst::debug!(RUNTIME_CAT, "nominal: pause cancelling try_next");
        block_on(try_next_ready_receiver.next()).unwrap();

        let pause_status = task.pause().check().unwrap();
        gst::debug!(RUNTIME_CAT, "nominal: awaiting pause ack");
        block_on(paused_receiver.next()).unwrap();
        assert_eq!(
            pause_status.block_on().unwrap(),
            Complete {
                origin: Started,
                target: Paused,
            },
        );

        // handle_item not reached
        assert!(handle_item_ready_receiver.try_next().is_err());
        // try_next not reached again
        assert!(try_next_ready_receiver.try_next().is_err());

        gst::debug!(
            RUNTIME_CAT,
            "nominal: starting (after pause cancelling try_next)"
        );
        let start_receiver = task.start().check().unwrap();
        block_on(started_receiver.next()).unwrap();
        assert_eq!(
            start_receiver.block_on().unwrap(),
            Complete {
                origin: Paused,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);

        gst::debug!(RUNTIME_CAT, "nominal: pause // handle_item");
        block_on(try_next_ready_receiver.next()).unwrap();
        block_on(try_next_sender.send(())).unwrap();
        // Make sure item is picked
        block_on(handle_item_ready_receiver.next()).unwrap();

        gst::debug!(RUNTIME_CAT, "nominal: requesting to pause");
        let pause_status = task.pause().check().unwrap();

        gst::debug!(RUNTIME_CAT, "nominal: unlocking item handling");
        block_on(handle_item_receiver.next()).unwrap();

        gst::debug!(RUNTIME_CAT, "nominal: awaiting pause ack");
        block_on(paused_receiver.next()).unwrap();
        assert_eq!(
            pause_status.block_on().unwrap(),
            Complete {
                origin: Started,
                target: Paused,
            },
        );

        // try_next not reached again
        assert!(try_next_ready_receiver.try_next().is_err());

        gst::debug!(
            RUNTIME_CAT,
            "nominal: starting (after pause // handle_item)"
        );
        let start_receiver = task.start().check().unwrap();
        block_on(started_receiver.next()).unwrap();
        assert_eq!(
            start_receiver.block_on().unwrap(),
            Complete {
                origin: Paused,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);

        gst::debug!(RUNTIME_CAT, "nominal: stopping");
        assert_eq!(
            task.stop().block_on().unwrap(),
            Complete {
                origin: Started,
                target: Stopped,
            },
        );

        assert_eq!(task.state(), Stopped);
        let _ = block_on(stopped_receiver.next());

        // purge remaining iteration received before stop if any
        let _ = try_next_ready_receiver.try_next();

        gst::debug!(RUNTIME_CAT, "nominal: starting (after stop)");
        assert_eq!(
            task.start().block_on().unwrap(),
            Complete {
                origin: Stopped,
                target: Started,
            },
        );
        let _ = block_on(started_receiver.next());

        gst::debug!(RUNTIME_CAT, "nominal: stopping");
        assert_eq!(
            task.stop().block_on().unwrap(),
            Complete {
                origin: Started,
                target: Stopped,
            },
        );

        assert_eq!(
            task.unprepare().block_on().unwrap(),
            Complete {
                origin: Stopped,
                target: Unprepared,
            },
        );

        assert_eq!(task.state(), Unprepared);
        let _ = block_on(unprepared_receiver.next());
    }

    #[test]
    fn prepare_error() {
        gst::init().unwrap();

        struct TaskPrepareTest {
            obj: gst::Object,
            prepare_error_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskPrepareTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "prepare_error: prepare returning an error");
                Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["prepare_error: intentional error"]
                ))
            }

            async fn handle_action_error(
                &mut self,
                trigger: Trigger,
                state: TaskState,
                err: gst::ErrorMessage,
            ) -> Trigger {
                gst::debug!(
                    RUNTIME_CAT,
                    "prepare_error: handling prepare error {:?}",
                    err
                );
                match (trigger, state) {
                    (Prepare, Unprepared) => {
                        self.prepare_error_sender.send(()).await.unwrap();
                    }
                    other => unreachable!("{:?}", other),
                }
                Trigger::Error
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                unreachable!("prepare_error: try_next");
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("prepare_error: handle_item");
            }
        }

        let context = Context::acquire("prepare_error", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        assert_eq!(task.state(), Unprepared);

        let (prepare_error_sender, mut prepare_error_receiver) = mpsc::channel(1);
        let prepare_status = task.prepare(
            TaskPrepareTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::prepare_error")
                    .build()
                    .into(),
                prepare_error_sender,
            },
            context,
        );

        gst::debug!(
            RUNTIME_CAT,
            "prepare_error: await action error notification"
        );
        block_on(prepare_error_receiver.next()).unwrap();

        match prepare_status.block_on().unwrap_err() {
            TransitionError {
                trigger: Trigger::Error,
                state: Preparing,
                ..
            } => (),
            other => panic!("{other:?}"),
        }

        // Wait for state machine to reach Error
        while TaskState::Error != task.state() {
            std::thread::sleep(Duration::from_millis(2));
        }

        match task.start().block_on().unwrap_err() {
            TransitionError {
                trigger: Start,
                state: TaskState::Error,
                ..
            } => (),
            other => panic!("{other:?}"),
        }

        block_on(task.unprepare()).unwrap();
    }

    #[test]
    fn prepare_start_ok() {
        // Hold the preparation function so that it completes after the start request is engaged

        gst::init().unwrap();

        struct TaskPrepareTest {
            obj: gst::Object,
            prepare_receiver: mpsc::Receiver<()>,
        }

        impl TaskImpl for TaskPrepareTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(
                    RUNTIME_CAT,
                    "prepare_start_ok: preparation awaiting trigger"
                );
                self.prepare_receiver.next().await.unwrap();
                gst::debug!(RUNTIME_CAT, "prepare_start_ok: preparation complete Ok");
                Ok(())
            }

            async fn handle_action_error(
                &mut self,
                _trigger: Trigger,
                _state: TaskState,
                _err: gst::ErrorMessage,
            ) -> Trigger {
                unreachable!("prepare_start_ok: handle_prepare_error");
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "prepare_start_ok: started");
                Ok(())
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("prepare_start_ok: handle_item");
            }
        }

        let context = Context::acquire("prepare_start_ok", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (mut prepare_sender, prepare_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskPrepareTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::prepare_start_ok")
                    .build()
                    .into(),
                prepare_receiver,
            },
            context,
        );
        drop(fut);

        let start_ctx = Context::acquire("prepare_start_ok_requester", Duration::ZERO).unwrap();
        let (ready_sender, ready_receiver) = oneshot::channel();
        let start_handle = start_ctx.spawn(async move {
            assert_eq!(task.state(), Preparing);
            gst::debug!(RUNTIME_CAT, "prepare_start_ok: starting");
            let start_status = task.start();
            match start_status {
                Pending {
                    trigger: Start,
                    origin: Preparing,
                    ..
                } => (),
                other => panic!("{other:?}"),
            }
            ready_sender.send(()).unwrap();
            assert_eq!(
                start_status.await.unwrap(),
                Complete {
                    origin: Prepared,
                    target: Started,
                },
            );
            assert_eq!(task.state(), Started);

            let stop_status = task.stop();
            match stop_status {
                Pending {
                    trigger: Stop,
                    origin: Started,
                    ..
                } => (),
                other => panic!("{other:?}"),
            }
            assert_eq!(
                stop_status.await.unwrap(),
                Complete {
                    origin: Started,
                    target: Stopped,
                },
            );
            assert_eq!(task.state(), Stopped);

            let unprepare_status = task.unprepare();
            match unprepare_status {
                Pending {
                    trigger: Unprepare,
                    origin: Stopped,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                unprepare_status.await.unwrap(),
                Complete {
                    origin: Stopped,
                    target: Unprepared,
                },
            );
            assert_eq!(task.state(), Unprepared);
        });

        gst::debug!(RUNTIME_CAT, "prepare_start_ok: awaiting for start_ctx");
        block_on(ready_receiver).unwrap();

        gst::debug!(RUNTIME_CAT, "prepare_start_ok: triggering preparation");
        block_on(prepare_sender.send(())).unwrap();

        block_on(start_handle).unwrap();
    }

    #[test]
    fn prepare_start_error() {
        // Hold the preparation function so that it completes after the start request is engaged

        gst::init().unwrap();

        struct TaskPrepareTest {
            obj: gst::Object,
            prepare_receiver: mpsc::Receiver<()>,
            prepare_error_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskPrepareTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(
                    RUNTIME_CAT,
                    "prepare_start_error: preparation awaiting trigger"
                );
                self.prepare_receiver.next().await.unwrap();
                gst::debug!(RUNTIME_CAT, "prepare_start_error: preparation complete Err");

                Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["prepare_start_error: intentional error"]
                ))
            }

            async fn handle_action_error(
                &mut self,
                trigger: Trigger,
                state: TaskState,
                err: gst::ErrorMessage,
            ) -> Trigger {
                gst::debug!(
                    RUNTIME_CAT,
                    "prepare_start_error: handling prepare error {:?}",
                    err
                );
                match (trigger, state) {
                    (Prepare, Unprepared) => {
                        self.prepare_error_sender.send(()).await.unwrap();
                    }
                    other => panic!("action error for {other:?}"),
                }
                Trigger::Error
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                unreachable!("prepare_start_error: start");
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                unreachable!("prepare_start_error: try_next");
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("prepare_start_error: handle_item");
            }
        }

        let context = Context::acquire("prepare_start_error", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (mut prepare_sender, prepare_receiver) = mpsc::channel(1);
        let (prepare_error_sender, mut prepare_error_receiver) = mpsc::channel(1);
        let prepare_status = task.prepare(
            TaskPrepareTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::prepare_start_error")
                    .build()
                    .into(),
                prepare_receiver,
                prepare_error_sender,
            },
            context,
        );
        match prepare_status {
            Pending {
                trigger: Prepare,
                origin: Unprepared,
                ..
            } => (),
            other => panic!("{other:?}"),
        };

        let start_ctx = Context::acquire("prepare_start_error_requester", Duration::ZERO).unwrap();
        let (ready_sender, ready_receiver) = oneshot::channel();
        let start_handle = start_ctx.spawn(async move {
            gst::debug!(RUNTIME_CAT, "prepare_start_error: starting (Err)");
            let fut = task.start();
            drop(fut);
            ready_sender.send(()).unwrap();
            // FIXME we loose the origin Trigger (Start)
            // and only get the Trigger returned by handle_action_error
            // see also: comment in exec_action!
            match prepare_status.await {
                Err(TransitionError {
                    trigger: Trigger::Error,
                    state: Preparing,
                    ..
                }) => (),
                other => panic!("{other:?}"),
            }

            let unprepare_status = task.unprepare();
            match unprepare_status {
                Pending {
                    trigger: Unprepare,
                    origin: TaskState::Error,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                unprepare_status.await.unwrap(),
                Complete {
                    origin: TaskState::Error,
                    target: Unprepared,
                },
            );
        });

        gst::debug!(RUNTIME_CAT, "prepare_start_error: awaiting for start_ctx");
        block_on(ready_receiver).unwrap();

        gst::debug!(
            RUNTIME_CAT,
            "prepare_start_error: triggering preparation (failure)"
        );
        block_on(prepare_sender.send(())).unwrap();

        gst::debug!(
            RUNTIME_CAT,
            "prepare_start_error: await prepare error notification"
        );
        block_on(prepare_error_receiver.next()).unwrap();

        block_on(start_handle).unwrap();
    }

    #[test]
    fn item_error() {
        gst::init().unwrap();

        struct TaskTest {
            obj: gst::Object,
            try_next_receiver: mpsc::Receiver<gst::FlowError>,
        }

        impl TaskImpl for TaskTest {
            type Item = gst::FlowError;

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<gst::FlowError, gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "item_error: awaiting try_next");
                Ok(self.try_next_receiver.next().await.unwrap())
            }

            async fn handle_item(&mut self, item: gst::FlowError) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "item_error: handle_item received {:?}", item);
                Err(item)
            }
        }

        let context = Context::acquire("item_error", Duration::from_millis(2)).unwrap();
        let task = Task::default();
        gst::debug!(RUNTIME_CAT, "item_error: prepare and start");
        let (mut try_next_sender, try_next_receiver) = mpsc::channel(1);
        task.prepare(
            TaskTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::item_error")
                    .build()
                    .into(),
                try_next_receiver,
            },
            context,
        )
        .block_on()
        .unwrap();
        task.start().block_on().unwrap();

        gst::debug!(RUNTIME_CAT, "item_error: req. handle_item to return Eos");
        block_on(try_next_sender.send(gst::FlowError::Eos)).unwrap();
        // Wait for state machine to reach Stopped
        while Stopped != task.state() {
            std::thread::sleep(Duration::from_millis(2));
        }

        gst::debug!(RUNTIME_CAT, "item_error: starting (after stop)");
        assert_eq!(
            task.start().block_on().unwrap(),
            Complete {
                origin: Stopped,
                target: Started,
            },
        );

        gst::debug!(RUNTIME_CAT, "item_error: req. handle_item to return Error");
        block_on(try_next_sender.send(gst::FlowError::Error)).unwrap();
        // Wait for state machine to reach Error
        while TaskState::Error != task.state() {
            std::thread::sleep(Duration::from_millis(2));
        }

        gst::debug!(RUNTIME_CAT, "item_error: attempting to start (after Error)");
        match task.start().block_on().unwrap_err() {
            TransitionError {
                trigger: Start,
                state: TaskState::Error,
                ..
            } => (),
            other => panic!("{other:?}"),
        }

        assert_eq!(
            task.unprepare().block_on().unwrap(),
            Complete {
                origin: TaskState::Error,
                target: Unprepared,
            },
        );
    }

    #[test]
    fn flush_regular_sync() {
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("flush_regular_sync: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_regular_sync: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_regular_sync: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("flush_regular_sync", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let obj = gst::Pad::builder(gst::PadDirection::Unknown)
            .name("runtime::Task::flush_regular_sync")
            .build();
        let fut = task.prepare(
            TaskFlushTest {
                obj: obj.clone().into(),
                flush_start_sender,
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        gst::debug!(RUNTIME_CAT, "flush_regular_sync: start");
        block_on(task.start()).unwrap();

        gst::debug!(RUNTIME_CAT, "flush_regular_sync: starting flush");
        assert_eq!(
            task.flush_start().block_on().unwrap(),
            Complete {
                origin: Started,
                target: Flushing,
            },
        );
        assert_eq!(task.state(), Flushing);

        block_on(flush_start_receiver.next()).unwrap();

        gst::debug!(RUNTIME_CAT, "flush_regular_sync: stopping flush");
        assert_eq!(
            task.flush_stop().block_on_or_add_subtask(&obj).unwrap(),
            Complete {
                origin: Flushing,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);

        block_on(flush_stop_receiver.next()).unwrap();

        let fut = task.pause();
        drop(fut);
        stop_then_unprepare(task);
    }

    #[test]
    fn flush_regular_different_context() {
        // Purpose: make sure a flush sequence triggered from a Context doesn't block.
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("flush_regular_different_context: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(
                    RUNTIME_CAT,
                    "flush_regular_different_context: started flushing"
                );
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(
                    RUNTIME_CAT,
                    "flush_regular_different_context: stopped flushing"
                );
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context =
            Context::acquire("flush_regular_different_context", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let obj = gst::Pad::builder(gst::PadDirection::Unknown)
            .name("runtime::Task::flush_regular_different_context")
            .build();
        let fut = task.prepare(
            TaskFlushTest {
                obj: obj.clone().into(),
                flush_start_sender,
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        gst::debug!(RUNTIME_CAT, "flush_regular_different_context: start");
        task.start().block_on().unwrap();

        let oob_context = Context::acquire(
            "flush_regular_different_context_oob",
            Duration::from_millis(2),
        )
        .unwrap();

        let task_clone = task.clone();
        let flush_res_fut = oob_context.spawn(async move {
            let flush_start_status = task_clone.flush_start();
            match flush_start_status {
                Pending {
                    trigger: FlushStart,
                    origin: Started,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                flush_start_status.await.unwrap(),
                Complete {
                    origin: Started,
                    target: Flushing,
                },
            );
            assert_eq!(task_clone.state(), Flushing);
            flush_start_receiver.next().await.unwrap();

            let flush_stop_status = task_clone.flush_stop();
            match flush_stop_status {
                Pending {
                    trigger: FlushStop,
                    origin: Flushing,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                flush_stop_status.block_on_or_add_subtask(&obj).unwrap(),
                NotWaiting {
                    trigger: FlushStop,
                    origin: Flushing,
                },
            );

            Context::drain_sub_tasks().await.unwrap();
            assert_eq!(task_clone.state(), Started);
        });

        block_on(flush_res_fut).unwrap();
        block_on(flush_stop_receiver.next()).unwrap();

        stop_then_unprepare(task);
    }

    #[test]
    fn flush_regular_same_context() {
        // Purpose: make sure a flush sequence triggered from the same Context doesn't block.
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("flush_regular_same_context: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_regular_same_context: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_regular_same_context: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context =
            Context::acquire("flush_regular_same_context", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskFlushTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::flush_regular_same_context")
                    .build()
                    .into(),
                flush_start_sender,
                flush_stop_sender,
            },
            context.clone(),
        );
        drop(fut);

        block_on(task.start()).unwrap();

        let task_clone = task.clone();
        let flush_handle = context.spawn(async move {
            let flush_start_status = task_clone.flush_start();
            match flush_start_status {
                Pending {
                    trigger: FlushStart,
                    origin: Started,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                flush_start_status.await.unwrap(),
                Complete {
                    origin: Started,
                    target: Flushing,
                },
            );
            assert_eq!(task_clone.state(), Flushing);
            flush_start_receiver.next().await.unwrap();

            let flush_stop_status = task_clone.flush_stop();
            match flush_stop_status {
                Pending {
                    trigger: FlushStop,
                    origin: Flushing,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                flush_stop_status.await.unwrap(),
                Complete {
                    origin: Flushing,
                    target: Started,
                },
            );
            assert_eq!(task_clone.state(), Started);
        });

        block_on(flush_handle).unwrap();
        block_on(flush_stop_receiver.next()).unwrap();

        stop_then_unprepare(task);
    }

    #[test]
    fn flush_from_loop() {
        // Purpose: make sure a flush_start triggered from an iteration doesn't block.
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            task: Task,
            flush_start_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                Ok(())
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "flush_from_loop: flush_start from handle_item");
                match self.task.flush_start() {
                    Pending {
                        trigger: FlushStart,
                        origin: Started,
                        ..
                    } => (),
                    other => panic!("{other:?}"),
                }
                Ok(())
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_from_loop: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("flush_from_loop", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskFlushTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::flush_from_loop")
                    .build()
                    .into(),
                task: task.clone(),
                flush_start_sender,
            },
            context,
        );
        drop(fut);

        let fut = task.start();
        drop(fut);

        gst::debug!(
            RUNTIME_CAT,
            "flush_from_loop: awaiting flush_start notification"
        );
        block_on(flush_start_receiver.next()).unwrap();

        assert_eq!(
            task.stop().block_on().unwrap(),
            Complete {
                origin: Flushing,
                target: Stopped,
            },
        );
        task.unprepare().block_on().unwrap();
    }

    #[test]
    fn pause_from_loop() {
        // Purpose: make sure a start triggered from an iteration doesn't block.
        // E.g. an auto pause cancellation after a delay.
        gst::init().unwrap();

        struct TaskStartTest {
            obj: gst::Object,
            task: Task,
            pause_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskStartTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                Ok(())
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "pause_from_loop: entering handle_item");

                crate::runtime::timer::delay_for(Duration::from_millis(50)).await;

                gst::debug!(RUNTIME_CAT, "pause_from_loop: pause from handle_item");
                match self.task.pause() {
                    Pending {
                        trigger: Pause,
                        origin: Started,
                        ..
                    } => (),
                    other => panic!("{other:?}"),
                }

                Ok(())
            }

            async fn pause(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_from_loop: entering pause action");
                self.pause_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("pause_from_loop", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (pause_sender, mut pause_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskStartTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::pause_from_loop")
                    .build()
                    .into(),
                task: task.clone(),
                pause_sender,
            },
            context,
        );
        drop(fut);

        let fut = task.start();
        drop(fut);

        gst::debug!(RUNTIME_CAT, "pause_from_loop: awaiting pause notification");
        block_on(pause_receiver.next()).unwrap();

        stop_then_unprepare(task);
    }

    #[test]
    fn trigger_from_action() {
        // Purpose: make sure an event triggered from a transition action doesn't block.
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            task: Task,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("trigger_from_action: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(
                    RUNTIME_CAT,
                    "trigger_from_action: flush_start triggering flush_stop"
                );
                match self.task.flush_stop() {
                    Pending {
                        trigger: FlushStop,
                        origin: Started,
                        ..
                    } => (),
                    other => panic!("{other:?}"),
                }

                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "trigger_from_action: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("trigger_from_action", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskFlushTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::trigger_from_action")
                    .build()
                    .into(),
                task: task.clone(),
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        task.start().block_on().unwrap();
        let fut = task.flush_start();
        drop(fut);

        gst::debug!(
            RUNTIME_CAT,
            "trigger_from_action: awaiting flush_stop notification"
        );
        block_on(flush_stop_receiver.next()).unwrap();

        stop_then_unprepare(task);
    }

    #[test]
    fn pause_flush_start() {
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            started_sender: mpsc::Sender<()>,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flush_start: started");
                self.started_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("pause_flush_start: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flush_start: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flush_start: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("pause_flush_start", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (started_sender, mut started_receiver) = mpsc::channel(1);
        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskFlushTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::pause_flush_start")
                    .build()
                    .into(),
                started_sender,
                flush_start_sender,
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        // Pause, FlushStart, FlushStop, Start

        gst::debug!(RUNTIME_CAT, "pause_flush_start: pausing");
        assert_eq!(
            task.pause().block_on().unwrap(),
            Complete {
                origin: Prepared,
                target: Paused,
            },
        );

        gst::debug!(RUNTIME_CAT, "pause_flush_start: starting flush");
        assert_eq!(
            task.flush_start().block_on().unwrap(),
            Complete {
                origin: Paused,
                target: PausedFlushing,
            },
        );
        assert_eq!(task.state(), PausedFlushing);
        block_on(flush_start_receiver.next());

        gst::debug!(RUNTIME_CAT, "pause_flush_start: stopping flush");
        assert_eq!(
            task.flush_stop().block_on().unwrap(),
            Complete {
                origin: PausedFlushing,
                target: Paused,
            },
        );
        assert_eq!(task.state(), Paused);
        block_on(flush_stop_receiver.next());

        // start action not executed
        started_receiver.try_next().unwrap_err();

        gst::debug!(RUNTIME_CAT, "pause_flush_start: starting after flushing");
        assert_eq!(
            task.start().block_on().unwrap(),
            Complete {
                origin: Paused,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);
        block_on(started_receiver.next());

        stop_then_unprepare(task);
    }

    #[test]
    fn pause_flushing_start() {
        gst::init().unwrap();

        struct TaskFlushTest {
            obj: gst::Object,
            started_sender: mpsc::Sender<()>,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskFlushTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flushing_start: started");
                self.started_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("pause_flushing_start: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flushing_start: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "pause_flushing_start: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("pause_flushing_start", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (started_sender, mut started_receiver) = mpsc::channel(1);
        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskFlushTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::pause_flushing_start")
                    .build()
                    .into(),
                started_sender,
                flush_start_sender,
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        // Pause, FlushStart, Start, FlushStop

        gst::debug!(RUNTIME_CAT, "pause_flushing_start: pausing");
        let fut = task.pause();
        drop(fut);

        gst::debug!(RUNTIME_CAT, "pause_flushing_start: starting flush");
        block_on(task.flush_start()).unwrap();
        assert_eq!(task.state(), PausedFlushing);
        block_on(flush_start_receiver.next());

        gst::debug!(RUNTIME_CAT, "pause_flushing_start: starting while flushing");
        assert_eq!(
            task.start().block_on().unwrap(),
            Complete {
                origin: PausedFlushing,
                target: Flushing,
            },
        );
        assert_eq!(task.state(), Flushing);

        // start action not executed
        started_receiver.try_next().unwrap_err();

        gst::debug!(RUNTIME_CAT, "pause_flushing_start: stopping flush");
        assert_eq!(
            task.flush_stop().block_on().unwrap(),
            Complete {
                origin: Flushing,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);
        block_on(flush_stop_receiver.next());
        block_on(started_receiver.next());

        stop_then_unprepare(task);
    }

    #[test]
    fn flush_concurrent_start() {
        // Purpose: check the racy case of start being triggered in // after flush_start
        // e.g.: a FlushStart event received on a Pad and the element starting after a Pause
        gst::init().unwrap();

        struct TaskStartTest {
            obj: gst::Object,
            flush_start_sender: mpsc::Sender<()>,
            flush_stop_sender: mpsc::Sender<()>,
        }

        impl TaskImpl for TaskStartTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                pending().await
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                unreachable!("flush_concurrent_start: handle_item");
            }

            async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_concurrent_start: started flushing");
                self.flush_start_sender.send(()).await.unwrap();
                Ok(())
            }

            async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
                gst::debug!(RUNTIME_CAT, "flush_concurrent_start: stopped flushing");
                self.flush_stop_sender.send(()).await.unwrap();
                Ok(())
            }
        }

        let context = Context::acquire("flush_concurrent_start", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (flush_start_sender, mut flush_start_receiver) = mpsc::channel(1);
        let (flush_stop_sender, mut flush_stop_receiver) = mpsc::channel(1);
        let fut = task.prepare(
            TaskStartTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::flush_concurrent_start")
                    .build()
                    .into(),
                flush_start_sender,
                flush_stop_sender,
            },
            context,
        );
        drop(fut);

        let oob_context =
            Context::acquire("flush_concurrent_start_oob", Duration::from_millis(2)).unwrap();
        let task_clone = task.clone();

        block_on(task.pause()).unwrap();

        // Launch flush_start // start
        let (ready_sender, ready_receiver) = oneshot::channel();
        gst::debug!(RUNTIME_CAT, "flush_concurrent_start: spawning flush_start");
        let flush_start_handle = oob_context.spawn(async move {
            gst::debug!(RUNTIME_CAT, "flush_concurrent_start: // flush_start");
            ready_sender.send(()).unwrap();
            let status = task_clone.flush_start();
            match status {
                Pending {
                    trigger: FlushStart,
                    origin: Paused,
                    ..
                } => (),
                Pending {
                    trigger: FlushStart,
                    origin: Started,
                    ..
                } => (),
                other => panic!("{other:?}"),
            };
            status.await.unwrap();
            flush_start_receiver.next().await.unwrap();
        });

        gst::debug!(
            RUNTIME_CAT,
            "flush_concurrent_start: awaiting for oob_context"
        );
        block_on(ready_receiver).unwrap();

        gst::debug!(RUNTIME_CAT, "flush_concurrent_start: // start");
        match block_on(task.start()) {
            Ok(Complete {
                origin: Paused,
                target: Started,
            }) => (),
            Ok(Complete {
                origin: PausedFlushing,
                target: Flushing,
            }) => (),
            other => panic!("{other:?}"),
        }

        block_on(flush_start_handle).unwrap();

        gst::debug!(RUNTIME_CAT, "flush_concurrent_start: requesting flush_stop");
        assert_eq!(
            task.flush_stop().block_on().unwrap(),
            Complete {
                origin: Flushing,
                target: Started,
            },
        );
        assert_eq!(task.state(), Started);
        block_on(flush_stop_receiver.next());

        stop_then_unprepare(task);
    }

    #[test]
    fn start_timer() {
        use crate::runtime::timer;

        // Purpose: make sure a Timer initialized in a transition is
        // available when iterating in the loop.
        gst::init().unwrap();

        struct TaskTimerTest {
            obj: gst::Object,
            timer: Option<timer::Oneshot>,
            timer_elapsed_sender: Option<oneshot::Sender<()>>,
        }

        impl TaskImpl for TaskTimerTest {
            type Item = ();

            fn obj(&self) -> &impl IsA<glib::Object> {
                &self.obj
            }

            async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
                self.timer = Some(crate::runtime::timer::delay_for(Duration::from_millis(50)));
                gst::debug!(RUNTIME_CAT, "start_timer: started");
                Ok(())
            }

            async fn try_next(&mut self) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "start_timer: awaiting timer");
                self.timer.take().unwrap().await;
                Ok(())
            }

            async fn handle_item(&mut self, _item: ()) -> Result<(), gst::FlowError> {
                gst::debug!(RUNTIME_CAT, "start_timer: timer elapsed");
                if let Some(timer_elapsed_sender) = self.timer_elapsed_sender.take() {
                    timer_elapsed_sender.send(()).unwrap();
                }

                Err(gst::FlowError::Eos)
            }
        }

        let context = Context::acquire("start_timer", Duration::from_millis(2)).unwrap();

        let task = Task::default();

        let (timer_elapsed_sender, timer_elapsed_receiver) = oneshot::channel();
        let fut = task.prepare(
            TaskTimerTest {
                obj: gst::Pad::builder(gst::PadDirection::Unknown)
                    .name("runtime::Task::start_timer")
                    .build()
                    .into(),
                timer: None,
                timer_elapsed_sender: Some(timer_elapsed_sender),
            },
            context,
        );
        drop(fut);

        gst::debug!(RUNTIME_CAT, "start_timer: start");
        let fut = task.start();
        drop(fut);

        block_on(timer_elapsed_receiver).unwrap();
        gst::debug!(RUNTIME_CAT, "start_timer: timer elapsed received");

        stop_then_unprepare(task);
    }
}
