// Copyright (C) 2025 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-ts-intersink
 * @see_also: ts-intersrc, ts-proxysink, ts-proxysrc, intersink, intersrc
 *
 * Thread-sharing sink for inter-pipelines communication.
 *
 * `ts-intersink` is an element that proxies events travelling downstream, non-serialized
 * queries and buffers (including metas) to other pipelines that contains a matching
 * `ts-intersrc` element. The purpose is to allow one to many decoupled pipelines
 * to function as though they were one without having to manually shuttle buffers,
 * events, queries, etc.
 *
 * This element doesn't implement back-pressure like `ts-proxysink` does as we don't
 * want one lagging downstream `ts-proxysrc` to block the others.
 *
 * The `ts-intersink` & `ts-intersrc` elements take advantage of the `threadshare`
 * runtime, reducing the number of threads & context switches which would be
 * necessary with other forms of inter-pipelines elements.
 *
 * ## Usage
 *
 * See document for `ts-intersrc`.
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;
use gst::glib::ControlFlow;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    LazyLock, Mutex,
};

use crate::runtime::executor::block_on_or_add_subtask;
use crate::runtime::prelude::*;
use crate::runtime::PadSink;

use crate::dataqueue::DataQueueItem;

use crate::inter::{InterContext, InterContextWeak, DEFAULT_INTER_CONTEXT, INTER_CONTEXTS};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-intersink",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing inter sink"),
    )
});

#[derive(Debug, Clone)]
struct Settings {
    inter_context: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            inter_context: DEFAULT_INTER_CONTEXT.into(),
        }
    }
}

#[derive(Debug)]
struct InterContextSink {
    shared: InterContext,
    got_first_buffer: bool,
}

impl InterContextSink {
    async fn add(name: String, sinkpad: gst::Pad) -> Option<Self> {
        let mut inter_ctxs = INTER_CONTEXTS.lock().await;

        let shared = if let Some(shared) = inter_ctxs.get(&name).and_then(InterContextWeak::upgrade)
        {
            {
                let mut shared = shared.write().await;
                if shared.sinkpad.is_some() {
                    gst::error!(CAT, "Attempt to set the InterContext sink more than once");
                    return None;
                }
                shared.sinkpad = Some(sinkpad);
            }

            shared
        } else {
            let shared = InterContext::new(&name);
            shared.write().await.sinkpad = Some(sinkpad);
            inter_ctxs.insert(name, shared.downgrade());

            shared
        };

        Some(InterContextSink {
            shared,
            got_first_buffer: false,
        })
    }
}

impl Drop for InterContextSink {
    fn drop(&mut self) {
        let shared = self.shared.clone();
        block_on_or_add_subtask(async move {
            let _ = shared.write().await.sinkpad.take();
        });
    }
}

#[derive(Clone, Debug)]
struct InterSinkPadHandler;

impl PadSinkHandler for InterSinkPadHandler {
    type ElementImpl = InterSink;

    async fn sink_chain(
        self,
        pad: gst::Pad,
        elem: super::InterSink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {buffer:?}");
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::Buffer(buffer)).await
    }

    async fn sink_chain_list(
        self,
        pad: gst::Pad,
        elem: super::InterSink,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {list:?}");
        let imp = elem.imp();
        imp.enqueue_item(DataQueueItem::BufferList(list)).await
    }

    fn sink_event(self, _pad: &gst::Pad, imp: &InterSink, event: gst::Event) -> bool {
        let elem = imp.obj().clone();

        if event.is_downstream() {
            block_on_or_add_subtask(async move {
                let imp = elem.imp();
                gst::debug!(
                    CAT,
                    imp = imp,
                    "Handling non-serialized downstream {event:?}"
                );

                let shared_ctx = imp.shared_ctx();
                let shared_ctx = shared_ctx.read().await;
                if shared_ctx.sources.is_empty() {
                    gst::info!(CAT, imp = imp, "No sources to forward {event:?} to");
                } else {
                    gst::log!(
                        CAT,
                        imp = imp,
                        "Forwarding non-serialized downstream {event:?}"
                    );
                    for (_, source) in shared_ctx.sources.iter() {
                        if imp.stop.load(Ordering::SeqCst) {
                            gst::log!(CAT, imp = imp, "Stop requested");
                            return false;
                        }

                        if !source.send_event(event.clone()) {
                            gst::warning!(
                                CAT,
                                imp = imp,
                                "Failed to forward {event:?} to {}",
                                source.name()
                            );
                        }
                    }
                }

                true
            })
            .unwrap_or(true)
        } else {
            gst::debug!(
                CAT,
                obj = elem,
                "Handling non-serialized upstream {event:?}"
            );

            imp.sinkpad.gst_pad().push_event(event)
        }
    }

    async fn sink_event_serialized(
        self,
        pad: gst::Pad,
        elem: super::InterSink,
        event: gst::Event,
    ) -> bool {
        gst::log!(CAT, obj = pad, "Handling serialized {event:?}");

        let imp = elem.imp();

        if let gst::EventView::Eos(..) = event.view() {
            let _ = elem.post_message(gst::message::Eos::builder().src(&elem).build());
        }

        gst::log!(CAT, obj = pad, "Queuing serialized {:?}", event);
        imp.enqueue_item(DataQueueItem::Event(event)).await.is_ok()
    }
}

