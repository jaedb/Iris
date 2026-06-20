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

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{Context, PadSink, PadSinkWeak, PadSrc, PadSrcWeak, Task};

use crate::dataqueue::{DataQueue, DataQueueItem};

static PROXY_CONTEXTS: LazyLock<Mutex<HashMap<String, Weak<Mutex<ProxyContextInner>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PROXY_SRC_PADS: LazyLock<Mutex<HashMap<String, PadSrcWeak>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PROXY_SINK_PADS: LazyLock<Mutex<HashMap<String, PadSinkWeak>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const DEFAULT_PROXY_CONTEXT: &str = "";

const DEFAULT_MAX_SIZE_BUFFERS: u32 = 200;
const DEFAULT_MAX_SIZE_BYTES: u32 = 1024 * 1024;
const DEFAULT_MAX_SIZE_TIME: gst::ClockTime = gst::ClockTime::SECOND;
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

#[derive(Debug, Clone)]
struct SettingsSink {
    proxy_context: String,
}

impl Default for SettingsSink {
    fn default() -> Self {
        SettingsSink {
            proxy_context: DEFAULT_PROXY_CONTEXT.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct SettingsSrc {
    max_size_buffers: u32,
    max_size_bytes: u32,
    max_size_time: gst::ClockTime,
    context: String,
    context_wait: Duration,
    proxy_context: String,
}

impl Default for SettingsSrc {
    fn default() -> Self {
        SettingsSrc {
            max_size_buffers: DEFAULT_MAX_SIZE_BUFFERS,
            max_size_bytes: DEFAULT_MAX_SIZE_BYTES,
            max_size_time: DEFAULT_MAX_SIZE_TIME,
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            proxy_context: DEFAULT_PROXY_CONTEXT.into(),
        }
    }
}

// TODO: Refactor into a Sender and Receiver instead of the have_ booleans

#[derive(Debug, Default)]
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

#[derive(Debug)]
struct ProxyContextInner {
    name: String,
    dataqueue: Option<DataQueue>,
    last_res: Result<gst::FlowSuccess, gst::FlowError>,
    pending_queue: Option<PendingQueue>,
    upstream_latency: Option<gst::ClockTime>,
    sink: Option<super::ProxySink>,
    src: Option<super::ProxySrc>,
}

impl Drop for ProxyContextInner {
    fn drop(&mut self) {
        let mut proxy_ctxs = PROXY_CONTEXTS.lock().unwrap();
        proxy_ctxs.remove(&self.name);
    }
}

#[derive(Debug)]
struct ProxyContextSink {
    shared: Arc<Mutex<ProxyContextInner>>,
    name: String,
}

impl ProxyContextSink {
    #[inline]
    fn lock_shared(&self) -> MutexGuard<'_, ProxyContextInner> {
        self.shared.lock().unwrap()
    }

    fn add(name: &str, sink: &super::ProxySink) -> Option<Self> {
        let mut proxy_ctxs = PROXY_CONTEXTS.lock().unwrap();

        let mut proxy_ctx = None;
        if let Some(shared_weak) = proxy_ctxs.get(name) {
            if let Some(shared) = shared_weak.upgrade() {
                {
                    let shared = shared.lock().unwrap();
                    if shared.sink.is_some() {
                        return None;
                    }
                }

                proxy_ctx = Some(ProxyContextSink {
                    shared,
                    name: name.into(),
                });
            }
        }

        if proxy_ctx.is_none() {
            let shared = Arc::new(Mutex::new(ProxyContextInner {
                name: name.into(),
                dataqueue: None,
                last_res: Err(gst::FlowError::Flushing),
                pending_queue: None,
                upstream_latency: None,
                sink: Some(sink.clone()),
                src: None,
            }));

            proxy_ctxs.insert(name.into(), Arc::downgrade(&shared));

            proxy_ctx = Some(ProxyContextSink {
                shared,
                name: name.into(),
            });
        }

        proxy_ctx
    }
}

impl Drop for ProxyContextSink {
    fn drop(&mut self) {
        let mut shared_ctx = self.lock_shared();
        shared_ctx.sink = None;
        let _ = shared_ctx.pending_queue.take();
    }
}

#[derive(Debug)]
struct ProxyContextSrc {
    shared: Arc<Mutex<ProxyContextInner>>,
    name: String,
}

impl ProxyContextSrc {
    #[inline]
    fn lock_shared(&self) -> MutexGuard<'_, ProxyContextInner> {
        self.shared.lock().unwrap()
    }

    fn add(name: &str, src: &super::ProxySrc) -> Option<Self> {
        let mut proxy_ctxs = PROXY_CONTEXTS.lock().unwrap();

        let mut proxy_ctx = None;
        if let Some(shared_weak) = proxy_ctxs.get(name) {
            if let Some(shared) = shared_weak.upgrade() {
                {
                    let shared = shared.lock().unwrap();
                    if shared.src.is_some() {
                        return None;
                    }
                }

                proxy_ctx = Some(ProxyContextSrc {
                    shared,
                    name: name.into(),
                });
            }
        }

        if proxy_ctx.is_none() {
            let shared = Arc::new(Mutex::new(ProxyContextInner {
                name: name.into(),
                dataqueue: None,
                last_res: Err(gst::FlowError::Flushing),
                pending_queue: None,
                upstream_latency: None,
                sink: None,
                src: Some(src.clone()),
            }));

            proxy_ctxs.insert(name.into(), Arc::downgrade(&shared));

            proxy_ctx = Some(ProxyContextSrc {
                shared,
                name: name.into(),
            });
        }

        proxy_ctx
    }
}

impl Drop for ProxyContextSrc {
    fn drop(&mut self) {
        let mut shared_ctx = self.lock_shared();
        shared_ctx.src = None;
        let _ = shared_ctx.dataqueue.take();
    }
}

#[derive(Clone, Debug)]
struct ProxySinkPadHandler;

impl PadSinkHandler for ProxySinkPadHandler {
    type ElementImpl = ProxySink;

