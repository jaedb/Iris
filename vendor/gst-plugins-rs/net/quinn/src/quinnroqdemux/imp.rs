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
use bytes::{Buf, BytesMut};
use gst::{glib, prelude::*, subclass::prelude::*};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Cursor, Read};
use std::ops::{Range, RangeBounds, RangeFrom};
use std::sync::{LazyLock, Mutex};

const SIGNAL_FLOW_ID_MAP: &str = "request-flow-id-map";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnroqdemux",
        gst::DebugColorFlags::empty(),
        Some("Quinn RTP over QUIC Demuxer"),
    )
});

struct Reassembler {
    buffer: BytesMut,
    last_packet_len: u64,
    // Track the timestamp of the last buffer pushed. Chunk boundaries
    // in QUIC do not correspond to peer writes, and hence cannot be
    // used for framing. BaseSrc will timestamp the buffers captured
    // with running time. We make a best case effort to timestamp the
    // reassembled packets by using the timestamp of the last buffer
    // pushed after which a reassembled packet becomes available.
    last_ts: Option<gst::ClockTime>,
    // Source pad which this reassembler is for, primarily used for
    // trace logging.
    pad: gst::Pad,
}

impl Reassembler {
    fn new(pad: gst::Pad) -> Self {
        Self {
            // `quinnquicsrc` reads 4096 bytes at a time by default
            // which is the default BaseSrc blocksize.
            buffer: BytesMut::with_capacity(4096),
            // Track the length of the packet we are reassembling
            last_packet_len: 0,
            last_ts: None,
            pad,
        }
    }

    fn build_buffer(&mut self, packet_sz: u64) -> gst::Buffer {
        let packet = self.buffer.split_to(packet_sz as usize);

        let mut buffer = gst::Buffer::with_size(packet_sz as usize).unwrap();
        {
            let buffer_mut = buffer.get_mut().unwrap();
            {
                let mut buf_mut = buffer_mut.map_writable().unwrap();
                buf_mut.clone_from_slice(&packet);
            }

            // Set DTS and let downstream manage PTS using jitterbuffer
            // as jitterbuffer will do the DTS -> PTS work.
            buffer_mut.set_dts(self.last_ts);
        }

        buffer.to_owned()
    }

    fn push_buffer(
        &mut self,
        buffer: &mut gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if buffer.size() == 0 {
            return Ok(gst::FlowSuccess::Ok);
        }

        self.last_ts = buffer.dts();

        let buffer_mut = buffer.get_mut().ok_or(gst::FlowError::Error)?;
        let map = buffer_mut
            .map_readable()
            .map_err(|_| gst::FlowError::Error)?;
        let buffer_slice = map.as_slice();

        self.buffer.extend_from_slice(buffer_slice);

        gst::trace!(
            CAT,
            obj = self.pad,
            "Added buffer of {} bytes, current buffer size: {}",
            buffer_slice.len(),
            self.buffer.len()
        );

        Ok(gst::FlowSuccess::Ok)
    }

    fn push_bytes(
        &mut self,
        buffer: &[u8],
        dts: Option<gst::ClockTime>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if buffer.is_empty() {
            return Ok(gst::FlowSuccess::Ok);
        }

        self.last_ts = dts;

        self.buffer.extend_from_slice(buffer);

        gst::trace!(
            CAT,
            obj = self.pad,
            "Added {} bytes, current buffer size: {}",
            buffer.len(),
            self.buffer.len()
        );

        Ok(gst::FlowSuccess::Ok)
    }

