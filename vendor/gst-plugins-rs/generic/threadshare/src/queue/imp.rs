// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::channel::oneshot;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{Context, PadSink, PadSrc, Task};

use crate::dataqueue::{DataQueue, DataQueueItem};

const DEFAULT_MAX_SIZE_BUFFERS: u32 = 200;
const DEFAULT_MAX_SIZE_BYTES: u32 = 1024 * 1024;
const DEFAULT_MAX_SIZE_TIME: gst::ClockTime = gst::ClockTime::SECOND;
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

#[derive(Debug, Clone)]
struct Settings {
    max_size_buffers: u32,
    max_size_bytes: u32,
    max_size_time: gst::ClockTime,
    context: String,
    context_wait: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_size_buffers: DEFAULT_MAX_SIZE_BUFFERS,
            max_size_bytes: DEFAULT_MAX_SIZE_BYTES,
            max_size_time: DEFAULT_MAX_SIZE_TIME,
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
        }
    }
}

#[derive(Debug)]
struct PendingQueue {
    more_queue_space_sender: Option<oneshot::Sender<()>>,
    scheduled: bool,
    items: VecDeque<DataQueueItem>,
}

impl PendingQueue {
    fn notify_more_queue_space(&mut self) {
        self.more_queue_space_sender.take();
    }
}

#[derive(Clone)]
struct QueuePadSinkHandler;

impl PadSinkHandler for QueuePadSinkHandler {
    type ElementImpl = Queue;

    async fn sink_chain(
        self,
        pad: gst::Pad,
        elem: super::Queue,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {:?}", buffer);
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::Buffer(buffer)).await
    }

    async fn sink_chain_list(
        self,
        pad: gst::Pad,
        elem: super::Queue,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {:?}", list);
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::BufferList(list)).await
    }

    fn sink_event(self, pad: &gst::Pad, imp: &Queue, event: gst::Event) -> bool {
        gst::debug!(CAT, obj = pad, "Handling non-serialized {event:?}");

        if let gst::EventView::FlushStart(..) = event.view() {
            if imp
                .task
                .flush_start()
                .block_on_or_add_subtask_then(imp.obj(), |elem, res| {
                    if let Err(err) = res {
                        gst::error!(CAT, obj = elem, "FlushStart failed {err:?}");
                        gst::element_error!(
                            elem,
                            gst::StreamError::Failed,
                            ("Internal data stream error"),
                            ["FlushStart failed {err:?}"]
                        );
                    }
                })
                .is_err()
            {
                return false;
            }
        }

        gst::log!(CAT, obj = pad, "Forwarding non-serialized {event:?}");
        imp.src_pad.gst_pad().push_event(event)
    }

    async fn sink_event_serialized(
        self,
        pad: gst::Pad,
        elem: super::Queue,
        event: gst::Event,
    ) -> bool {
        gst::log!(CAT, obj = pad, "Handling serialized {:?}", event);

        let imp = elem.imp();

        if let gst::EventView::FlushStop(..) = event.view() {
            if let Err(err) = imp.task.flush_stop().await {
                gst::error!(CAT, obj = pad, "FlushStop failed {:?}", err);
                gst::element_imp_error!(
                    imp,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["FlushStop failed {:?}", err]
                );
            }
        }

        gst::log!(CAT, obj = pad, "Queuing serialized {:?}", event);
        imp.enqueue_item(DataQueueItem::Event(event)).await.is_ok()
    }

    fn sink_query(self, pad: &gst::Pad, imp: &Queue, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", query);

        if query.is_serialized() {
            // FIXME: How can we do this?
            gst::log!(CAT, obj = pad, "Dropping serialized {:?}", query);
            false
        } else {
            gst::log!(CAT, obj = pad, "Forwarding {:?}", query);
            imp.src_pad.gst_pad().peer_query(query)
        }
    }
}

#[derive(Clone, Debug)]
struct QueuePadSrcHandler;

impl PadSrcHandler for QueuePadSrcHandler {
    type ElementImpl = Queue;