    async fn sink_chain(
        self,
        pad: gst::Pad,
        elem: super::ProxySink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(SINK_CAT, obj = pad, "Handling {:?}", buffer);
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::Buffer(buffer)).await
    }

    async fn sink_chain_list(
        self,
        pad: gst::Pad,
        elem: super::ProxySink,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(SINK_CAT, obj = pad, "Handling {:?}", list);
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::BufferList(list)).await
    }

    fn sink_event(self, pad: &gst::Pad, imp: &ProxySink, event: gst::Event) -> bool {
        gst::debug!(SINK_CAT, obj = pad, "Handling non-serialized {:?}", event);

        let src_pad = {
            let proxy_ctx = imp.proxy_ctx.lock().unwrap();

            PROXY_SRC_PADS
                .lock()
                .unwrap()
                .get(&proxy_ctx.as_ref().unwrap().name)
                .and_then(|src_pad| src_pad.upgrade())
                .map(|src_pad| src_pad.gst_pad().clone())
        };

        if let gst::EventView::FlushStart(..) = event.view() {
            imp.stop();
        }

        if let Some(src_pad) = src_pad {
            gst::log!(SINK_CAT, obj = pad, "Forwarding non-serialized {:?}", event);
            src_pad.push_event(event)
        } else {
            gst::error!(
                SINK_CAT,
                obj = pad,
                "No src pad to forward non-serialized {:?} to",
                event
            );
            true
        }
    }

    async fn sink_event_serialized(
        self,
        pad: gst::Pad,
        elem: super::ProxySink,
        event: gst::Event,
    ) -> bool {
        gst::log!(SINK_CAT, obj = pad, "Handling serialized {:?}", event);

        let imp = elem.imp();

        use gst::EventView;
        match event.view() {
            EventView::Eos(..) => {
                let _ = elem.post_message(gst::message::Eos::builder().src(&elem).build());
            }
            EventView::FlushStop(..) => imp.start(),
            _ => (),
        }

        gst::log!(SINK_CAT, obj = pad, "Queuing serialized {:?}", event);
        imp.enqueue_item(DataQueueItem::Event(event)).await.is_ok()
    }
}

#[derive(Debug)]
pub struct ProxySink {
    sink_pad: PadSink,
    proxy_ctx: Mutex<Option<ProxyContextSink>>,
    settings: Mutex<SettingsSink>,
}

static SINK_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-proxysink",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing proxy sink"),
    )
});

impl ProxySink {
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