    fn pop(&mut self) -> Option<gst::Buffer> {
        if self.buffer.is_empty() {
            return None;
        }

        self.last_ts?;

        if self.last_packet_len != 0 {
            // In the middle of reassembling a packet
            if self.buffer.len() > self.last_packet_len as usize {
                // Enough data to reassemble the packet
                let buffer = self.build_buffer(self.last_packet_len);

                self.last_packet_len = 0;

                gst::trace!(
                    CAT,
                    obj = self.pad,
                    "Reassembled packet of size: {}, current buffer size: {}",
                    buffer.size(),
                    self.buffer.len()
                );

                return Some(buffer.to_owned());
            } else {
                // Do not have enough data yet to reassemble the packet
                return None;
            }
        }

        let (packet_sz, packet_sz_len) = get_varint(&self.buffer)?;

        gst::trace!(
            CAT,
            obj = self.pad,
            "Reassembler, packet size length: {}, packet: {}, buffer: {}",
            packet_sz_len,
            packet_sz,
            self.buffer.len(),
        );

        self.buffer.advance(packet_sz_len);

        if packet_sz > self.buffer.len() as u64 {
            // Packet will span multiple buffers
            self.last_packet_len = packet_sz;

            gst::trace!(
                CAT,
                obj = self.pad,
                "Accumulating for packet of size: {}, current buffer size: {}",
                packet_sz,
                self.buffer.len()
            );

            return None;
        }

        let buffer = self.build_buffer(packet_sz);

        gst::trace!(
            CAT,
            obj = self.pad,
            "Reassembled packet of size: {}, current buffer size: {}",
            buffer.size(),
            self.buffer.len()
        );

        Some(buffer.to_owned())
    }
}

#[derive(Default)]
struct Started {
    stream_pads_map: HashMap<u64 /* Stream ID */, (gst::Pad, Reassembler)>,
    datagram_pads_map: HashMap<u64 /* Flow ID */, gst::Pad>,
}

#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started(Started),
}

pub struct QuinnRoqDemux {
    state: Mutex<State>,
    sinkpad: gst::Pad,
}

impl GstObjectImpl for QuinnRoqDemux {}

impl ElementImpl for QuinnRoqDemux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn RTP over QUIC Demultiplexer",
                "Source/Network/QUIC",
                "Demultiplexes multiple RTP streams over QUIC",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let src_caps = gst::Caps::builder("application/x-rtp").build();

            let src_pad_template = gst::PadTemplate::new(
                "src_%u", // src_<flow-id>
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
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
        let ret = self.parent_change_state(transition)?;

        if let gst::StateChange::NullToReady = transition {
            let mut state = self.state.lock().unwrap();
            *state = State::Started(Started::default());
        }

        Ok(ret)
    }
}

impl ObjectImpl for QuinnRoqDemux {
    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            /*
             * See section 5.1 of RTP over QUIC specification.
             *
             * Endpoints need to associate flow identifiers with RTP
             * streams. Depending on the context of the application,
             * the association can be statically configured, signaled
             * using an out-of-band signaling mechanism (e.g., SDP),
             * or applications might be able to identify the stream
             * based on the RTP packets sent on the stream (e.g., by
             * inspecting the payload type).
             *
             * In this implementation, we use this signal to associate
             * the flow-id with an incoming stream by requesting caps.
             * If no caps are provided, the pipeline will fail with a
             * flow error.
             *
             * TODO: Close the connection with ROQ_UNKNOWN_FLOW_ID error
             * code if the signal fails. This will have to be communicated
             * upstream to quinnquicsrc.
             */
            vec![glib::subclass::Signal::builder(SIGNAL_FLOW_ID_MAP)
                .param_types([u64::static_type()])
                .return_type::<gst::Caps>()
                .build()]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        self.obj()
            .add_pad(&self.sinkpad)
            .expect("Failed to add sink pad");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnRoqDemux {
    const NAME: &'static str = "GstQuinnQuicRtpDemux";
    type Type = super::QuinnRoqDemux;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let sinkpad = gst::Pad::builder_from_template(&klass.pad_template("sink").unwrap())
            .chain_function(|_pad, parent, buffer| {
                QuinnRoqDemux::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |demux| demux.sink_chain(buffer),
                )
            })
            .event_function(|pad, parent, event| {
                QuinnRoqDemux::catch_panic_pad_function(
                    parent,
                    || false,
                    |demux| demux.sink_event(pad, event),
                )
            })
            .build();

        Self {
            state: Mutex::new(State::default()),
            sinkpad,
        }
    }
}

