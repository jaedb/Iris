// Copyright (C) 2018-2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use async_task::Runnable;
use concurrent_queue::ConcurrentQueue;

use futures::future::BoxFuture;
use futures::prelude::*;

use pin_project_lite::pin_project;

use slab::Slab;

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use super::CallOnDrop;
use crate::runtime::RUNTIME_CAT;

thread_local! {
    static CURRENT_TASK_ID: Cell<Option<TaskId>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct TaskId(pub(super) usize);

impl TaskId {
    pub(super) fn current() -> Option<TaskId> {
        CURRENT_TASK_ID.try_with(Cell::get).ok().flatten()
    }
}

pub type SubTaskOutput = Result<(), gst::FlowError>;

pin_project! {
    pub(super) struct TaskFuture<F: Future> {
        id: TaskId,
        #[pin]
        future: F,
    }

}

impl<F: Future> Future for TaskFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        struct TaskIdGuard {
            prev_task_id: Option<TaskId>,
        }

        impl Drop for TaskIdGuard {
            fn drop(&mut self) {
                let _ = CURRENT_TASK_ID.try_with(|cur| cur.replace(self.prev_task_id.take()));
            }
        }

        let task_id = self.id;
        let project = self.project();

        let _guard = TaskIdGuard {
            prev_task_id: CURRENT_TASK_ID.with(|cur| cur.replace(Some(task_id))),
        };

        project.future.poll(cx)
    }
}

pub(super) struct Task {
    id: TaskId,
    sub_tasks: VecDeque<BoxFuture<'static, SubTaskOutput>>,
}

impl Task {
    fn new(id: TaskId) -> Self {
        Task {
            id,
            sub_tasks: VecDeque::new(),
        }
    }

    fn add_sub_task<T>(&mut self, sub_task: T)
    where
        T: Future<Output = SubTaskOutput> + Send + 'static,
    {
        self.sub_tasks.push_back(sub_task.boxed());
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Task")
            .field("id", &self.id)
            .field("sub_tasks len", &self.sub_tasks.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(super) struct TaskQueue {
    runnables: Arc<ConcurrentQueue<Runnable>>,
    tasks: Arc<Mutex<Slab<Task>>>,
}

impl Default for TaskQueue {
    fn default() -> Self {
        TaskQueue {
            runnables: Arc::new(ConcurrentQueue::unbounded()),
            tasks: Arc::new(Mutex::new(Slab::new())),
        }
    }
}

impl TaskQueue {
    pub fn add<F>(&self, future: F) -> (TaskId, async_task::Task<<F as Future>::Output>)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let tasks_weak = Arc::downgrade(&self.tasks);
        let mut tasks = self.tasks.lock().unwrap();
        let task_id = TaskId(tasks.vacant_entry().key());

        let task_fut = async move {
            gst::trace!(RUNTIME_CAT, "Running {task_id:?}");

            let _guard = CallOnDrop::new(move || {
                if let Some(task) = tasks_weak
                    .upgrade()
                    .and_then(|tasks| tasks.lock().unwrap().try_remove(task_id.0))
                {
                    if !task.sub_tasks.is_empty() {
                        gst::warning!(
                            RUNTIME_CAT,
                            "Task {task_id:?} has {} pending sub tasks",
                            task.sub_tasks.len(),
                        );
                    }
                }

                gst::trace!(RUNTIME_CAT, "Done {task_id:?}",);
            });

            TaskFuture {
                id: task_id,
                future,
            }
            .await
        };

        let runnables = Arc::clone(&self.runnables);
        let (runnable, task) = async_task::spawn(task_fut, move |runnable| {
            runnables.push(runnable).unwrap();
        });
        tasks.insert(Task::new(task_id));
        drop(tasks);

        runnable.schedule();

        (task_id, task)
    }

    /// Adds a task to be blocked on immediately.
    ///
    /// # Safety
    ///
    /// The function and its output must outlive the execution
    /// of the resulting task and the retrieval of the result.
    pub unsafe fn add_sync<F, O>(&self, f: F) -> async_task::Task<O>
    where
        F: FnOnce() -> O + Send,
        O: Send,
    {
        let tasks_clone = Arc::clone(&self.tasks);
        let mut tasks = self.tasks.lock().unwrap();
        let task_id = TaskId(tasks.vacant_entry().key());

        let task_fut = async move {
            gst::trace!(RUNTIME_CAT, "Executing sync function as {task_id:?}");

            let _guard = CallOnDrop::new(move || {
                let _ = tasks_clone.lock().unwrap().try_remove(task_id.0);

                gst::trace!(RUNTIME_CAT, "Done executing sync function as {task_id:?}");
            });

            f()
        };

        let runnables = Arc::clone(&self.runnables);
        // This is the unsafe call for which the lifetime must hold
        // until the the Future is Ready and its Output retrieved.
        let (runnable, task) = async_task::spawn_unchecked(task_fut, move |runnable| {
            runnables.push(runnable).unwrap();
        });
        tasks.insert(Task::new(task_id));
        drop(tasks);

        runnable.schedule();

        task
    }

    pub fn pop_runnable(&self) -> Result<Runnable, concurrent_queue::PopError> {
        self.runnables.pop()
    }

    pub fn add_sub_task<T>(&self, task_id: TaskId, sub_task: T) -> Result<(), T>
    where
        T: Future<Output = SubTaskOutput> + Send + 'static,
    {
        let mut state = self.tasks.lock().unwrap();
        match state.get_mut(task_id.0) {
            Some(task) => {
                gst::trace!(RUNTIME_CAT, "Adding subtask to {task_id:?}");
                task.add_sub_task(sub_task);
                Ok(())
            }
            None => {
                gst::trace!(RUNTIME_CAT, "Task was removed in the meantime");
                Err(sub_task)
            }
        }
    }

    pub async fn drain_sub_tasks(&self, task_id: TaskId) -> SubTaskOutput {
        loop {
            let mut sub_tasks = match self.tasks.lock().unwrap().get_mut(task_id.0) {
                Some(task) if !task.sub_tasks.is_empty() => std::mem::take(&mut task.sub_tasks),
                _ => return Ok(()),
            };

            gst::trace!(
                RUNTIME_CAT,
                "Draining {} sub tasks from {task_id:?}",
                sub_tasks.len(),
            );

            for sub_task in sub_tasks.drain(..) {
                sub_task.await?;
            }
        }
    }
}