    async fn schedule_pending_queue(&self) {
        loop {
            let more_queue_space_receiver = {
                let proxy_ctx = self.proxy_ctx.lock().unwrap();
                let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

                gst::log!(SINK_CAT, imp = self, "Trying to empty pending queue");

                let ProxyContextInner {
                    pending_queue: ref mut pq,
                    ref dataqueue,
                    ..
                } = *shared_ctx;

                if let Some(ref mut pending_queue) = *pq {
                    if let Some(ref dataqueue) = dataqueue {
                        let mut failed_item = None;
                        while let Some(item) = pending_queue.items.pop_front() {
                            if let Err(item) = self.push(dataqueue, item) {
                                failed_item = Some(item);
                                break;
                            }
                        }

                        if let Some(failed_item) = failed_item {
                            pending_queue.items.push_front(failed_item);
                            let (sender, receiver) = oneshot::channel();
                            pending_queue.more_queue_space_sender = Some(sender);

                            receiver
                        } else {
                            gst::log!(SINK_CAT, imp = self, "Pending queue is empty now");
                            *pq = None;
                            return;
                        }
                    } else {
                        let (sender, receiver) = oneshot::channel();
                        pending_queue.more_queue_space_sender = Some(sender);

                        receiver
                    }
                } else {
                    gst::log!(SINK_CAT, imp = self, "Flushing, dropping pending queue");
                    *pq = None;
                    return;
                }
            };

            gst::log!(SINK_CAT, imp = self, "Waiting for more queue space");
            let _ = more_queue_space_receiver.await;
        }
    }

    async fn enqueue_item(&self, item: DataQueueItem) -> Result<gst::FlowSuccess, gst::FlowError> {
        let wait_fut = {
            let proxy_ctx = self.proxy_ctx.lock().unwrap();
            let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

            /* We've taken the lock again, make sure not to recreate
             * a pending queue if tearing down */
            shared_ctx.last_res?;

            let item = {
                let ProxyContextInner {
                    ref mut pending_queue,
                    ref dataqueue,
                    ..
                } = *shared_ctx;

                match (pending_queue, dataqueue) {
                    (None, Some(ref dataqueue)) => self.push(dataqueue, item),
                    (Some(ref mut pending_queue), Some(ref dataqueue)) => {
                        if !pending_queue.scheduled {
                            let mut failed_item = None;
                            while let Some(item) = pending_queue.items.pop_front() {
                                if let Err(item) = self.push(dataqueue, item) {
                                    failed_item = Some(item);
                                    break;
                                }
                            }

                            if let Some(failed_item) = failed_item {
                                pending_queue.items.push_front(failed_item);

                                Err(item)
                            } else {
                                self.push(dataqueue, item)
                            }
                        } else {
                            Err(item)
                        }
                    }
                    _ => Err(item),
                }
            };

            if let Err(item) = item {
                if shared_ctx
                    .pending_queue
                    .as_ref()
                    .map(|pending_queue| !pending_queue.scheduled)
                    .unwrap_or(true)
                {
                    if shared_ctx.pending_queue.is_none() {
                        if shared_ctx.last_res == Err(gst::FlowError::Flushing) {
                            return Err(gst::FlowError::Flushing);
                        }

                        shared_ctx.pending_queue = Some(PendingQueue::default());
                    }

                    let pending_queue = shared_ctx.pending_queue.as_mut().unwrap();

                    let schedule_now = !matches!(
                        item,
                        DataQueueItem::Event(ref ev) if ev.type_() != gst::EventType::Eos,
                    );

                    pending_queue.items.push_back(item);

                    gst::log!(
                        SINK_CAT,
                        imp = self,
                        "Proxy is full - Pushing first item on pending queue"
                    );

                    if schedule_now {
                        gst::log!(SINK_CAT, imp = self, "Scheduling pending queue now");
                        pending_queue.scheduled = true;

                        let wait_fut = self.schedule_pending_queue();
                        Some(wait_fut)
                    } else {
                        gst::log!(SINK_CAT, imp = self, "Scheduling pending queue later");

                        None
                    }
                } else {
                    shared_ctx
                        .pending_queue
                        .as_mut()
                        .unwrap()
                        .items
                        .push_back(item);

                    None
                }
            } else {
                None
            }
        };

        if let Some(wait_fut) = wait_fut {
            gst::log!(SINK_CAT, imp = self, "Blocking until queue has space again");
            wait_fut.await;
        }

        let proxy_ctx = self.proxy_ctx.lock().unwrap();
        let shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
        shared_ctx.last_res
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(SINK_CAT, imp = self, "Preparing");

        let proxy_context = self.settings.lock().unwrap().proxy_context.to_string();

        let proxy_ctx = ProxyContextSink::add(&proxy_context, &self.obj()).ok_or_else(|| {
            gst::error_msg!(
                gst::ResourceError::OpenRead,
                ["Failed to create or get ProxyContext"]
            )
        })?;

        {
            let mut proxy_sink_pads = PROXY_SINK_PADS.lock().unwrap();
            assert!(!proxy_sink_pads.contains_key(&proxy_context));
            proxy_sink_pads.insert(proxy_context, self.sink_pad.downgrade());
        }

        *self.proxy_ctx.lock().unwrap() = Some(proxy_ctx);

        gst::debug!(SINK_CAT, imp = self, "Prepared");

        Ok(())
    }

