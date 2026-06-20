// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// Implements RTP over QUIC as per the following specification,
// https://datatracker.ietf.org/doc/draft-ietf-avtcore-rtp-over-quic/

use crate::common::*;
use crate::quinnquicmeta::QuinnQuicMeta;
use crate::quinnquicquery::*;
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use itertools::Itertools;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{LazyLock, Mutex};

const INITIAL_FLOW_ID: u64 = 1;
const MAXIMUM_FLOW_ID: u64 = (1 << 62) - 1;
const DEFAULT_STREAM_PRIORITY: i32 = 0;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnroqmux",
        gst::DebugColorFlags::empty(),
        Some("Quinn RTP over QUIC Muxer"),
    )
});

#[derive(Default)]
struct PadState {
    flow_id_sent: bool,
    stream_id: Option<u64>,
}

struct QuinnRoqMuxPadSettings {
    flow_id: u64,
    priority: i32,
}

impl Default for QuinnRoqMuxPadSettings {
    fn default() -> Self {
        Self {
            flow_id: INITIAL_FLOW_ID,
            priority: 0,
        }
    }
}

#[derive(Default)]
pub(crate) struct QuinnRoqMuxPad {
    settings: Mutex<QuinnRoqMuxPadSettings>,
    state: Mutex<PadState>,
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnRoqMuxPad {
    const NAME: &'static str = "QuinnRoqMuxPad";
    type Type = super::QuinnRoqMuxPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for QuinnRoqMuxPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("flow-id")
                    .nick("Flow identifier")
                    .blurb("Flow identifier")
                    .default_value(INITIAL_FLOW_ID)
                    .minimum(INITIAL_FLOW_ID)
                    .maximum(MAXIMUM_FLOW_ID)
                    .readwrite()
                    .build(),
                glib::ParamSpecInt::builder("priority")
                    .nick("Priority of the stream, ignored by datagrams")
                    .blurb("Priority of the stream, ignored by datagrams")
                    .default_value(DEFAULT_STREAM_PRIORITY)
                    .readwrite()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "flow-id" => {
                let mut settings = self.settings.lock().unwrap();
                settings.flow_id = value.get::<u64>().expect("type checked upstream");
            }
            "priority" => {
                let mut settings = self.settings.lock().unwrap();
                settings.priority = value.get::<i32>().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "flow-id" => {
                let settings = self.settings.lock().unwrap();
                settings.flow_id.to_value()
            }
            "priority" => {
                let settings = self.settings.lock().unwrap();
                settings.priority.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for QuinnRoqMuxPad {}

impl PadImpl for QuinnRoqMuxPad {}

impl ProxyPadImpl for QuinnRoqMuxPad {}

impl AggregatorPadImpl for QuinnRoqMuxPad {}

#[derive(Default)]
struct State {
    datagrams: u64,
    stream_uni_conns: u64,
    segment_updated: bool,
}

pub struct QuinnRoqMux {
    state: Mutex<State>,
}

impl GstObjectImpl for QuinnRoqMux {}

impl ElementImpl for QuinnRoqMux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn RTP over QUIC Multiplexer",
                "Source/Network/QUIC",
                "Multiplexes multiple RTP streams over QUIC",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("application/x-rtp").build();

            let stream_pad_template = gst::PadTemplate::with_gtype(
                "stream_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &sink_caps,
                super::QuinnRoqMuxPad::static_type(),
            )
            .unwrap();

            let datagram_pad_template = gst::PadTemplate::with_gtype(
                "datagram_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &sink_caps,
                super::QuinnRoqMuxPad::static_type(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![stream_pad_template, datagram_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        match templ.name_template() {
            "stream_%u" => {
                let mut state = self.state.lock().unwrap();

                let sink_pad_name = format!("stream_{}", state.stream_uni_conns);

                gst::debug!(CAT, imp = self, "Requesting pad {}", sink_pad_name);

                let sinkpad = gst::PadBuilder::<super::QuinnRoqMuxPad>::from_template(templ)
                    .name(sink_pad_name.clone())
                    .flags(gst::PadFlags::FIXED_CAPS)
                    .build();

                self.obj()
                    .add_pad(&sinkpad)
                    .expect("Failed to add sink pad");

                state.stream_uni_conns += 1;

                drop(state);

                self.obj().child_added(&sinkpad, &sinkpad.name());

                Some(sinkpad.upcast())
            }
            "datagram_%u" => {
                if request_datagram(&self.srcpad()) {
                    gst::warning!(CAT, imp = self, "Datagram unsupported by peer");
                    return None;
                }

                let mut state = self.state.lock().unwrap();

                let sink_pad_name = format!("datagram_{}", state.datagrams);

                gst::debug!(CAT, imp = self, "Requesting pad {}", sink_pad_name);

                let sinkpad = gst::PadBuilder::<super::QuinnRoqMuxPad>::from_template(templ)
                    .name(sink_pad_name.clone())
                    .flags(gst::PadFlags::FIXED_CAPS)
                    .build();

                self.obj()
                    .add_pad(&sinkpad)
                    .expect("Failed to add sink pad");

                state.datagrams += 1;

                drop(state);

                self.obj().child_added(&sinkpad, &sinkpad.name());

                Some(sinkpad.upcast())
            }
            _ => None,
        }
    }

    fn release_pad(&self, pad: &gst::Pad) {
        if pad.name().starts_with("stream") {
            self.close_stream_for_pad(pad);
        }

        self.parent_release_pad(pad);

        self.obj().child_removed(pad, &pad.name());
    }
}

impl ObjectImpl for QuinnRoqMux {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_force_live(true);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnRoqMux {
    const NAME: &'static str = "GstQuinnRoqMux";
    type Type = super::QuinnRoqMux;
    type ParentType = gst_base::Aggregator;
    type Interfaces = (gst::ChildProxy,);

    fn with_class(_klass: &Self::Class) -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }
}

impl ChildProxyImpl for QuinnRoqMux {
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

impl AggregatorImpl for QuinnRoqMux {
    fn aggregate(&self, timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Aggregate (timeout: {timeout})");

        let all_eos = self.obj().sink_pads().iter().all(|pad| {
            pad.downcast_ref::<super::QuinnRoqMuxPad>()
                .expect("Not a QuinnRoqMux pad")
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
                .downcast_ref::<super::QuinnRoqMuxPad>()
                .expect("Not a QuinnRoqMux pad");

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
                .downcast_ref::<super::QuinnRoqMuxPad>()
                .expect("Not a QuinnRoqMux pad");

            let buffer = muxpad.pop_buffer().unwrap();

            if muxpad.name().starts_with("stream_") {
                self.rtp_stream_sink_chain(muxpad, buffer)?;
            } else {
                self.rtp_datagram_sink_chain(muxpad, buffer)?;
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl QuinnRoqMux {
    fn rtp_datagram_sink_chain(
        &self,
        pad: &super::QuinnRoqMuxPad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        /*
         * As per section 5.2.1 of RTP over QUIC specification.
         * Stream encapsulation format for ROQ datagrams is as
         * follows:
         *
         * Payload {
         *      Flow Identifier(i)
         *      RTP Packet(..)
         * }
         *
         * See section 5.3 of RTP over QUIC specification.
         *
         * DATAGRAMs preserve application frame boundaries. Thus, a
         * single RTP packet can be mapped to a single DATAGRAM without
         * additional framing. Because QUIC DATAGRAMs cannot be
         * IP-fragmented (Section 5 of [RFC9221]), senders need to
         * consider the header overhead associated with DATAGRAMs, and
         * ensure that the RTP packets, including their payloads, flow
         * identifier, QUIC, and IP headers, will fit into the Path MTU.
         */

        let mux_pad_settings = pad.imp().settings.lock().unwrap();
        let flow_id = mux_pad_settings.flow_id;
        drop(mux_pad_settings);

        let size = get_varint_size(flow_id);
        let mut outbuf = gst::Buffer::with_size(size).unwrap();
        {
            let outbuffer = outbuf.get_mut().unwrap();
            {
                let mut map = outbuffer.map_writable().unwrap();
                let mut data = map.as_mut_slice();

                set_varint(&mut data, flow_id);
            }

            outbuffer.set_pts(buffer.pts());
            outbuffer.set_dts(buffer.dts());

            QuinnQuicMeta::add(outbuffer, 0, true);
        }

        outbuf.append(buffer);

        self.obj().finish_buffer(outbuf)
    }

    fn rtp_stream_sink_chain(
        &self,
        pad: &super::QuinnRoqMuxPad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        /*
         * As per section 5.2.1 of RTP over QUIC specification.
         * Stream encapsulation format for ROQ streams is as
         * follows:
         *
         * Payload {
         *      Flow Identifier(i)
         *      RTP Payload(..)
         * }
         *
         * RTP Payload {
         *      Length(i)
         *      RTP Packet(..)
         * }
         */

        let mut pad_state = pad.imp().state.lock().unwrap();
        let stream_id = match pad_state.stream_id {
            Some(stream_id) => stream_id,
            None => {
                let mux_pad_settings = pad.imp().settings.lock().unwrap();
                let priority = mux_pad_settings.priority;
                drop(mux_pad_settings);

                gst::info!(CAT, obj = pad, "Requesting stream with priority {priority}");

                match request_stream(&self.srcpad(), priority) {
                    Some(stream_id) => {
                        pad_state.stream_id = Some(stream_id);
                        stream_id
                    }
                    None => {
                        gst::error!(CAT, obj = pad, "Failed to request stream");

                        return Err(gst::FlowError::Error);
                    }
                }
            }
        };

        if !pad_state.flow_id_sent {
            let mux_pad_settings = pad.imp().settings.lock().unwrap();
            let flow_id = mux_pad_settings.flow_id;
            drop(mux_pad_settings);

            let size = get_varint_size(flow_id);
            let mut flow_id_buf = gst::Buffer::with_size(size).unwrap();
            {
                let buffer = flow_id_buf.get_mut().unwrap();
                {
                    let mut map = buffer.map_writable().unwrap();
                    let mut data = map.as_mut_slice();

                    set_varint(&mut data, flow_id);
                }

                QuinnQuicMeta::add(buffer, stream_id, false);
            }

            if let Err(e) = self.obj().finish_buffer(flow_id_buf) {
                gst::error!(CAT, obj = pad, "Failed to push flow id buffer: {e:?}");
                return Err(gst::FlowError::Error);
            }

            pad_state.flow_id_sent = true;
        }

        drop(pad_state);

        let buf_sz_len = get_varint_size(buffer.size() as u64);
        let mut outbuf = gst::Buffer::with_size(buf_sz_len).unwrap();

        gst::trace!(
            CAT,
            obj = pad,
            "Got input buffer of size: {}, pts: {:?}, dts: {:?} for stream: {stream_id}",
            buffer.size(),
            buffer.pts(),
            buffer.dts(),
        );

        {
            let outbuf = outbuf.get_mut().unwrap();
            {
                let mut obuf = outbuf.map_writable().unwrap();
                let mut obuf_slice = obuf.as_mut_slice();
                set_varint(&mut obuf_slice, buffer.size() as u64);
            }

            QuinnQuicMeta::add(outbuf, stream_id, false);

            outbuf.set_pts(buffer.pts());
            outbuf.set_dts(buffer.dts());
        }

        outbuf.append(buffer);

        gst::trace!(
            CAT,
            obj = pad,
            "Pushing buffer of {} bytes for stream: {stream_id}",
            outbuf.size(),
        );

        self.obj().finish_buffer(outbuf)
    }

    fn close_stream_for_pad(&self, pad: &gst::Pad) {
        let mux_pad = pad.downcast_ref::<super::QuinnRoqMuxPad>().unwrap();
        let pad_state = mux_pad.imp().state.lock().unwrap();

        if let Some(stream_id) = pad_state.stream_id {
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