    fn src_event(self, pad: &gst::Pad, imp: &Queue, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling {event:?}");

        use gst::EventView;
        match event.view() {
            EventView::FlushStart(..) => {
                let _ =
                    imp.task
                        .flush_start()
                        .block_on_or_add_subtask_then(imp.obj(), |elem, res| {
                            if let Err(err) = res {
                                gst::error!(CAT, obj = elem, "FlushStart failed {err:?}");
                            }
                        });
            }
            EventView::FlushStop(..) => {
                let _ =
                    imp.task
                        .flush_stop()
                        .block_on_or_add_subtask_then(imp.obj(), |elem, res| {
                            if let Err(err) = res {
                                gst::error!(CAT, obj = elem, "FlushStop failed {err:?}");
                                gst::element_error!(
                                    elem,
                                    gst::StreamError::Failed,
                                    ("Internal data stream error"),
                                    ["FlushStop failed {err:?}"]
                                );
                            }
                        });
            }
            _ => (),
        }

        gst::log!(CAT, obj = pad, "Forwarding {event:?}");
        imp.sink_pad.gst_pad().push_event(event)
    }

    fn src_query(self, pad: &gst::Pad, imp: &Queue, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {query:?}");

        use gst::QueryViewMut::*;
        match query.view_mut() {
            Latency(q) => {
                let mut new_query = gst::query::Latency::new();
                let res = imp.sink_pad.gst_pad().peer_query(&mut new_query);
                if !res {
                    return res;
                }

                gst::log!(CAT, obj = pad, "Upstream returned {new_query:?}");

                let (is_live, mut min, max) = new_query.result();

                let upstream_ctx = imp
                    .dataqueue
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|dq| dq.upstream_context());
                let is_same_ts_ctx = upstream_ctx == *imp.ts_ctx.lock().unwrap();

                let settings = imp.settings.lock().unwrap();
                if !is_same_ts_ctx {
                    min += gst::ClockTime::from_nseconds(settings.context_wait.as_nanos() as u64);
                }
                // TODO also use max-size-buffers & CAPS when applicable
                let max = max.unwrap_or(gst::ClockTime::ZERO) + settings.max_size_time;
                drop(settings);

                gst::debug!(
                    CAT,
                    obj = pad,
                    "Returning latency: live {is_live}, min {min}, max {max}"
                );
                q.set(is_live, min, gst::ClockTime::NONE);

                return true;
            }
            Scheduling(q) => {
                let mut new_query = gst::query::Scheduling::new();
                let res = imp.sink_pad.gst_pad().peer_query(&mut new_query);
                if !res {
                    return res;
                }

                gst::log!(CAT, obj = pad, "Upstream returned {new_query:?}");

                let (flags, min, max, align) = new_query.result();
                q.set(flags, min, max, align);
                q.add_scheduling_modes(
                    new_query
                        .scheduling_modes()
                        .filter(|m| m != &gst::PadMode::Pull),
                );
                gst::log!(CAT, obj = pad, "Returning {:?}", q.query_mut());
                return true;
            }
            _ => (),
        }

        gst::log!(CAT, obj = pad, "Forwarding {query:?}");
        imp.sink_pad.gst_pad().peer_query(query)
    }
}

#[derive(Debug)]
struct QueueTask {
    element: super::Queue,
    dataqueue: DataQueue,
}

impl QueueTask {
    fn new(element: super::Queue, dataqueue: DataQueue) -> Self {
        QueueTask { element, dataqueue }
    }

    async fn push_item(&self, item: DataQueueItem) -> Result<(), gst::FlowError> {
        let queue = self.element.imp();

        if let Some(pending_queue) = queue.pending_queue.lock().unwrap().as_mut() {
            pending_queue.notify_more_queue_space();
        }

        match item {
            DataQueueItem::Buffer(buffer) => {
                gst::log!(CAT, obj = self.element, "Forwarding {:?}", buffer);
                queue.src_pad.push(buffer).await.map(drop)
            }
            DataQueueItem::BufferList(list) => {
                gst::log!(CAT, obj = self.element, "Forwarding {:?}", list);
                queue.src_pad.push_list(list).await.map(drop)
            }
            DataQueueItem::Event(event) => {
                gst::log!(CAT, obj = self.element, "Forwarding {:?}", event);
                queue.src_pad.push_event(event).await;
                Ok(())
            }
        }
    }
}

