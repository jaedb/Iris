// GStreamer RTP MPEG-TS Payloader
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp2tpay2
 * @see_also: rtpmp2tdepay2, rtpmp2tdepay, rtpmp2tpay, tsdemux, mpegtsmux
 *
 * Payload an MPEG Transport Stream into RTP packets as per [RFC 2250][rfc-2250].
 *
 * [rfc-2250]: https://www.rfc-editor.org/rfc/rfc2250.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 videotestsrc ! video/x-raw,width=1280,height=720,format=I420 ! timeoverlay font-desc=Sans,22 ! x264enc tune=zerolatency ! mpegtsmux alignment=7 ! rtpmp2tpay2 ! udpsink host=127.0.0.1 port=5555
 * ]| This will create and payload an MPEG-TS stream with a test pattern and send it out via UDP.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use std::num::NonZeroUsize;

use crate::basepay::{PacketToBufferRelation, RtpBasePay2Ext};

const RTP_MP2T_DEFAULT_PT: u8 = 33;

const RTP_MP2T_DEFAULT_PACKET_SIZE: usize = 188;

#[derive(Default)]
pub struct RtpMP2TPay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct PendingData {
    data: Vec<u8>,
    id: Option<u64>,
}

impl PendingData {
    fn clear(&mut self) {
        self.data.clear();
        self.id = None;
    }

    fn add(&mut self, data: &[u8], id: u64) {
        self.id = self.id.or(Some(id));
        self.data.extend_from_slice(data);
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn id(&self) -> u64 {
        self.id.unwrap()
    }
}

struct State {
    packet_size: Option<NonZeroUsize>,
    discont_pending: bool,

    // Leftover data for the next packet
    pending_data: PendingData,
}

impl Default for State {
    fn default() -> Self {
        State {
            packet_size: NonZeroUsize::new(RTP_MP2T_DEFAULT_PACKET_SIZE),
            discont_pending: true,
            pending_data: Default::default(),
        }
    }
}

impl State {
    fn want_marker_bit(&mut self) -> bool {
        // Clear discont_pending and return current value
        std::mem::replace(&mut self.discont_pending, false)
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp2tpay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-TS Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpMP2TPay {
    const NAME: &'static str = "GstRtpMP2TPay2";
    type Type = super::RtpMP2TPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpMP2TPay {}

impl GstObjectImpl for RtpMP2TPay {}

impl ElementImpl for RtpMP2TPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-TS Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an MPEG Transport Stream into RTP packets (RFC 2250)",
                "Tim-Philipp Müller <tim centricular com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("clock-rate", 90000i32)
                            .field("encoding-name", "MP2T")
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("payload", 33i32)
                            .field("clock-rate", 90000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/mpegts")
                    .field("packetsize", gst::List::new([188i32, 192, 204, 208]))
                    .field("systemstream", true)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basepay::RtpBasePay2Impl for RtpMP2TPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];
    const DEFAULT_PT: u8 = RTP_MP2T_DEFAULT_PT;

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        // Template caps have the field, so we know it exists and is one of the valid sizes
        let packet_size = s.get::<i32>("packetsize").unwrap() as usize;
        assert!(packet_size > 0);

        // Make sure configured MTU is large enough
        let max_payload_size = self.obj().max_payload_size() as usize;

        if packet_size > max_payload_size {
            gst::element_imp_error!(
                self,
                gst::LibraryError::Settings,
                ("Configured MTU is too small"),
                ["Payloader MTU {max_payload_size} must be able to fit at least one MPEG-TS packet of size {packet_size}"]
            );
            return false;
        }

        let src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "video")
            .field("encoding-name", "MP2T")
            .field("clock-rate", 90000i32)
            .build();

        self.obj().set_src_caps(&src_caps);

        let mut state = self.state.borrow_mut();
        state.packet_size = NonZeroUsize::new(packet_size);

        true
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();
        self.send_pending_data(&mut state)
    }

    fn flush(&self) {
        let mut state = self.state.borrow_mut();

        state.pending_data.clear();
        state.discont_pending = true;
    }