impl QuinnRoqDemux {
    fn add_srcpad_for_flowid(&self, flow_id: u64) -> Result<gst::Pad, gst::FlowError> {
        let caps = self
            .obj()
            .emit_by_name::<Option<gst::Caps>>(SIGNAL_FLOW_ID_MAP, &[&(flow_id)])
            .ok_or_else(|| {
                gst::error!(CAT, imp = self, "Could not get caps for flow-id {flow_id}");
                gst::FlowError::Error
            })?;

        let src_pad_name = format!("src_{flow_id}");
        let templ = self.obj().pad_template("src_%u").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .name(src_pad_name.clone())
            .build();

        srcpad.set_active(true).unwrap();

        let stream_start_evt = gst::event::StreamStart::builder(&flow_id.to_string())
            .group_id(gst::GroupId::next())
            .build();
        srcpad.push_event(stream_start_evt);

        gst::log!(
            CAT,
            imp = self,
            "Caps {caps:?}, received for pad {src_pad_name} for flow-id {flow_id}"
        );

        srcpad.push_event(gst::event::Caps::new(&caps));

        let segment_evt = gst::event::Segment::new(&gst::FormattedSegment::<gst::ClockTime>::new());
        srcpad.push_event(segment_evt);

        self.obj().add_pad(&srcpad).expect("Failed to add pad");

        gst::trace!(
            CAT,
            imp = self,
            "Added pad {src_pad_name} for flow-id {flow_id}"
        );

        Ok(srcpad)
    }

    fn remove_pad(&self, stream_id: u64) -> bool {
        gst::debug!(CAT, imp = self, "Removing pad for stream id {stream_id}");

        let mut state = self.state.lock().unwrap();
        let stream_pad = if let State::Started(ref mut state) = *state {
            state.stream_pads_map.remove(&stream_id)
        } else {
            None
        };
        drop(state);

        if let Some((pad, reassembler)) = stream_pad {
            drop(reassembler);

            let _ = pad.set_active(false);

            if let Err(err) = self.obj().remove_pad(&pad) {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to remove pad {} for stream id {stream_id}, error: {err:?}",
                    pad.name()
                );
                return false;
            } else {
                gst::log!(
                    CAT,
                    imp = self,
                    "Pad {} removed for stream id {stream_id}",
                    pad.name()
                );
                return true;
            }
        }

