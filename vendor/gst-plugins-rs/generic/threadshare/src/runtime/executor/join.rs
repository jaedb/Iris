// Copyright (C) 2018-2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2021 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use super::TaskId;
use super::{context::Context, scheduler};

#[derive(Debug)]
pub struct JoinError(TaskId);

impl fmt::Display for JoinError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{:?} was cancelled", self.0)
    }
}

impl std::error::Error for JoinError {}

pub struct JoinHandle<T> {
    task: Option<async_task::Task<T>>,
    task_id: TaskId,
    scheduler: scheduler::ThrottlingHandle,
}

unsafe impl<T: Send> Send for JoinHandle<T> {}
unsafe impl<T: Send> Sync for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    pub(super) fn new(
        task_id: TaskId,
        task: async_task::Task<T>,
        scheduler: &scheduler::ThrottlingHandle,
    ) -> Self {
        JoinHandle {
            task: Some(task),
            task_id,
            scheduler: scheduler.clone(),
        }
    }

    pub fn is_current(&self) -> bool {
        if let Some((cur_scheduler, task_id)) =
            scheduler::Throttling::current().zip(TaskId::current())
        {
            cur_scheduler == self.scheduler && task_id == self.task_id
        } else {
            false
        }
    }

    pub fn context(&self) -> Context {
        Context::from(self.scheduler.clone())
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub fn cancel(mut self) {
        let _ = self.task.take().map(|task| task.cancel());
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.as_ref().is_current() {
            panic!("Trying to join task {:?} from itself", self.as_ref());
        }

        if let Some(task) = self.as_mut().task.as_mut() {
            // Unfortunately, we can't detect whether the task has panicked
            // because the `async_task::Task` `Future` implementation
            // `expect`s and we can't `panic::catch_unwind` here because of `&mut cx`.
            // One solution for this would be to use our own `async_task` impl.
            task.poll_unpin(cx).map(Ok)
        } else {
            Poll::Ready(Err(JoinError(self.task_id)))
        }
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.detach();
        }
    }
}

impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("JoinHandle")
            .field("context", &self.scheduler.context_name())
            .field("task_id", &self.task_id)
            .finish()
    }
}
