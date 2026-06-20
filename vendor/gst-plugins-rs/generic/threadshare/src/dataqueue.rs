// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::future::{self, abortable, AbortHandle};

use gst::prelude::*;

use std::sync::LazyLock;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use crate::runtime::Context;

static DATA_QUEUE_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-dataqueue",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing queue"),
    )
});

#[derive(Debug, Clone)]
pub enum DataQueueItem {
    Buffer(gst::Buffer),
    BufferList(gst::BufferList),
    Event(gst::Event),
}

impl DataQueueItem {
    fn size(&self) -> (u32, u32) {
        match *self {
            DataQueueItem::Buffer(ref buffer) => (1, buffer.size() as u32),
            DataQueueItem::BufferList(ref list) => (
                list.len() as u32,
                list.iter().map(|b| b.size() as u32).sum::<u32>(),
            ),
            DataQueueItem::Event(_) => (0, 0),
        }
    }

    fn timestamp(&self) -> Option<gst::ClockTime> {
        match *self {
            DataQueueItem::Buffer(ref buffer) => buffer.dts_or_pts(),
            DataQueueItem::BufferList(ref list) => list.iter().find_map(|b| b.dts_or_pts()),
            DataQueueItem::Event(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataQueueState {
    Started,
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PushedStatus {
    FirstBuffer,
    GotBuffers,
    #[default]
    PendingBuffers,
}

impl PushedStatus {
    pub fn is_first_buffer(self) -> bool {
        matches!(self, PushedStatus::FirstBuffer)
    }
}

#[derive(Clone, Debug)]
pub struct DataQueue(Arc<Mutex<DataQueueInner>>);

#[derive(Debug)]
struct DataQueueInner {
    element: gst::Element,
    upstream_ctx: Option<Context>,
    src_pad: gst::Pad,

    state: DataQueueState,
    queue: VecDeque<DataQueueItem>,

    pushed_status: PushedStatus,

    cur_level_buffers: u32,
    cur_level_bytes: u32,
    cur_level_time: gst::ClockTime,
    max_size_buffers: Option<u32>,
    max_size_bytes: Option<u32>,
    max_size_time: Option<gst::ClockTime>,

    pending_handle: Option<AbortHandle>,
}

impl DataQueueInner {
    fn wake(&mut self) {
        if let Some(pending_handle) = self.pending_handle.take() {
            pending_handle.abort();
        }
    }
}

impl DataQueue {
    pub fn builder(element: &gst::Element, src_pad: &gst::Pad) -> DataQueueBuilder {
        DataQueueBuilder::new(element, src_pad)
    }

    pub fn state(&self) -> DataQueueState {
        self.0.lock().unwrap().state
    }

    pub fn upstream_context(&self) -> Option<Context> {
        self.0.lock().unwrap().upstream_ctx.clone()
    }

    pub fn cur_level_buffers(&self) -> u32 {
        self.0.lock().unwrap().cur_level_buffers
    }

    pub fn cur_level_bytes(&self) -> u32 {
        self.0.lock().unwrap().cur_level_bytes
    }

    pub fn cur_level_time(&self) -> gst::ClockTime {
        self.0.lock().unwrap().cur_level_time
    }

    pub fn start(&self) {
        let mut inner = self.0.lock().unwrap();
        if inner.state == DataQueueState::Started {
            gst::debug!(
                DATA_QUEUE_CAT,
                obj = inner.element,
                "Data queue already Started"
            );
            return;
        }
        gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Starting data queue");
        inner.state = DataQueueState::Started;
        inner.wake();
    }

    pub fn stop(&self) {
        let mut inner = self.0.lock().unwrap();
        if inner.state == DataQueueState::Stopped {
            gst::debug!(
                DATA_QUEUE_CAT,
                obj = inner.element,
                "Data queue already Stopped"
            );
            return;
        }
        gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Stopping data queue");
        inner.state = DataQueueState::Stopped;
        inner.wake();
    }

    pub fn clear(&self) {
        let mut inner = self.0.lock().unwrap();

        gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Clearing data queue");

        let src_pad = inner.src_pad.clone();
        for item in inner.queue.drain(..) {
            if let DataQueueItem::Event(event) = item {
                if event.is_sticky()
                    && event.type_() != gst::EventType::Segment
                    && event.type_() != gst::EventType::Eos
                {
                    let _ = src_pad.store_sticky_event(&event);
                }
            }
        }

        inner.pushed_status = PushedStatus::default();

        inner.cur_level_buffers = 0;
        inner.cur_level_bytes = 0;
        inner.cur_level_time = gst::ClockTime::ZERO;

        gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Data queue cleared");
    }

    pub fn push(
        &self,
        obj: &gst::glib::Object,
        item: DataQueueItem,
    ) -> Result<PushedStatus, DataQueueItem> {
        let mut inner = self.0.lock().unwrap();

        if inner.state == DataQueueState::Stopped {
            gst::debug!(
                DATA_QUEUE_CAT,
                obj = obj,
                "Rejecting item {item:?} in state {:?}",
                inner.state
            );
            return Err(item);
        }

        gst::debug!(DATA_QUEUE_CAT, obj = obj, "Pushing item {item:?}");

        let (count, bytes) = item.size();
        let queue_ts = inner.queue.iter().find_map(|i| i.timestamp());
        let ts = item.timestamp();

        if let Some(max) = inner.max_size_buffers {
            if max <= inner.cur_level_buffers {
                gst::debug!(
                    DATA_QUEUE_CAT,
                    obj = obj,
                    "Queue is full (buffers): {max} <= {}",
                    inner.cur_level_buffers
                );
                return Err(item);
            }
        }

        if let Some(max) = inner.max_size_bytes {
            if max <= inner.cur_level_bytes {
                gst::debug!(
                    DATA_QUEUE_CAT,
                    obj = obj,
                    "Queue is full (bytes): {max} <= {}",
                    inner.cur_level_bytes
                );
                return Err(item);
            }
        }

        // FIXME: Use running time
        let level = if let (Some(queue_ts), Some(ts)) = (queue_ts, ts) {
            let level = if queue_ts > ts {
                queue_ts - ts
            } else {
                ts - queue_ts
            };

            if inner.max_size_time.opt_le(level).unwrap_or(false) {
                gst::debug!(
                    DATA_QUEUE_CAT,
                    obj = obj,
                    "Queue is full (time): {} <= {level}",
                    inner.max_size_time.display(),
                );
                return Err(item);
            }

            level
        } else {
            gst::ClockTime::ZERO
        };

        use PushedStatus::*;
        match inner.pushed_status {
            GotBuffers => (),
            FirstBuffer => inner.pushed_status = GotBuffers,
            PendingBuffers => {
                if let DataQueueItem::Buffer(_) | DataQueueItem::BufferList(_) = item {
                    inner.pushed_status = PushedStatus::FirstBuffer;
                    inner.upstream_ctx = Context::current();
                }
            }
        }

        inner.queue.push_back(item);
        inner.cur_level_buffers += count;
        inner.cur_level_bytes += bytes;
        inner.cur_level_time = level;

        inner.wake();

        Ok(inner.pushed_status)
    }

    // TODO: implement as a Stream now that we use a StdMutex
    #[allow(clippy::should_implement_trait)]
    pub async fn next(&mut self) -> Option<DataQueueItem> {
        loop {
            let pending_fut = {
                let mut inner = self.0.lock().unwrap();
                match inner.state {
                    DataQueueState::Started => match inner.queue.pop_front() {
                        None => {
                            gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Data queue is empty");
                        }
                        Some(item) => {
                            gst::debug!(
                                DATA_QUEUE_CAT,
                                obj = inner.element,
                                "Popped item {:?}",
                                item
                            );

                            let (count, bytes) = item.size();
                            inner.cur_level_buffers -= count;
                            inner.cur_level_bytes -= bytes;

                            return Some(item);
                        }
                    },
                    DataQueueState::Stopped => {
                        gst::debug!(DATA_QUEUE_CAT, obj = inner.element, "Data queue Stopped");
                        return None;
                    }
                }

                let (pending_fut, abort_handle) = abortable(future::pending::<()>());
                inner.pending_handle = Some(abort_handle);

                pending_fut
            };

            let _ = pending_fut.await;
        }
    }
}

#[derive(Debug)]
pub struct DataQueueBuilder(DataQueueInner);

impl DataQueueBuilder {
    fn new(element: &gst::Element, src_pad: &gst::Pad) -> DataQueueBuilder {
        DataQueueBuilder(DataQueueInner {
            element: element.clone(),
            upstream_ctx: None,
            src_pad: src_pad.clone(),
            state: DataQueueState::Stopped,
            queue: VecDeque::new(),
            pushed_status: PushedStatus::default(),
            cur_level_buffers: 0,
            cur_level_bytes: 0,
            cur_level_time: gst::ClockTime::ZERO,
            max_size_buffers: None,
            max_size_bytes: None,
            max_size_time: None,
            pending_handle: None,
        })
    }

    pub fn max_size_buffers(mut self, max_size_buffers: u32) -> Self {
        self.0.max_size_buffers = (max_size_buffers > 0).then_some(max_size_buffers);
        self
    }

    pub fn max_size_bytes(mut self, max_size_bytes: u32) -> Self {
        self.0.max_size_bytes = (max_size_bytes > 0).then_some(max_size_bytes);
        self
    }

    pub fn max_size_time(mut self, max_size_time: gst::ClockTime) -> Self {
        self.0.max_size_time = (!max_size_time.is_zero()).then_some(max_size_time);
        self
    }

    pub fn build(self) -> DataQueue {
        DataQueue(Arc::new(Mutex::new(self.0)))
    }
}