    fn unprepare(&self) {
        gst::debug!(SINK_CAT, imp = self, "Unpreparing");
        *self.proxy_ctx.lock().unwrap() = None;
        gst::debug!(SINK_CAT, imp = self, "Unprepared");
    }

    fn start(&self) {
        let proxy_ctx = self.proxy_ctx.lock().unwrap();
        let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

        gst::debug!(SINK_CAT, imp = self, "Starting");

        {
            let settings = self.settings.lock().unwrap();
            let mut proxy_sink_pads = PROXY_SINK_PADS.lock().unwrap();
            proxy_sink_pads.remove(&settings.proxy_context);
        }

        shared_ctx.last_res = Ok(gst::FlowSuccess::Ok);

        gst::debug!(SINK_CAT, imp = self, "Started");
    }

    fn stop(&self) {
        let proxy_ctx = self.proxy_ctx.lock().unwrap();
        let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

        gst::debug!(SINK_CAT, imp = self, "Stopping");

        let _ = shared_ctx.pending_queue.take();
        shared_ctx.last_res = Err(gst::FlowError::Flushing);

        gst::debug!(SINK_CAT, imp = self, "Stopped");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ProxySink {
    const NAME: &'static str = "GstTsProxySink";
    type Type = super::ProxySink;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            sink_pad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                ProxySinkPadHandler,
            ),
            proxy_ctx: Mutex::new(None),
            settings: Mutex::new(SettingsSink::default()),
        }
    }
}

impl ObjectImpl for ProxySink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("proxy-context")
                .nick("Proxy Context")
                .blurb("Context name of the proxy to share with")
                .default_value(Some(DEFAULT_PROXY_CONTEXT))
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "proxy-context" => {
                settings.proxy_context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_PROXY_CONTEXT.into());
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "proxy-context" => settings.proxy_context.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sink_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SINK);
    }
}

impl GstObjectImpl for ProxySink {}

impl ElementImpl for ProxySink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing proxy sink",
                "Sink/Generic",
                "Thread-sharing proxy sink",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        gst::log!(SINK_CAT, imp = self, "Got {event:?}");

        if let gst::EventView::Latency(lat_evt) = event.view() {
            let latency = lat_evt.latency();

            let proxy_ctx = self.proxy_ctx.lock().unwrap();
            let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
            shared_ctx.upstream_latency = Some(latency);

            if let Some(src) = shared_ctx.src.as_ref() {
                gst::log!(SINK_CAT, imp = self, "Setting upstream latency {latency}");
                src.imp().set_upstream_latency(latency);
            } else {
                gst::info!(SINK_CAT, imp = self, "No sources to set upstream latency");
            }
        }

        self.sink_pad.gst_pad().push_event(event)
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

            vec![sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(SINK_CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.stop();
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let success = self.parent_change_state(transition)?;

        if transition == gst::StateChange::ReadyToPaused {
            self.start();
        }

        Ok(success)
    }
}

#[derive(Clone, Debug)]
struct ProxySrcPadHandler;

impl PadSrcHandler for ProxySrcPadHandler {
    type ElementImpl = ProxySrc;

