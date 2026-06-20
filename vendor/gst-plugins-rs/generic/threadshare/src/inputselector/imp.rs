// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::future::{abortable, AbortHandle};
use futures::prelude::*;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{self, PadSink, PadSrc};

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

#[derive(Debug, Clone)]
struct Settings {
    context: String,
    context_wait: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
        }
    }
}

#[derive(Debug)]
struct InputSelectorPadSinkHandlerInner {
    segment: Option<gst::Segment>,
    send_sticky: bool,
    abort_handle: Option<AbortHandle>,
}

impl Default for InputSelectorPadSinkHandlerInner {
    fn default() -> Self {
        InputSelectorPadSinkHandlerInner {
            segment: None,
            send_sticky: true,
            abort_handle: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct InputSelectorPadSinkHandler(Arc<Mutex<InputSelectorPadSinkHandlerInner>>);

impl InputSelectorPadSinkHandler {
    /* Wait until specified time */
    async fn sync(&self, elem: &super::InputSelector, running_time: Option<gst::ClockTime>) {
        let now = elem.current_running_time();

        match running_time.opt_checked_sub(now) {
            Ok(Some(delay)) => {
                runtime::timer::delay_for(delay.into()).await;
            }
            _ => runtime::executor::yield_now().await,
        }
    }

    async fn handle_item(
        &self,
        pad: &gst::Pad,
        elem: &super::InputSelector,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let inputselector = elem.imp();

        let (stickies, is_active, sync_future, switched_pad) = {
            let mut state = inputselector.state.lock().unwrap();
            let mut inner = self.0.lock().unwrap();
            let mut stickies = vec![];
            let mut sync_future = None;
            let switched_pad = state.switched_pad;

            if let Some(segment) = &inner.segment {
                if let Some(segment) = segment.downcast_ref::<gst::format::Time>() {
                    let rtime = segment.to_running_time(buffer.pts());
                    let (sync_fut, abort_handle) = abortable(self.sync(elem, rtime));
                    inner.abort_handle = Some(abort_handle);
                    sync_future = Some(sync_fut.map_err(|_| gst::FlowError::Flushing));
                }
            }

            let is_active = {
                if state.active_sinkpad.as_ref() == Some(pad) {
                    if inner.send_sticky || state.switched_pad {
                        pad.sticky_events_foreach(|event| {
                            use std::ops::ControlFlow;
                            stickies.push(event.clone());
                            ControlFlow::Continue(gst::EventForeachAction::Keep)
                        });

                        inner.send_sticky = false;
                        state.switched_pad = false;
                    }
                    true
                } else {
                    false
                }
            };

            (stickies, is_active, sync_future, switched_pad)
        };

        if let Some(sync_fut) = sync_future {
            sync_fut.await?;
        }

        for event in stickies {
            inputselector.src_pad.push_event(event).await;
        }

        if is_active {
            gst::log!(CAT, obj = pad, "Forwarding {:?}", buffer);

            if switched_pad && !buffer.flags().contains(gst::BufferFlags::DISCONT) {
                let buffer = buffer.make_mut();
                buffer.set_flags(gst::BufferFlags::DISCONT);
            }

            inputselector.src_pad.push(buffer).await
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }
}

impl PadSinkHandler for InputSelectorPadSinkHandler {
    type ElementImpl = InputSelector;

    async fn sink_chain(
        self,
        pad: gst::Pad,
        elem: super::InputSelector,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.handle_item(&pad, &elem, buffer).await
    }

    async fn sink_chain_list(
        self,
        pad: gst::Pad,
        elem: super::InputSelector,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer list {:?}", list);
        // TODO: Ideally we would keep the list intact and forward it in one go
        for buffer in list.iter_owned() {
            self.handle_item(&pad, &elem, buffer).await?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    async fn sink_event_serialized(
        self,
        _pad: gst::Pad,
        _elem: super::InputSelector,
        event: gst::Event,
    ) -> bool {
        let mut inner = self.0.lock().unwrap();

        // Remember the segment for later use
        if let gst::EventView::Segment(e) = event.view() {
            inner.segment = Some(e.segment().clone());
        }

        // We sent sticky events together with the next buffer once it becomes
        // the active pad.
        //
        // TODO: Other serialized events for the active pad can also be forwarded
        // here, and sticky events could be forwarded directly. Needs forwarding of
        // all other sticky events first!
        if event.is_sticky() {
            inner.send_sticky = true;
            true
        } else {
            true
        }
    }

    fn sink_event(self, _pad: &gst::Pad, imp: &InputSelector, event: gst::Event) -> bool {
        /* Drop all events for now */
        if let gst::EventView::FlushStart(..) = event.view() {
            /* Unblock downstream */
            imp.src_pad.gst_pad().push_event(event.clone());

            let mut inner = self.0.lock().unwrap();

            if let Some(abort_handle) = inner.abort_handle.take() {
                abort_handle.abort();
            }
        }
        true
    }

    fn sink_query(self, pad: &gst::Pad, imp: &InputSelector, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        if query.is_serialized() {
            // FIXME: How can we do this (drops ALLOCATION and DRAIN)?
            gst::log!(CAT, obj = pad, "Dropping serialized query {:?}", query);
            false
        } else {
            gst::log!(CAT, obj = pad, "Forwarding query {:?}", query);
            imp.src_pad.gst_pad().peer_query(query)
        }
    }
}

#[derive(Clone, Debug)]
struct InputSelectorPadSrcHandler;

impl PadSrcHandler for InputSelectorPadSrcHandler {
    type ElementImpl = InputSelector;

    fn src_query(self, pad: &gst::Pad, imp: &InputSelector, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", query);

        use gst::QueryViewMut;
        match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let mut ret = true;
                let mut min_latency = gst::ClockTime::ZERO;
                let mut max_latency = gst::ClockTime::NONE;
                let pads = {
                    let pads = imp.pads.lock().unwrap();
                    pads.sink_pads
                        .iter()
                        .map(|p| p.0.clone())
                        .collect::<Vec<_>>()
                };

                for pad in pads {
                    let mut peer_query = gst::query::Latency::new();

                    ret = pad.peer_query(&mut peer_query);

                    if ret {
                        let (live, min, max) = peer_query.result();
                        if live {
                            min_latency = min.max(min_latency);
                            max_latency = max.opt_min(max_latency).or(max);
                        }
                    }
                }

                q.set(true, min_latency, max_latency);

                ret
            }
            _ => {
                let sinkpad = {
                    let state = imp.state.lock().unwrap();
                    state.active_sinkpad.clone()
                };

                if let Some(sinkpad) = sinkpad {
                    sinkpad.peer_query(query)
                } else {
                    true
                }
            }
        }
    }
}

#[derive(Debug)]
struct State {
    active_sinkpad: Option<gst::Pad>,
    switched_pad: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            active_sinkpad: None,
            switched_pad: true,
        }
    }
}

#[derive(Debug, Default)]
struct Pads {
    pad_serial: u32,
    sink_pads: HashMap<gst::Pad, PadSink>,
}

#[derive(Debug)]
pub struct InputSelector {
    src_pad: PadSrc,
    state: Mutex<State>,
    settings: Mutex<Settings>,
    pads: Mutex<Pads>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-input-selector",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing input selector"),
    )
});