#[derive(Debug)]
pub struct InterSink {
    sinkpad: PadSink,
    sink_ctx: Mutex<Option<InterContextSink>>,
    stop: AtomicBool,
    settings: Mutex<Settings>,
}

impl InterSink {
    fn shared_ctx(&self) -> InterContext {
        let local_ctx = self.sink_ctx.lock().unwrap();
        local_ctx.as_ref().expect("set in prepare").shared.clone()
    }

    async fn enqueue_item(&self, item: DataQueueItem) -> Result<gst::FlowSuccess, gst::FlowError> {
        let shared_ctx = {
            let mut local_ctx_grd = self.sink_ctx.lock().unwrap();
            let local_ctx = local_ctx_grd.as_mut().expect("set in prepare");

            if self.stop.load(Ordering::SeqCst) {
                gst::log!(CAT, imp = self, "Stop requested");
                return Err(gst::FlowError::Flushing);
            }

            let shared_ctx = local_ctx.shared.clone();

            if !local_ctx.got_first_buffer
                && matches!(
                    item,
                    DataQueueItem::Buffer(_) | DataQueueItem::BufferList(_)
                )
            {
                local_ctx.got_first_buffer = true;
                drop(local_ctx_grd);

                let _ = self.post_message(gst::message::Latency::new());
            }

            shared_ctx
        };

        let shared_ctx = shared_ctx.read().await;

        for (_, dq) in shared_ctx.dataqueues.iter() {
            if self.stop.load(Ordering::SeqCst) {
                gst::log!(CAT, imp = self, "Stop requested");
                return Err(gst::FlowError::Flushing);
            }

            if dq.push(self.obj().upcast_ref(), item.clone()).is_err() {
                gst::debug!(CAT, imp = self, "Failed to enqueue item: {item:?}");
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let obj = self.obj().clone();
        let sinkpad = self.sinkpad.gst_pad().clone();

        let ctx_name = self.settings.lock().unwrap().inter_context.clone();

        block_on_or_add_subtask(async move {
            let sink_ctx = InterContextSink::add(ctx_name, sinkpad).await;
            if sink_ctx.is_some() {
                let imp = obj.imp();
                *imp.sink_ctx.lock().unwrap() = sink_ctx;
                gst::debug!(CAT, imp = imp, "Prepared");

                Ok(())
            } else {
                Err(gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to add the Sink to InterContext"]
                ))
            }
        })
        .unwrap_or(Ok(()))
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        *self.sink_ctx.lock().unwrap() = None;
        gst::debug!(CAT, imp = self, "Unprepared");
    }

    fn start(&self) {
        gst::debug!(CAT, imp = self, "Started");
        self.stop.store(false, Ordering::SeqCst);
    }

    fn stop(&self) {
        gst::debug!(CAT, imp = self, "Stopping");

        self.stop.store(true, Ordering::SeqCst);

        let mut local_ctx = self.sink_ctx.lock().unwrap();
        let local_ctx = local_ctx.as_mut().expect("set in prepare");
        local_ctx.got_first_buffer = false;

        let shared_ctx = local_ctx.shared.clone();
        let elem = self.obj().clone();
        block_on_or_add_subtask(async move {
            shared_ctx.write().await.upstream_latency = None;
            gst::debug!(CAT, obj = elem, "Stopped");
        });
    }
}

#[glib::object_subclass]
impl ObjectSubclass for InterSink {
    const NAME: &'static str = "GstTsInterSink";
    type Type = super::InterSink;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            sinkpad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                InterSinkPadHandler,
            ),
            sink_ctx: Mutex::new(None),
            stop: AtomicBool::new(true),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for InterSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("inter-context")
                .nick("Inter Context")
                .blurb("Context name of the inter elements to share with")
                .default_value(Some(DEFAULT_INTER_CONTEXT))
                .readwrite()
                .construct_only()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "inter-context" => {
                settings.inter_context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_INTER_CONTEXT.into());
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "inter-context" => settings.inter_context.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sinkpad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SINK);
    }
}

impl GstObjectImpl for InterSink {}

impl ElementImpl for InterSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing inter sink",
                "Sink/Generic",
                "Thread-sharing inter-pipelines sink",
                "François Laignel <francois@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        gst::log!(CAT, imp = self, "Got {event:?}");

        if let gst::EventView::Latency(lat_evt) = event.view() {
            let latency = lat_evt.latency();

            let obj = self.obj().clone();
            let shared_ctx = self.shared_ctx();

            let res = block_on_or_add_subtask(async move {
                let mut shared_ctx = shared_ctx.write().await;
                shared_ctx.upstream_latency = Some(latency);

                if shared_ctx.sources.is_empty() {
                    gst::info!(CAT, obj = obj, "No sources to set upstream latency");
                } else {
                    gst::log!(CAT, obj = obj, "Setting upstream latency {latency}");
                    for (_, src) in shared_ctx.sources.iter() {
                        if obj.imp().stop.load(Ordering::SeqCst) {
                            gst::log!(CAT, obj = obj, "Stop requested");
                            return ControlFlow::Break;
                        }

                        src.imp().set_upstream_latency(latency);
                    }
                }
                ControlFlow::Continue
            })
            .unwrap_or(ControlFlow::Continue);

            if res.is_break() {
                // We are stopping, don't propagate upstream
                return false;
            }
        }

        self.sinkpad.gst_pad().push_event(event)
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
        gst::trace!(CAT, imp = self, "Changing state {transition:?}");

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