    fn src_event(self, pad: &gst::Pad, imp: &ProxySrc, event: gst::Event) -> bool {
        gst::log!(SRC_CAT, obj = pad, "Handling {:?}", event);

        let sink_pad = {
            let proxy_ctx = imp.proxy_ctx.lock().unwrap();

            PROXY_SINK_PADS
                .lock()
                .unwrap()
                .get(&proxy_ctx.as_ref().unwrap().name)
                .and_then(|sink_pad| sink_pad.upgrade())
                .map(|sink_pad| sink_pad.gst_pad().clone())
        };

        use gst::EventView;
        match event.view() {
            EventView::FlushStart(..) => {
                let _ =
                    imp.task
                        .flush_start()
                        .block_on_or_add_subtask_then(imp.obj(), |elem, res| {
                            if let Err(err) = res {
                                gst::error!(SRC_CAT, obj = elem, "FlushStart failed {err:?}");
                                gst::element_error!(
                                    elem,
                                    gst::StreamError::Failed,
                                    ("Internal data stream error"),
                                    ["FlushStart failed {err:?}"]
                                );
                            }
                        });
            }
            EventView::FlushStop(..) => {
                let _ =
                    imp.task
                        .flush_stop()
                        .block_on_or_add_subtask_then(imp.obj(), |elem, res| {
                            if let Err(err) = res {
                                gst::error!(SRC_CAT, obj = elem, "FlushStop failed {err:?}");
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

        if let Some(sink_pad) = sink_pad {
            gst::log!(SRC_CAT, obj = pad, "Forwarding {:?}", event);
            sink_pad.push_event(event)
        } else {
            gst::error!(SRC_CAT, obj = pad, "No sink pad to forward {:?} to", event);
            false
        }
    }

    fn src_query(self, pad: &gst::Pad, proxysrc: &ProxySrc, query: &mut gst::QueryRef) -> bool {
        gst::log!(SRC_CAT, obj = pad, "Handling {:?}", query);

        use gst::QueryViewMut;
        let ret = match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let Some(mut min) = *proxysrc.upstream_latency.lock().unwrap() else {
                    gst::debug!(
                        SRC_CAT,
                        obj = pad,
                        "Upstream latency not available yet, can't handle {query:?}"
                    );
                    return false;
                };

                let upstream_ctx = proxysrc
                    .dataqueue
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|dq| dq.upstream_context());
                let is_same_ts_ctx = upstream_ctx == *proxysrc.ts_ctx.lock().unwrap();

                let settings = proxysrc.settings.lock().unwrap();
                if !is_same_ts_ctx {
                    min += gst::ClockTime::from_nseconds(settings.context_wait.as_nanos() as u64);
                }
                // TODO also use max-size-buffers & CAPS when applicable
                let max = settings.max_size_time;
                drop(settings);

                gst::debug!(
                    SRC_CAT,
                    obj = pad,
                    "Returning latency: live true, min {min}, max {max}"
                );
                q.set(true, min, max);

                true
            }
            QueryViewMut::Scheduling(q) => {
                q.set(gst::SchedulingFlags::SEQUENTIAL, 1, -1, 0);
                q.add_scheduling_modes([gst::PadMode::Push]);
                true
            }
            QueryViewMut::Caps(q) => {
                let caps = if let Some(ref caps) = pad.current_caps() {
                    q.filter()
                        .map(|f| f.intersect_with_mode(caps, gst::CapsIntersectMode::First))
                        .unwrap_or_else(|| caps.clone())
                } else {
                    q.filter()
                        .map(|f| f.to_owned())
                        .unwrap_or_else(gst::Caps::new_any)
                };

                q.set_result(&caps);

                true
            }
            _ => false,
        };

        if ret {
            gst::log!(SRC_CAT, obj = pad, "Handled {:?}", query);
        } else {
            gst::log!(SRC_CAT, obj = pad, "Didn't handle {:?}", query);
        }

        ret
    }
}

#[derive(Debug)]
struct ProxySrcTask {
    element: super::ProxySrc,
    dataqueue: DataQueue,
}

impl ProxySrcTask {
    fn new(element: super::ProxySrc, dataqueue: DataQueue) -> Self {
        ProxySrcTask { element, dataqueue }
    }

    /// Tries to get the upstream latency.
    ///
    /// This is needed when a `ts-proxysrc` joins the `proxy-context` after
    /// the matching `ts-proxysink` has already started streaming.
    async fn maybe_get_upstream_latency(&self) -> Result<(), gst::FlowError> {
        let proxysrc = self.element.imp();

        if proxysrc.upstream_latency.lock().unwrap().is_some() {
            return Ok(());
        }

        gst::log!(SRC_CAT, imp = proxysrc, "Getting upstream latency");

        let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
        let shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

        if let Some(latency) = shared_ctx.upstream_latency {
            proxysrc.set_upstream_latency_priv(latency);
        } else {
            gst::log!(SRC_CAT, imp = proxysrc, "Upstream latency is still unknown");
        }

        Ok(())
    }

    async fn push_item(&mut self, item: DataQueueItem) -> Result<(), gst::FlowError> {
        let proxysrc = self.element.imp();

        {
            let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
            let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

            if let Some(pending_queue) = shared_ctx.pending_queue.as_mut() {
                pending_queue.notify_more_queue_space();
            }
        }

        match item {
            DataQueueItem::Buffer(buffer) => {
                self.maybe_get_upstream_latency().await?;
                gst::log!(SRC_CAT, obj = self.element, "Forwarding {:?}", buffer);
                proxysrc.src_pad.push(buffer).await.map(drop)
            }
            DataQueueItem::BufferList(list) => {
                self.maybe_get_upstream_latency().await?;
                gst::log!(SRC_CAT, obj = self.element, "Forwarding {:?}", list);
                proxysrc.src_pad.push_list(list).await.map(drop)
            }
            DataQueueItem::Event(event) => {
                gst::log!(SRC_CAT, obj = self.element, "Forwarding {:?}", event);

                let is_eos = event.type_() == gst::EventType::Eos;
                proxysrc.src_pad.push_event(event).await;

                if is_eos {
                    return Err(gst::FlowError::Eos);
                }

                Ok(())
            }
        }
    }
}

impl TaskImpl for ProxySrcTask {
    type Item = DataQueueItem;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.element
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(SRC_CAT, obj = self.element, "Starting task");

        let proxysrc = self.element.imp();
        let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
        let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

        if let Some(pending_queue) = shared_ctx.pending_queue.as_mut() {
            pending_queue.notify_more_queue_space();
        }

        self.dataqueue.start();

        shared_ctx.last_res = Ok(gst::FlowSuccess::Ok);

        gst::log!(SRC_CAT, obj = self.element, "Task started");
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
        let proxysrc = self.element.imp();
        match res {
            Ok(()) => {
                gst::log!(SRC_CAT, obj = self.element, "Successfully pushed item");
                let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
                let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
                shared_ctx.last_res = Ok(gst::FlowSuccess::Ok);
            }
            Err(gst::FlowError::Flushing) => {
                gst::debug!(SRC_CAT, obj = self.element, "Flushing");
                let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
                let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
                shared_ctx.last_res = Err(gst::FlowError::Flushing);
            }
            Err(gst::FlowError::Eos) => {
                gst::debug!(SRC_CAT, obj = self.element, "EOS");
                let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
                let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
                shared_ctx.last_res = Err(gst::FlowError::Eos);
            }
            Err(err) => {
                gst::error!(SRC_CAT, obj = self.element, "Got error {}", err);
                gst::element_error!(
                    &self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );
                let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
                let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();
                shared_ctx.last_res = Err(err);
            }
        }

        res
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(SRC_CAT, obj = self.element, "Stopping task");
        self.flush_start().await?;
        gst::log!(SRC_CAT, obj = self.element, "Task stopped");
        Ok(())
    }

    async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(SRC_CAT, obj = self.element, "Task flush start");

        let proxysrc = self.element.imp();
        let proxy_ctx = proxysrc.proxy_ctx.lock().unwrap();
        let mut shared_ctx = proxy_ctx.as_ref().unwrap().lock_shared();

        self.dataqueue.stop();
        self.dataqueue.clear();

        shared_ctx.last_res = Err(gst::FlowError::Flushing);

        if let Some(mut pending_queue) = shared_ctx.pending_queue.take() {
            pending_queue.notify_more_queue_space();
        }

        gst::log!(SRC_CAT, obj = self.element, "Task flush started");
        Ok(())
    }

