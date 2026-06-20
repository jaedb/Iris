// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! A `runtime` for the `threadshare` GStreamer plugins framework.
//!
//! Many `GStreamer` `Element`s internally spawn OS `thread`s. For most applications, this is not an
//! issue. However, in applications which process many `Stream`s in parallel, the high number of
//! `threads` leads to reduced efficiency due to:
//!
//! * context switches,
//! * scheduler overhead,
//! * most of the threads waiting for some resources to be available.
//!
//! The `threadshare` `runtime` is a framework to build `Element`s for such applications. It
//! uses light-weight threading to allow multiple `Element`s share a reduced number of OS `thread`s.
//!
//! See this [talk] ([slides]) for a presentation of the motivations and principles,
//! and this [blog post].
//!
//! Current implementation uses a custom executor mostly based on the [`smol`] ecosystem.
//!
//! Most `Element`s implementations should use the high-level features provided by [`PadSrc`] &
//! [`PadSink`].
//!
//! [talk]: https://gstconf.ubicast.tv/videos/when-adding-more-threads-adds-more-problems-thread-sharing-between-elements-in-gstreamer/
//! [slides]: https://gstreamer.freedesktop.org/data/events/gstreamer-conference/2018/Sebastian%20Dr%C3%B6ge%20-%20When%20adding%20more%20threads%20adds%20more%20problems:%20Thread-sharing%20between%20elements%20in%20GStreamer.pdf
//! [blog post]: https://coaxion.net/blog/2018/04/improving-gstreamer-performance-on-a-high-number-of-network-streams-by-sharing-threads-between-elements-with-rusts-tokio-crate
//! [`smol`]: https://github.com/smol-rs/
//! [`PadSrc`]: pad/struct.PadSrc.html
//! [`PadSink`]: pad/struct.PadSink.html

pub mod executor;
pub use executor::{timer, Async, Context, JoinHandle, SubTaskOutput};

pub mod pad;
pub use pad::{PadSink, PadSinkRef, PadSinkWeak, PadSrc, PadSrcRef, PadSrcWeak};

pub mod task;
pub use task::{Task, TaskState};

pub mod prelude {
    pub use super::pad::{PadSinkHandler, PadSrcHandler};
    pub use super::task::TaskImpl;
}

use std::sync::LazyLock;

static RUNTIME_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-runtime",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing Runtime"),
    )
});
