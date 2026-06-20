// GStreamer RTP KLV Metadata Depayloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpklvdepay2
 * @see_also: rtpklvpay2, rtpklvdepay, rtpklvpay
 *
 * Depayload an SMPTE ST 336 KLV metadata stream from RTP packets as per [RFC 6597][rfc-6597].
 *
 * [rfc-6597]: https://www.rfc-editor.org/rfc/rfc6597.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=(string)application, clock-rate=(int)90000, encoding-name=(string)SMPTE336M' ! rtpklvdepay2 ! fakesink dump=true
 * ]| This will depayload an RTP KLV stream and display a hexdump of the KLV data on stdout.
 * You can use the #rtpklvpay2 or #rtpklvpay elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{
    Packet, PacketToBufferRelation, RtpBaseDepay2Ext, RtpBaseDepay2Impl, RtpBaseDepay2ImplExt,
};

use crate::klv::klv_utils;

use std::cmp::Ordering;

#[derive(Debug, PartialEq)]
enum LooksLike {
    Start,
    SelfContained,
    Undetermined,
}

#[derive(Default)]
pub struct RtpKlvDepay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    prev_marker_seqnum: Option<u64>,
    accumulator: Vec<u8>,
    acc_seqnum: Option<u64>,
    acc_ts: Option<u64>,
}

impl State {
    fn clear_accumulator(&mut self) {
        self.accumulator.clear();
        self.acc_seqnum = None;
        self.acc_ts = None;
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpklvdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP KLV Metadata Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpKlvDepay {
    const NAME: &'static str = "GstRtpKlvDepay2";
    type Type = super::RtpKlvDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpKlvDepay {}

impl GstObjectImpl for RtpKlvDepay {}

impl ElementImpl for RtpKlvDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP KLV Metadata Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an SMPTE ST 336 KLV metadata stream from RTP packets (RFC 6597)",
                "Tim-Philipp Müller <tim centricular com>",
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
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "application")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "SMPTE336M")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("meta/x-klv")
                    .field("parsed", true)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl RtpBaseDepay2Impl for RtpKlvDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

    fn set_sink_caps(&self, _caps: &gst::Caps) -> bool {
        let src_caps = gst::Caps::builder("meta/x-klv")
            .field("parsed", true)
            .build();

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc6597.html#section-4.2
    //
    // We either get a full single KLV unit in an RTP packet, or a fragment of a single KLV unit.
    //
    fn handle_packet(&self, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let payload = packet.payload();

        // Clear out any unused accumulated data on discont or timestamp changes
        if !state.accumulator.is_empty()
            && (packet.discont() || state.acc_ts != Some(packet.ext_timestamp()))
        {
            gst::debug!(
                CAT,
                imp = self,
                "Discontinuity, discarding {} bytes in accumulator",
                state.accumulator.len(),
            );

            state.clear_accumulator();
        }

        let looks_like = match klv_utils::peek_klv(payload) {
            Ok(klv_unit_size) => match payload.len().cmp(&klv_unit_size) {
                Ordering::Equal => LooksLike::SelfContained,
                Ordering::Less => LooksLike::Start,
                Ordering::Greater => LooksLike::Undetermined, // Questionable?
            },
            _ => LooksLike::Undetermined,
        };

        // Packet looks like start or self-contained, or is directly after one with marker bit set?
        let start = looks_like != LooksLike::Undetermined
            || match state.prev_marker_seqnum {
                Some(prev_marker_seqnum) => packet.ext_seqnum() == (prev_marker_seqnum + 1),
                None => false,
            };

        let end = packet.marker_bit() || looks_like == LooksLike::SelfContained;

        gst::trace!(
            CAT,
            imp = self,
            "start: {start}, end: {end}, looks like: {looks_like:?}"
        );

        if end {
            state.prev_marker_seqnum = Some(packet.ext_seqnum());
        }

        if start && looks_like == LooksLike::Undetermined {
            gst::warning!(
                CAT,
                imp = self,
                "New start, but data doesn't look like the start of a KLV unit?! Discarding",
            );
            state.clear_accumulator();
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        // Self-contained? Push out as-is, re-using the input buffer

        if looks_like == LooksLike::SelfContained {
            state.clear_accumulator();
            gst::debug!(
                CAT,
                imp = self,
                "Finished KLV unit, pushing out {} bytes",
                payload.len()
            );
            return self
                .obj()
                .queue_buffer(packet.into(), packet.payload_buffer());
        }

        // .. else accumulate

        if looks_like == LooksLike::Start {
            if !state.accumulator.is_empty() {
                gst::debug!(
                    CAT,
                    imp = self,
                    "New start, but still {} bytes in accumulator, discarding",
                    state.accumulator.len(),
                );
                state.clear_accumulator();
            }

            state.accumulator.extend_from_slice(payload);
            state.acc_seqnum = Some(packet.ext_seqnum());
            state.acc_ts = Some(packet.ext_timestamp());

            // if it looks like a start we know we don't have enough bytes yet
            gst::debug!(
                CAT,
                imp = self,
                "Start. Have {} bytes, but want {} bytes, waiting for more data",
                state.accumulator.len(),
                klv_utils::peek_klv(payload).unwrap(),
            );

            return Ok(gst::FlowSuccess::Ok);
        }

        // Continuation fragment

        assert_eq!(looks_like, LooksLike::Undetermined);

        if state.accumulator.is_empty() {
            gst::debug!(
                CAT,
                imp = self,
                "Continuation fragment, but no data in accumulator. Need to wait for start of next unit, discarding.",
            );
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        state.accumulator.extend_from_slice(payload);

        let Ok(klv_unit_size) = klv_utils::peek_klv(&state.accumulator) else {
            gst::warning!(
                CAT,
                imp = self,
                "Accumulator does not contain KLV unit start?! Clearing.",
            );

            state.clear_accumulator();
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        };

        gst::log!(
            CAT,
            imp = self,
            "Continuation. Have {} bytes, want {} bytes",
            state.accumulator.len(),
            klv_unit_size,
        );

        // Push out once we have enough data
        if state.accumulator.len() >= klv_unit_size || end {
            if state.accumulator.len() != klv_unit_size {
                if state.accumulator.len() > klv_unit_size {
                    gst::warning!(CAT, imp = self, "More bytes than expected in accumulator!");
                } else {
                    // For now we'll honour the marker bit unconditionally and don't second-guess it
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Fewer bytes than expected in accumulator, but marker bit set!",
                    );
                }
            }

            let accumulator = std::mem::replace(
                &mut state.accumulator,
                Vec::<u8>::with_capacity(klv_unit_size),
            );

            gst::debug!(
                CAT,
                imp = self,
                "Finished KLV unit, pushing out {} bytes",
                accumulator.len(),
            );

            let outbuf = gst::Buffer::from_mut_slice(accumulator);

            let first_seqnum = state.acc_seqnum.unwrap();

            return self.obj().queue_buffer(
                PacketToBufferRelation::Seqnums(first_seqnum..=packet.ext_seqnum()),
                outbuf,
            );
        }

        // .. else wait for more data
        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, mut event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        #[allow(clippy::single_match)]
        match event.view() {
            // Add SPARSE flag to stream-start event stream flags
            gst::EventView::StreamStart(stream_start) => {
                let stream_flags = stream_start.stream_flags();

                let ev = event.make_mut();
                let s = ev.structure_mut();

                s.set("stream-flags", stream_flags | gst::StreamFlags::SPARSE);
            }
            _ => (),
        };

        self.parent_sink_event(event)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }
}