impl TaskImpl for QueueTask {
    type Item = DataQueueItem;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.element
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Starting task");

        let queue = self.element.imp();
        let mut last_res = queue.last_res.lock().unwrap();

        if let Some(pending_queue) = queue.pending_queue.lock().unwrap().as_mut() {
            pending_queue.notify_more_queue_space();
        }

        self.dataqueue.start();

        *last_res = Ok(gst::FlowSuccess::Ok);

        gst::log!(CAT, obj = self.element, "Task started");
        Ok(())
    }

    async fn try_next(&mut self) -> Result<DataQueueItem, gst::FlowError> {
        self.dataqueue
            .next()
            .await
            .ok_or_else(|| panic!("DataQueue stopped while Task is Started"))
    }

    async fn handle_item(&mut self, item: DataQueueItem) -> Result<(), gst::FlowError> {
        let res = self.push_item(item).await;
        let queue = self.element.imp();
        match res {
            Ok(()) => {
                gst::log!(CAT, obj = self.element, "Successfully pushed item");
                *queue.last_res.lock().unwrap() = Ok(gst::FlowSuccess::Ok);
            }
            Err(gst::FlowError::Flushing) => {
                gst::debug!(CAT, obj = self.element, "Flushing");
                *queue.last_res.lock().unwrap() = Err(gst::FlowError::Flushing);
            }
            Err(gst::FlowError::Eos) => {
                gst::debug!(CAT, obj = self.element, "EOS");
                *queue.last_res.lock().unwrap() = Err(gst::FlowError::Eos);
                queue.src_pad.push_event(gst::event::Eos::new()).await;
            }
            Err(err) => {
                gst::error!(CAT, obj = self.element, "Got error {}", err);
                gst::element_error!(
                    &self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );
                *queue.last_res.lock().unwrap() = Err(err);
            }
        }

        res
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task");
        self.flush_start().await?;
        gst::log!(CAT, obj = self.element, "Task stopped");
        Ok(())
    }

    async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Task flush start");

        let queue = self.element.imp();
        let mut last_res = queue.last_res.lock().unwrap();

        self.dataqueue.stop();
        self.dataqueue.clear();

        *last_res = Err(gst::FlowError::Flushing);

        if let Some(mut pending_queue) = queue.pending_queue.lock().unwrap().take() {
            pending_queue.notify_more_queue_space();
        }

        gst::log!(CAT, obj = self.element, "Task flush started");
        Ok(())
    }

    async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Task flush stop");
        self.start().await?;
        gst::log!(CAT, obj = self.element, "Task flush stopped");
        Ok(())
    }
}

#[derive(Debug)]
pub struct Queue {
    sink_pad: PadSink,
    src_pad: PadSrc,
    ts_ctx: Mutex<Option<Context>>,
    task: Task,
    dataqueue: Mutex<Option<DataQueue>>,
    pending_queue: Mutex<Option<PendingQueue>>,
    last_res: Mutex<Result<gst::FlowSuccess, gst::FlowError>>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-queue",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing queue"),
    )
});

impl Queue {
    fn push(&self, dataqueue: &DataQueue, item: DataQueueItem) -> Result<(), DataQueueItem> {
        if dataqueue
            .push(self.obj().upcast_ref(), item)?
            .is_first_buffer()
        {
            let elem = self.obj();
            let _ = elem.post_message(gst::message::Latency::builder().src(&*elem).build());
        }

        Ok(())
    }

    /* Try transferring all the items from the pending queue to the DataQueue, then
     * the current item. Errors out if the DataQueue was full, or the pending queue
     * is already scheduled, in which case the current item should be added to the
     * pending queue */
    fn queue_until_full(
        &self,
        dataqueue: &DataQueue,
        pending_queue: &mut Option<PendingQueue>,
        item: DataQueueItem,
    ) -> Result<(), DataQueueItem> {
        match pending_queue {
            None => self.push(dataqueue, item),
            Some(PendingQueue {
                scheduled: false,
                ref mut items,
                ..
            }) => {
                let mut failed_item = None;
                while let Some(item) = items.pop_front() {
                    if let Err(item) = self.push(dataqueue, item) {
                        failed_item = Some(item);
                    }
                }

                if let Some(failed_item) = failed_item {
                    items.push_front(failed_item);

                    Err(item)
                } else {
                    self.push(dataqueue, item)
                }
            }
            _ => Err(item),
        }
    }