        false
    }

    fn sink_chain(&self, buffer: gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let meta = buffer.meta::<QuinnQuicMeta>();

        match meta {
            Some(m) => {
                if m.is_datagram() {
                    self.datagram_sink_chain(buffer)
                } else {
                    let stream_id = m.stream_id();

                    self.stream_sink_chain(buffer, stream_id)
                }
            }
            None => {
                gst::warning!(CAT, imp = self, "Buffer dropped, no metadata");
                Ok(gst::FlowSuccess::Ok)
            }
        }
    }

    fn datagram_sink_chain(
        &self,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        /*
         * See section 5.3 of RTP over QUIC specification.
         *
         * DATAGRAMs preserve application frame boundaries. Thus, a
         * single RTP packet can be mapped to a single DATAGRAM without
         * additional framing.
         *
         * Since datagrams preserve framing we do not need packet
         * reassembly here.
         */
        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut started) = *state {
            let dts = buffer.dts();
            let buf_mut = buffer.get_mut().ok_or(gst::FlowError::Error)?;
            let map = buf_mut.map_readable().map_err(|_| gst::FlowError::Error)?;

            let data = map.as_slice();
            let varint = get_varint(data);

            if varint.is_none() {
                gst::error!(CAT, imp = self, "Unexpected VarInt parse error");
                return Err(gst::FlowError::Error);
            }

            let (flow_id, flow_id_len) = {
                let (flow_id, flow_id_len) = varint.unwrap();
                (flow_id, flow_id_len)
            };

            gst::trace!(CAT, imp = self, "Got buffer with flow-id {flow_id}",);

            let mut outbuf = buf_mut
                .copy_region(gst::BufferCopyFlags::all(), flow_id_len..)
                .unwrap();
            {
                let outbuf_mut = outbuf.get_mut().ok_or(gst::FlowError::Error)?;
                // Set DTS and let downstream manage PTS using jitterbuffer
                // as jitterbuffer will do the DTS -> PTS work.
                outbuf_mut.set_dts(dts);
            }

            match started.datagram_pads_map.get_mut(&flow_id) {
                Some(srcpad) => {
                    gst::trace!(
                        CAT,
                        obj = srcpad,
                        "Pushing buffer of {} bytes with dts: {:?}",
                        outbuf.size(),
                        outbuf.dts(),
                    );

                    return srcpad.push(outbuf);
                }
                None => {
                    let srcpad = self.add_srcpad_for_flowid(flow_id)?;

                    gst::trace!(
                        CAT,
                        obj = srcpad,
                        "Pushing buffer of {} bytes with dts: {:?}",
                        outbuf.size(),
                        outbuf.dts(),
                    );

                    started.datagram_pads_map.insert(flow_id, srcpad.clone());

                    return srcpad.push(outbuf);
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn stream_sink_chain(
        &self,
        mut buffer: gst::Buffer,
        stream_id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut started) = *state {
            match started.stream_pads_map.get_mut(&stream_id) {
                Some((srcpad, reassembler)) => {
                    reassembler.push_buffer(&mut buffer)?;

                    while let Some(buffer) = reassembler.pop() {
                        gst::trace!(
                            CAT,
                            obj = srcpad,
                            "Pushing buffer of {} bytes with dts: {:?} for stream: {stream_id}",
                            buffer.size(),
                            buffer.dts(),
                        );

                        if let Err(err) = srcpad.push(buffer) {
                            gst::error!(CAT, imp = self, "Failed to push buffer: {err}");
                            return Err(gst::FlowError::Error);
                        }
                    }

                    return Ok(gst::FlowSuccess::Ok);
                }
                None => {
                    let dts = buffer.dts();

                    let buf_mut = buffer.get_mut().ok_or(gst::FlowError::Error)?;
                    let map = buf_mut.map_readable().map_err(|_| gst::FlowError::Error)?;

                    let data = map.as_slice();
                    let varint = get_varint(data);

                    if varint.is_none() {
                        gst::error!(CAT, imp = self, "Unexpected VarInt parse error");
                        return Err(gst::FlowError::Error);
                    }

                    // We have not seen the flow id for this pad
                    let (flow_id, flow_id_len) = {
                        let (flow_id, flow_id_len) = varint.unwrap();
                        (flow_id, flow_id_len)
                    };

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Got stream-id {stream_id} with flow-id {flow_id} of length {flow_id_len} {}",
                        data.len()
                    );

                    let srcpad = self.add_srcpad_for_flowid(flow_id)?;

                    let mut reassembler = Reassembler::new(srcpad.clone());
                    reassembler.push_bytes(&data[flow_id_len..], dts)?;

                    while let Some(buffer) = reassembler.pop() {
                        gst::trace!(
                            CAT,
                            obj = srcpad,
                            "Pushing output buffer of size: {} with dts: {:?} for stream: {stream_id}",
                            buffer.size(),
                            buffer.dts(),
                        );

                        if let Err(err) = srcpad.push(buffer) {
                            gst::error!(CAT, imp = self, "Failed to push buffer: {err}");
                            return Err(gst::FlowError::Error);
                        }
                    }

                    started
                        .stream_pads_map
                        .insert(stream_id, (srcpad, reassembler));

                    return Ok(gst::FlowSuccess::Ok);
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling event {:?}", event);

        if let EventView::CustomDownstream(ev) = event.view() {
            if let Some(s) = ev.structure() {
                if s.name() == QUIC_STREAM_CLOSE_CUSTOMDOWNSTREAM_EVENT {
                    if let Ok(stream_id) = s.get::<u64>(QUIC_STREAM_ID) {
                        return self.remove_pad(stream_id);
                    }
                }
            }
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}