    // Encapsulation of MPEG System and Transport Streams:
    // https://www.rfc-editor.org/rfc/rfc2250.html#section-2
    //
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let packet_size = match state.packet_size {
            Some(s) => s.get(),
            None => return Err(gst::FlowError::NotNegotiated),
        };

        // https://www.rfc-editor.org/rfc/rfc2250.html#section-2.1
        //
        // Set marker flag whenever the timestamp is discontinuous.
        if buffer.flags().contains(gst::BufferFlags::DISCONT) {
            gst::debug!(CAT, imp = self, "discont, pushing out pending packets");
            self.send_pending_data(&mut state)?;
            self.obj().finish_pending_packets()?;
            state.discont_pending = true;
        }

        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        if map.size() % packet_size != 0 {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ("MPEG-TS input is not properly framed"),
                [
                    "MPEG-TS packet size {packet_size} but buffer is {} bytes",
                    map.len()
                ]
            );
            return Err(gst::FlowError::Error);
        }

        let max_payload_size = self.obj().max_payload_size() as usize;

        // Target size as clean multiple of the TS packet size
        let target_payload_size = max_payload_size - (max_payload_size % packet_size);

        let mut data = map.as_slice();

        // New data and pending data still isn't enough to fill a whole RTP payload?
        if state.pending_data.len() + data.len() + packet_size <= max_payload_size {
            state.pending_data.add(data, id);
            return Ok(gst::FlowSuccess::Ok);
        }

        // If we have pending data, the first packet will be a combo of the pending data and new data
        if !state.pending_data.is_empty() {
            let pending_id = state.pending_data.id();

            let n_bytes_from_new_data_in_first_packet =
                target_payload_size - state.pending_data.len();

            gst::log!(
                CAT,
                imp = self,
                "Using {} bytes ({} packets) of old data and {} bytes ({} packets) from new buffer",
                state.pending_data.len(),
                state.pending_data.len() / packet_size,
                n_bytes_from_new_data_in_first_packet,
                n_bytes_from_new_data_in_first_packet / packet_size,
            );

            let marker = state.want_marker_bit();

            self.obj().queue_packet(
                PacketToBufferRelation::Ids(pending_id..=id),
                rtp_types::RtpPacketBuilder::new()
                    .payload(state.pending_data.data.as_slice())
                    .payload(&data[0..n_bytes_from_new_data_in_first_packet])
                    .marker_bit(marker),
            )?;

            state.pending_data.clear();

            // Trim off the bytes at the start that were sent out with the old pending data already
            data = &data[n_bytes_from_new_data_in_first_packet..];
        }

        // Send out as many fully-filled RTP packets as we can
        let iter = data.chunks_exact(target_payload_size);

        let remainder = iter.remainder();

        gst::log!(CAT, imp = self,
            "Sending {} bytes ({} packets) in {} RTP packets with max payload size {}, {} bytes ({} packets) remaining for next time",
            data.len() - remainder.len(),
            (data.len() - remainder.len()) / packet_size, data.len() / target_payload_size,
            self.obj().max_payload_size(), remainder.len(),
            remainder.len() / packet_size,
        );

        for packet_payload in iter {
            let marker = state.want_marker_bit();

            self.obj().queue_packet(
                id.into(),
                rtp_types::RtpPacketBuilder::new()
                    .payload(packet_payload)
                    .marker_bit(marker),
            )?;
        }

        // .. and stash any leftovers for sending with the next buffer
        if !remainder.is_empty() {
            state.pending_data.add(remainder, id);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }
}

impl RtpMP2TPay {
    fn send_pending_data(&self, state: &mut State) -> Result<gst::FlowSuccess, gst::FlowError> {
        if state.pending_data.is_empty() {
            gst::log!(CAT, imp = self, "No pending data, nothing to do");
            return Ok(gst::FlowSuccess::Ok);
        }

        gst::log!(
            CAT,
            imp = self,
            "Sending {} bytes of old data",
            state.pending_data.len()
        );

        let pending_id = state.pending_data.id();

        let marker = state.want_marker_bit();

        let res = self.obj().queue_packet(
            pending_id.into(),
            rtp_types::RtpPacketBuilder::new()
                .payload(state.pending_data.data.as_slice())
                .marker_bit(marker),
        );

        state.pending_data.clear();

        res
    }
}