    /* Schedules emptying of the pending queue. If there is an upstream
     * TaskContext, the new task is spawned, it is otherwise
     * returned, for the caller to block on */
    async fn schedule_pending_queue(&self) {
        loop {
            let more_queue_space_receiver = {
                let dataqueue = self.dataqueue.lock().unwrap();
                if dataqueue.is_none() {
                    return;
                }
                let mut pending_queue_grd = self.pending_queue.lock().unwrap();

                gst::log!(CAT, imp = self, "Trying to empty pending queue");

                if let Some(pending_queue) = pending_queue_grd.as_mut() {
                    let mut failed_item = None;
                    while let Some(item) = pending_queue.items.pop_front() {
                        if let Err(item) = self.push(dataqueue.as_ref().unwrap(), item) {
                            failed_item = Some(item);
                        }
                    }

                    if let Some(failed_item) = failed_item {
                        pending_queue.items.push_front(failed_item);
                        let (sender, receiver) = oneshot::channel();
                        pending_queue.more_queue_space_sender = Some(sender);

                        receiver
                    } else {
                        gst::log!(CAT, imp = self, "Pending queue is empty now");
                        *pending_queue_grd = None;
                        return;
                    }
                } else {
                    gst::log!(CAT, imp = self, "Flushing, dropping pending queue");
                    return;
                }
            };

            gst::log!(CAT, imp = self, "Waiting for more queue space");
            let _ = more_queue_space_receiver.await;
        }
    }

