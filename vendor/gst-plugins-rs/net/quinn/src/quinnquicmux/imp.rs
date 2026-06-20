// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::quinnquicmeta::QuinnQuicMeta;
use crate::quinnquicquery::*;
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use itertools::Itertools;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{LazyLock, Mutex};

const DEFAULT_STREAM_PRIORITY: i32 = 0;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnquicmux",
        gst::DebugColorFlags::empty(),
        Some("Quinn QUIC Mux"),
    )
});

#[derive(Default)]
struct QuinnQuicMuxPadSettings {
    priority: i32,
}

#[derive(Default)]
pub(crate) struct QuinnQuicMuxPad {
    settings: Mutex<QuinnQuicMuxPadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnQuicMuxPad {
    const NAME: &'static str = "QuinnQuicMuxPad";
    type Type = super::QuinnQuicMuxPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for QuinnQuicMuxPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecInt::builder("priority")
                .nick("Priority of the stream")
                .blurb("Priority of the stream")
                .default_value(DEFAULT_STREAM_PRIORITY)
                .readwrite()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "priority" => {
                let mut settings = self.settings.lock().unwrap();
                settings.priority = value.get::<i32>().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "priority" => {
                let settings = self.settings.lock().unwrap();
                settings.priority.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for QuinnQuicMuxPad {}

impl PadImpl for QuinnQuicMuxPad {}

impl ProxyPadImpl for QuinnQuicMuxPad {}

impl AggregatorPadImpl for QuinnQuicMuxPad {}

#[derive(Default)]
struct State {
    stream_uni_conns: u64,
    datagram_requested: bool,
    stream_id_map: HashMap<gst::Pad, u64>,
    segment_updated: bool,
}

pub struct QuinnQuicMux {
    state: Mutex<State>,
}

impl GstObjectImpl for QuinnQuicMux {}

impl ElementImpl for QuinnQuicMux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn QUIC Multiplexer",
                "Source/Network/QUIC",
                "Multiplexes multiple streams and datagram for QUIC",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let stream_uni_pad_template = gst::PadTemplate::with_gtype(
                "stream_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &gst::Caps::new_any(),
                super::QuinnQuicMuxPad::static_type(),
            )
            .unwrap();

            let datagram_pad_template = gst::PadTemplate::with_gtype(
                "datagram",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &gst::Caps::new_any(),
                gst_base::AggregatorPad::static_type(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![
                datagram_pad_template,
                stream_uni_pad_template,
                src_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        match templ.name_template() {
            "stream_%u" => {
                let mut state = self.state.lock().unwrap();

                let stream_pad_name = if let Some(pad_name) = name {
                    pad_name.to_string()
                } else {
                    state.stream_uni_conns += 1;
                    format!("stream_{}", state.stream_uni_conns).to_string()
                };

                gst::debug!(CAT, imp = self, "Requesting pad {}", stream_pad_name);

                let stream_pad = gst::PadBuilder::<super::QuinnQuicMuxPad>::from_template(templ)
                    .name(stream_pad_name)
                    .flags(gst::PadFlags::FIXED_CAPS)
                    .build();

                self.obj()
                    .add_pad(&stream_pad)
                    .expect("Failed to add unidirectional stream pad");

                drop(state);

                self.obj().child_added(&stream_pad, &stream_pad.name());

                Some(stream_pad.upcast())
            }
            "datagram" => {
                gst::debug!(CAT, imp = self, "Requesting datagram pad");

                let mut state = self.state.lock().unwrap();

                if state.datagram_requested {
                    gst::warning!(CAT, imp = self, "datagram pad has already been requested");

                    return None;
                }

                let datagram_pad = gst::Pad::builder_from_template(templ)
                    .name("datagram")
                    .flags(gst::PadFlags::FIXED_CAPS)
                    .build();

                state.datagram_requested = true;

                self.obj()
                    .add_pad(&datagram_pad)
                    .expect("Failed to add datagram pad");

                drop(state);

                self.obj().child_added(&datagram_pad, &datagram_pad.name());

                Some(datagram_pad.upcast())
            }
            _ => None,
        }
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut state = self.state.lock().unwrap();

        if pad.name() == "datagram" {
            state.datagram_requested = false;
        } else if pad.name().starts_with("stream") {
            self.close_stream_for_pad(pad, &mut state);
        }

        drop(state);

        self.parent_release_pad(pad);

        self.obj().child_removed(pad, &pad.name());
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if let gst::StateChange::NullToReady = transition {
            for pad in self.obj().sink_pads() {
                if pad.name() == "datagram" && !request_datagram(&self.srcpad()) {
                    gst::warning!(CAT, imp = self, "Datagram unsupported by the peer");

                    return Err(gst::StateChangeError);
                }
            }
        }

        self.parent_change_state(transition)
    }
}

impl AggregatorImpl for QuinnQuicMux {
    fn aggregate(&self, timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Aggregate (timeout: {timeout})");

        let all_eos = self.obj().sink_pads().iter().all(|pad| {
            pad.downcast_ref::<super::QuinnQuicMuxPad>()
                .expect("Not a QuinnQuicMux pad")
                .is_eos()
        });

        if all_eos {
            gst::debug!(CAT, imp = self, "All pads are EOS now");
            return Err(gst::FlowError::Eos);
        }

        // Buffers might arrive with different PTS on the pads. Consider
        // a scenario where a buffer with PTS 1s arrives after a buffer
        // with PTS 2s on one of the pads. Buffer with PTS 2s gets pushed
        // and then buffer with PTS 1s gets pushed. Sink will wait to sync
        // on 2s PTS. This effectively creates a scenario similar to HOL
        // blocking which we want to avoid with QUIC streams in the first
        // place.
        //
        // To avoid this, we order the pads based on the buffer PTS and
        // then push the buffers downstream.
        let mut pad_pts: Vec<(Option<gst::ClockTime>, gst::Pad)> = Vec::new();
        let mut state = self.state.lock().unwrap();

        for pad in self.obj().sink_pads() {
            let muxpad = pad
                .downcast_ref::<super::QuinnQuicMuxPad>()
                .expect("Not a QuinnQuicMux pad");

            if muxpad.is_eos() {
                continue;
            }

            match muxpad.peek_buffer() {
                Some(buffer) => {
                    pad_pts.push((buffer.pts(), pad));
                }
                None => continue,
            }
        }

        let buffers_sorted_by_pts = pad_pts.into_iter().sorted_by_key(|x| x.0);
        gst::trace!(
            CAT,
            imp = self,
            "Buffers PTS sorted: {buffers_sorted_by_pts:?}"
        );

        for (pts, pad) in buffers_sorted_by_pts {
            if !state.segment_updated {
                state.segment_updated = true;
                let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
                segment.set_start(pts);
                self.obj().update_segment(&segment);
            }

            let muxpad = pad
                .downcast_ref::<super::QuinnQuicMuxPad>()
                .expect("Not a QuinnQuicMux pad");

            if muxpad.name().starts_with("stream_") {
                let stream_id = match state.stream_id_map.get(&pad) {
                    Some(stream_id) => *stream_id,
                    None => {
                        let mux_pad_settings = muxpad.imp().settings.lock().unwrap();
                        let priority = mux_pad_settings.priority;
                        drop(mux_pad_settings);

                        gst::info!(
                            CAT,
                            obj = pad,
                            "Requesting stream connection with priority {priority}"
                        );

                        match request_stream(&self.srcpad(), priority) {
                            Some(stream_id) => {
                                state.stream_id_map.insert(pad.clone(), stream_id);
                                stream_id
                            }
                            None => {
                                gst::error!(CAT, obj = pad, "Failed to request stream");

                                return Err(gst::FlowError::Error);
                            }
                        }
                    }
                };

                let buffer = {
                    let mut buffer = muxpad.pop_buffer().unwrap();
                    let outbuf = buffer.make_mut();

                    QuinnQuicMeta::add(outbuf, stream_id, false);

                    buffer
                };

                gst::trace!(
                    CAT,
                    obj = pad,
                    "Finishing buffer {buffer:?} for stream {stream_id}"
                );

                self.obj().finish_buffer(buffer)?;

                gst::trace!(CAT, obj = pad, "Finished buffer for stream {stream_id}");
            } else {
                let buffer = {
                    let mut buffer = muxpad.pop_buffer().unwrap();
                    let outbuf = buffer.make_mut();

                    QuinnQuicMeta::add(outbuf, 0, true);

                    buffer
                };

                gst::trace!(CAT, obj = pad, "Finishing buffer {buffer:?} for datagram");

                self.obj().finish_buffer(buffer)?;
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl ObjectImpl for QuinnQuicMux {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_force_live(true);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnQuicMux {
    const NAME: &'static str = "GstQuinnQuicMux";
    type Type = super::QuinnQuicMux;
    type ParentType = gst_base::Aggregator;
    type Interfaces = (gst::ChildProxy,);

    fn with_class(_klass: &Self::Class) -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }
}

impl ChildProxyImpl for QuinnQuicMux {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}

impl QuinnQuicMux {
    fn close_stream_for_pad(&self, pad: &gst::Pad, state: &mut State) {
        if let Some(stream_id) = state.stream_id_map.remove(pad) {
            if close_stream(&self.srcpad(), stream_id) {
                gst::info!(CAT, obj = pad, "Closed connection");
            } else {
                gst::warning!(CAT, obj = pad, "Failed to close connection");
            }
        }
    }

    fn srcpad(&self) -> gst::Pad {
        let obj = self.obj();
        obj.src_pad().upcast_ref::<gst::Pad>().clone()
    }
}