    async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(SRC_CAT, obj = self.element, "Task flush stop");
        self.start().await?;
        gst::log!(SRC_CAT, obj = self.element, "Task flush stopped");
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProxySrc {
    src_pad: PadSrc,
    task: Task,
    proxy_ctx: Mutex<Option<ProxyContextSrc>>,
    ts_ctx: Mutex<Option<Context>>,
    dataqueue: Mutex<Option<DataQueue>>,
    upstream_latency: Mutex<Option<gst::ClockTime>>,
    settings: Mutex<SettingsSrc>,
}

static SRC_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-proxysrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing proxy source"),
    )
});

impl ProxySrc {
    // Sets the upstream latency without blocking the caller.
    pub fn set_upstream_latency(&self, up_latency: gst::ClockTime) {
        if let Some(ref ts_ctx) = *self.ts_ctx.lock().unwrap() {
            let obj = self.obj().clone();

            gst::log!(SRC_CAT, imp = self, "Setting upstream latency async");
            ts_ctx.spawn(async move {
                obj.imp().set_upstream_latency_priv(up_latency);
            });
        } else {
            gst::debug!(SRC_CAT, imp = self, "Not ready to handle upstream latency");
        }
    }

    // Sets the upstream latency blocking the caller until it's handled.
    fn set_upstream_latency_priv(&self, new_latency: gst::ClockTime) {
        {
            let mut cur_upstream_latency = self.upstream_latency.lock().unwrap();
            if let Some(cur_upstream_latency) = *cur_upstream_latency {
                if cur_upstream_latency == new_latency {
                    return;
                }
            }
            *cur_upstream_latency = Some(new_latency);
        }

        gst::debug!(
            SRC_CAT,
            imp = self,
            "Got new upstream latency {new_latency}"
        );

        self.post_message(gst::message::Latency::builder().src(&*self.obj()).build());
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(SRC_CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap().clone();

        let proxy_ctx =
            ProxyContextSrc::add(&settings.proxy_context, &self.obj()).ok_or_else(|| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to create get shared_state"]
                )
            })?;