impl InputSelector {
    fn unprepare(&self) {
        let mut state = self.state.lock().unwrap();
        gst::debug!(CAT, imp = self, "Unpreparing");
        *state = State::default();
        gst::debug!(CAT, imp = self, "Unprepared");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for InputSelector {
    const NAME: &'static str = "GstTsInputSelector";
    type Type = super::InputSelector;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                InputSelectorPadSrcHandler,
            ),
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
            pads: Mutex::new(Pads::default()),
        }
    }
}

impl ObjectImpl for InputSelector {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .readwrite()
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .readwrite()
                    .build(),
                glib::ParamSpecObject::builder::<gst::Pad>("active-pad")
                    .nick("Active Pad")
                    .blurb("Currently active pad")
                    .readwrite()
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "context" => {
                let mut settings = self.settings.lock().unwrap();
                settings.context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                let mut settings = self.settings.lock().unwrap();
                settings.context_wait = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "active-pad" => {
                let pad = value
                    .get::<Option<gst::Pad>>()
                    .expect("type checked upstream");
                let mut state = self.state.lock().unwrap();
                let pads = self.pads.lock().unwrap();
                let mut old_pad = None;
                if let Some(ref pad) = pad {
                    if pads.sink_pads.contains_key(pad) {
                        old_pad.clone_from(&state.active_sinkpad);
                        state.active_sinkpad = Some(pad.clone());
                        state.switched_pad = true;
                    }
                } else {
                    state.active_sinkpad = None;
                }

                drop(pads);
                drop(state);

                if let Some(old_pad) = old_pad {
                    if Some(&old_pad) != pad.as_ref() {
                        let _ = old_pad.push_event(gst::event::Reconfigure::new());
                    }
                }

                if let Some(pad) = pad {
                    let _ = pad.push_event(gst::event::Reconfigure::new());
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "context" => {
                let settings = self.settings.lock().unwrap();
                settings.context.to_value()
            }
            "context-wait" => {
                let settings = self.settings.lock().unwrap();
                (settings.context_wait.as_millis() as u32).to_value()
            }
            "active-pad" => {
                let state = self.state.lock().unwrap();
                let active_pad = state.active_sinkpad.clone();
                active_pad.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::PROVIDE_CLOCK | gst::ElementFlags::REQUIRE_CLOCK);
    }
}

impl GstObjectImpl for InputSelector {}

impl ElementImpl for InputSelector {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing input selector",
                "Generic",
                "Simple input selector element",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let sink_pad_template = gst::PadTemplate::new(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
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

        if let gst::StateChange::ReadyToNull = transition {
            self.unprepare();
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut state = self.state.lock().unwrap();
        let mut pads = self.pads.lock().unwrap();
        let sink_pad = gst::Pad::builder_from_template(templ)
            .name(format!("sink_{}", pads.pad_serial).as_str())
            .build();
        pads.pad_serial += 1;
        sink_pad.set_active(true).unwrap();
        self.obj().add_pad(&sink_pad).unwrap();
        let sink_pad = PadSink::new(sink_pad, InputSelectorPadSinkHandler::default());
        let ret = sink_pad.gst_pad().clone();

        if state.active_sinkpad.is_none() {
            state.active_sinkpad = Some(ret.clone());
            state.switched_pad = true;
        }

        pads.sink_pads.insert(ret.clone(), sink_pad);
        drop(pads);
        drop(state);

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        Some(ret)
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut pads = self.pads.lock().unwrap();
        let sink_pad = pads.sink_pads.remove(pad).unwrap();
        drop(sink_pad);
        self.obj().remove_pad(pad).unwrap();
        drop(pads);

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
    }

    fn provide_clock(&self) -> Option<gst::Clock> {
        Some(gst::SystemClock::obtain())
    }
}
