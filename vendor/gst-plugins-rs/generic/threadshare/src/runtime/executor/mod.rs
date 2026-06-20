// Copyright (C) 2018-2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2021 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! The `Executor` for the `threadshare` GStreamer plugins framework.
//!
//! The [`threadshare`]'s `Executor` consists in a set of [`Context`]s. Each [`Context`] is
//! identified by a `name` and runs a loop in a dedicated `thread`. Users can use the [`Context`]
//! to spawn `Future`s. `Future`s are asynchronous processings which allow waiting for resources
//! in a non-blocking way. Examples of non-blocking operations are:
//!
//! * Waiting for an incoming packet on a Socket.
//! * Waiting for an asynchronous `Mutex` `lock` to succeed.
//! * Waiting for a time related `Future`.
//!
//! `Element` implementations should use [`PadSrc`] & [`PadSink`] which provides high-level features.
//!
//! [`threadshare`]: ../../index.html
//! [`PadSrc`]: ../pad/struct.PadSrc.html
//! [`PadSink`]: ../pad/struct.PadSink.html

pub mod async_wrapper;
pub use async_wrapper::Async;

mod context;
pub use context::{block_on, block_on_or_add_subtask, yield_now, Context};

mod join;
pub use join::JoinHandle;

pub mod reactor;
use reactor::{Reactor, Readable, ReadableOwned, Registration, Source, Writable, WritableOwned};

// We need the `Mutex<bool>` to work in pair with `Condvar`.
#[allow(clippy::mutex_atomic)]
mod scheduler;

mod task;
pub use task::{SubTaskOutput, TaskId};

pub mod timer;

struct CallOnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> CallOnDrop<F> {
    fn new(f: F) -> Self {
        CallOnDrop(Some(f))
    }
}

impl<F: FnOnce()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()()
    }
}