        let ts_ctx = Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenRead,
                ["Failed to acquire Context: {}", err]
            )
        })?;
        *self.ts_ctx.lock().unwrap() = Some(ts_ctx.clone());

        let dataqueue = DataQueue::builder(self.obj().upcast_ref(), self.src_pad.gst_pad())
            .max_size_buffers(settings.max_size_buffers)
            .max_size_bytes(settings.max_size_bytes)
            .max_size_time(settings.max_size_time)
            .build();

        {
            let mut shared_ctx = proxy_ctx.lock_shared();
            shared_ctx.dataqueue = Some(dataqueue.clone());

            let mut proxy_src_pads = PROXY_SRC_PADS.lock().unwrap();
            assert!(!proxy_src_pads.contains_key(&settings.proxy_context));
            proxy_src_pads.insert(settings.proxy_context, self.src_pad.downgrade());
        }

        *self.proxy_ctx.lock().unwrap() = Some(proxy_ctx);
        *self.dataqueue.lock().unwrap() = Some(dataqueue.clone());

        self.task
            .prepare(ProxySrcTask::new(self.obj().clone(), dataqueue), ts_ctx)
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(SRC_CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(SRC_CAT, imp = self, "Unpreparing");

        {
            let settings = self.settings.lock().unwrap();
            let mut proxy_src_pads = PROXY_SRC_PADS.lock().unwrap();
            proxy_src_pads.remove(&settings.proxy_context);
        }

        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                let imp = elem.imp();
                *imp.dataqueue.lock().unwrap() = None;
                *imp.proxy_ctx.lock().unwrap() = None;
                *imp.ts_ctx.lock().unwrap() = None;

                gst::debug!(SRC_CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(SRC_CAT, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(SRC_CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(SRC_CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(SRC_CAT, obj = elem, "Started");
                }
            })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ProxySrc {
    const NAME: &'static str = "GstTsProxySrc";
    type Type = super::ProxySrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                ProxySrcPadHandler,
            ),
            task: Task::default(),
            proxy_ctx: Mutex::new(None),
            ts_ctx: Mutex::new(None),
            dataqueue: Mutex::new(None),
            upstream_latency: Mutex::new(None),
            settings: Mutex::new(SettingsSrc::default()),
        }
    }
}

impl ObjectImpl for ProxySrc {
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
                glib::ParamSpecString::builder("proxy-context")
                    .nick("Proxy Context")
                    .blurb("Context name of the proxy to share with")
                    .default_value(Some(DEFAULT_PROXY_CONTEXT))
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
                    .unwrap_or_else(|| "".into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "proxy-context" => {
                settings.proxy_context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_PROXY_CONTEXT.into());
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
            "proxy-context" => self.settings.lock().unwrap().proxy_context.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for ProxySrc {}

impl ElementImpl for ProxySrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing proxy source",
                "Source/Generic",
                "Thread-sharing proxy source",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(SRC_CAT, imp = self, "Changing state {:?}", transition);

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

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToPlaying => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }
}