    async fn enqueue_item(&self, item: DataQueueItem) -> Result<gst::FlowSuccess, gst::FlowError> {
        let wait_fut = {
            let dataqueue = self.dataqueue.lock().unwrap();
            let dataqueue = dataqueue.as_ref().ok_or_else(|| {
                gst::error!(CAT, imp = self, "No DataQueue");
                gst::FlowError::Error
            })?;

            let mut pending_queue = self.pending_queue.lock().unwrap();

            if let Err(item) = self.queue_until_full(dataqueue, &mut pending_queue, item) {
                if pending_queue
                    .as_ref()
                    .map(|pq| !pq.scheduled)
                    .unwrap_or(true)
                {
                    if pending_queue.is_none() {
                        // Lock order: last_res, then pending_queue
                        drop(pending_queue);
                        if *self.last_res.lock().unwrap() == Err(gst::FlowError::Flushing) {
                            return Err(gst::FlowError::Flushing);
                        }
                        pending_queue = self.pending_queue.lock().unwrap();

                        *pending_queue = Some(PendingQueue {
                            more_queue_space_sender: None,
                            scheduled: false,
                            items: VecDeque::new(),
                        });
                    }

                    let schedule_now = !matches!(
                        item,
                        DataQueueItem::Event(ref ev) if ev.type_() != gst::EventType::Eos,
                    );

                    pending_queue.as_mut().unwrap().items.push_back(item);

                    gst::log!(
                        CAT,
                        imp = self,
                        "Queue is full - Pushing first item on pending queue"
                    );

                    if schedule_now {
                        gst::log!(CAT, imp = self, "Scheduling pending queue now");
                        pending_queue.as_mut().unwrap().scheduled = true;

                        let wait_fut = self.schedule_pending_queue();
                        Some(wait_fut)
                    } else {
                        gst::log!(CAT, imp = self, "Scheduling pending queue later");
                        None
                    }
                } else {
                    pending_queue.as_mut().unwrap().items.push_back(item);
                    None
                }
            } else {
                None
            }
        };

        if let Some(wait_fut) = wait_fut {
            gst::log!(CAT, imp = self, "Blocking until queue has space again");
            wait_fut.await;
        }

        *self.last_res.lock().unwrap()
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap().clone();

        let dataqueue = DataQueue::builder(self.obj().upcast_ref(), self.src_pad.gst_pad())
            .max_size_buffers(settings.max_size_buffers)
            .max_size_bytes(settings.max_size_bytes)
            .max_size_time(settings.max_size_time)
            .build();

        let context =
            Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;

        *self.ts_ctx.lock().unwrap() = Some(context.clone());
        *self.dataqueue.lock().unwrap() = Some(dataqueue.clone());

        self.task
            .prepare(QueueTask::new(self.obj().clone(), dataqueue), context)
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                let imp = elem.imp();
                *imp.dataqueue.lock().unwrap() = None;
                *imp.pending_queue.lock().unwrap() = None;
                *imp.ts_ctx.lock().unwrap() = None;

                *imp.last_res.lock().unwrap() = Ok(gst::FlowSuccess::Ok);

                gst::debug!(CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Started");
                }
            })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Queue {
    const NAME: &'static str = "GstTsQueue";
    type Type = super::Queue;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            sink_pad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                QueuePadSinkHandler,
            ),
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                QueuePadSrcHandler,
            ),
            ts_ctx: Mutex::new(None),
            task: Task::default(),
            dataqueue: Mutex::new(None),
            pending_queue: Mutex::new(None),
            last_res: Mutex::new(Ok(gst::FlowSuccess::Ok)),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for Queue {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .build(),
                glib::ParamSpecUInt::builder("max-size-buffers")
                    .nick("Max Size Buffers")
                    .blurb("Maximum number of buffers to queue (0=unlimited)")
                    .default_value(DEFAULT_MAX_SIZE_BUFFERS)
                    .build(),
                glib::ParamSpecUInt::builder("max-size-bytes")
                    .nick("Max Size Bytes")
                    .blurb("Maximum number of bytes to queue (0=unlimited)")
                    .default_value(DEFAULT_MAX_SIZE_BYTES)
                    .build(),
                glib::ParamSpecUInt64::builder("max-size-time")
                    .nick("Max Size Time")
                    .blurb("Maximum number of nanoseconds to queue (0=unlimited)")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_MAX_SIZE_TIME.nseconds())
                    .build(),
                glib::ParamSpecUInt::builder("current-level-buffers")
                    .nick("Current Level Buffers")
                    .blurb("Current number of buffers in the queue")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("current-level-bytes")
                    .nick("Current Level Bytes")
                    .blurb("Current amount of data in the queue (bytes)")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt64::builder("current-level-time")
                    .nick("Current Level Time")
                    .blurb("Current amount of data in the queue (in ns)")
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "max-size-buffers" => {
                settings.max_size_buffers = value.get().expect("type checked upstream");
            }
            "max-size-bytes" => {
                settings.max_size_bytes = value.get().expect("type checked upstream");
            }
            "max-size-time" => {
                settings.max_size_time = value.get::<u64>().unwrap().nseconds();
            }
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "max-size-buffers" => self.settings.lock().unwrap().max_size_buffers.to_value(),
            "max-size-bytes" => self.settings.lock().unwrap().max_size_bytes.to_value(),
            "max-size-time" => self
                .settings
                .lock()
                .unwrap()
                .max_size_time
                .nseconds()
                .to_value(),
            "current-level-buffers" => self
                .dataqueue
                .lock()
                .unwrap()
                .as_ref()
                .map_or(0, |d| d.cur_level_buffers())
                .to_value(),
            "current-level-bytes" => self
                .dataqueue
                .lock()
                .unwrap()
                .as_ref()
                .map_or(0, |d| d.cur_level_bytes())
                .to_value(),
            "current-level-time" => self
                .dataqueue
                .lock()
                .unwrap()
                .as_ref()
                .map_or(gst::ClockTime::ZERO, |d| d.cur_level_time())
                .nseconds()
                .to_value(),
            "context" => self.settings.lock().unwrap().context.to_value(),
            "context-wait" => {
                (self.settings.lock().unwrap().context_wait.as_millis() as u32).to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sink_pad.gst_pad()).unwrap();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
    }
}

impl GstObjectImpl for Queue {}

impl ElementImpl for Queue {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing queue",
                "Generic",
                "Simple data queue",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let success = self.parent_change_state(transition)?;

        if transition == gst::StateChange::ReadyToPaused {
            self.start().map_err(|_| gst::StateChangeError)?;
        }

        Ok(success)
    }
}
