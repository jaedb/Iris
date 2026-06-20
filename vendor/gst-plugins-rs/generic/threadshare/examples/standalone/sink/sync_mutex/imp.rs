// Copyright (C) 2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::EventView;

use std::sync::LazyLock;

use gstthreadshare::runtime::{prelude::*, PadSink};

use std::sync::{Arc, Mutex};

use super::super::{Settings, Stats, CAT};

#[derive(Debug, Default)]
struct PadSinkHandlerInner {
    is_flushing: bool,
    is_main_elem: bool,
    last_ts: Option<gst::ClockTime>,
    segment: Option<gst::FormattedSegment<gst::format::Time>>,
    stats: Option<Box<Stats>>,
}

impl PadSinkHandlerInner {
    fn handle_buffer(
        &mut self,
        elem: &super::DirectSink,
        buffer: gst::Buffer,
    ) -> Result<(), gst::FlowError> {
        if self.is_flushing {
            log_or_trace!(
                CAT,
                self.is_main_elem,
                obj = elem,
                "Discarding {buffer:?} (flushing)"
            );

            return Err(gst::FlowError::Flushing);
        }

        debug_or_trace!(CAT, self.is_main_elem, obj = elem, "Received {buffer:?}");

        if self.is_main_elem {
            let ts = self
                .segment
                .as_ref()
                .expect("Buffer without Time Segment")
                .to_running_time(buffer.dts_or_pts().expect("Buffer without ts"))
                .unwrap();

            if let Some(last_ts) = self.last_ts {
                let rt = elem.current_running_time().unwrap();
                let lateness = rt.nseconds() as i64 - ts.nseconds() as i64;
                let interval = ts.nseconds() as i64 - last_ts.nseconds() as i64;

                if let Some(stats) = self.stats.as_mut() {
                    stats.add_buffer(lateness, interval);
                }

                gst::debug!(CAT, obj = elem, "o lateness {lateness:.2?}");
                gst::debug!(CAT, obj = elem, "o interval {interval:.2?}",);
            }

            self.last_ts = Some(ts);
        }

        log_or_trace!(CAT, self.is_main_elem, obj = elem, "Buffer processed");

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct SyncPadSinkHandler(Arc<Mutex<PadSinkHandlerInner>>);

impl PadSinkHandler for SyncPadSinkHandler {
    type ElementImpl = DirectSink;

    async fn sink_chain(
        self,
        _pad: gst::Pad,
        elem: super::DirectSink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if self.0.lock().unwrap().handle_buffer(&elem, buffer).is_err() {
            return Err(gst::FlowError::Flushing);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    async fn sink_chain_list(
        self,
        _pad: gst::Pad,
        elem: <Self::ElementImpl as ObjectSubclass>::Type,
        buffer_list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        for buffer in buffer_list.iter_owned() {
            if self.0.lock().unwrap().handle_buffer(&elem, buffer).is_err() {
                return Err(gst::FlowError::Flushing);
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    async fn sink_event_serialized(
        self,
        _pad: gst::Pad,
        elem: super::DirectSink,
        event: gst::Event,
    ) -> bool {
        match event.view() {
            EventView::Eos(_) => {
                let is_main_elem = elem.imp().settings.lock().unwrap().is_main_elem;
                if is_main_elem {
                    gst::info!(CAT, obj = elem, "EOS");
                    if let Some(ref mut stats) = self.0.lock().unwrap().stats {
                        stats.log_global();
                    }
                    let _ = elem.post_message(
                        gst::message::Application::builder(gst::Structure::new_empty(
                            "ts-standalone-sink/eos",
                        ))
                        .src(&elem)
                        .build(),
                    );
                }
            }
            EventView::FlushStop(_) => {
                self.0.lock().unwrap().is_flushing = false;
            }
            EventView::Segment(evt) => {
                self.0.lock().unwrap().segment =
                    evt.segment().downcast_ref::<gst::ClockTime>().cloned();
                let _ = elem.post_message(
                    gst::message::Application::builder(gst::Structure::new_empty(
                        "ts-standalone-sink/streaming",
                    ))
                    .src(&elem)
                    .build(),
                );
            }
            EventView::SinkMessage(evt) => {
                let _ = elem.post_message(evt.message());
            }
            _ => (),
        }

        true
    }

    fn sink_event(self, _pad: &gst::Pad, _imp: &DirectSink, event: gst::Event) -> bool {
        if let EventView::FlushStart(..) = event.view() {
            self.0.lock().unwrap().is_flushing = true;
        }

        true
    }
}

impl SyncPadSinkHandler {
    fn prepare(&self, is_main_elem: bool, stats: Option<Stats>) {
        let mut inner = self.0.lock().unwrap();
        inner.is_main_elem = is_main_elem;
        inner.stats = stats.map(Box::new);
    }

    fn start(&self) {
        let mut inner = self.0.lock().unwrap();

        inner.is_flushing = false;
        inner.last_ts = None;

        if let Some(stats) = inner.stats.as_mut() {
            stats.start();
        }
    }

    fn stop(&self) {
        self.0.lock().unwrap().is_flushing = true;
    }
}

#[derive(Debug)]
pub struct DirectSink {
    sink_pad: PadSink,
    sink_pad_handler: SyncPadSinkHandler,
    settings: Mutex<Settings>,
}

impl DirectSink {
    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        debug_or_trace!(CAT, settings.is_main_elem, imp = self, "Preparing");
        let stats = if settings.logs_stats {
            Some(Stats::new(settings.push_period + settings.context_wait / 2))
        } else {
            None
        };

        self.sink_pad_handler.prepare(settings.is_main_elem, stats);
        debug_or_trace!(CAT, settings.is_main_elem, imp = self, "Prepared");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Stopping");
        self.sink_pad_handler.stop();
        debug_or_trace!(CAT, is_main_elem, imp = self, "Stopped");

        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Starting");
        self.sink_pad_handler.start();
        debug_or_trace!(CAT, is_main_elem, imp = self, "Started");

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for DirectSink {
    const NAME: &'static str = "TsStandaloneDirectSink";
    type Type = super::DirectSink;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let sink_pad_handler = SyncPadSinkHandler::default();
        Self {
            sink_pad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                sink_pad_handler.clone(),
            ),
            sink_pad_handler,
            settings: Default::default(),
        }
    }
}

impl ObjectImpl for DirectSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(Settings::properties);
        PROPERTIES.as_ref()
    }

    fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        self.settings.lock().unwrap().set_property(id, value, pspec);
    }

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        self.settings.lock().unwrap().property(id, pspec)
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sink_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SINK);
    }
}

impl GstObjectImpl for DirectSink {}

impl ElementImpl for DirectSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing standalone test direct sink",
                "Sink/Test",
                "Thread-sharing standalone test direct sink",
                "François Laignel <fengalin@free.fr>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        gst::log!(CAT, imp = self, "Got {event:?}");
        self.sink_pad.gst_pad().push_event(event)
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Got {query:?}");

        if !Self::parent_query(self, query) {
            debug_or_trace!(
                CAT,
                is_main_elem,
                imp = self,
                "Upstream didn't process {query:?}"
            );
            return false;
        }

        if query.type_() == gst::QueryType::Latency {
            debug_or_trace!(CAT, is_main_elem, imp = self, "Upstream returned {query:?}");

            let gst::QueryViewMut::Latency(q) = query.view_mut() else {
                unreachable!();
            };

            let (_, min, max) = q.result();

            debug_or_trace!(
                CAT,
                is_main_elem,
                imp = self,
                "Returning latency: live false, min {min}, max {max:?}"
            );
            q.set(false, min, max);
        }

        true
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
            gst::StateChange::ReadyToPaused => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
