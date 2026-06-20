// GStreamer RTP KLV Metadata Payloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpklvpay2
 * @see_also: rtpklvdepay2, rtpklvpay, rtpklvdepay
 *
 * Payload an SMPTE ST 336 KLV metadata stream into RTP packets as per [RFC 6597][rfc-6597].
 *
 * [rfc-6597]: https://www.rfc-editor.org/rfc/rfc6597.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 filesrc location=video-with-klv.ts ! tsdemux ! rtpklvpay2 ! udpsink
 * ]| This example pipeline will payload an RTP KLV stream extracted from an
 * MPEG-TS stream and send it via UDP to an RTP receiver. Note that `rtpklvpay2` expects the
 * incoming KLV packets to be timestamped, which may not always be the case when they come from
 * an MPEG-TS file. For testing purposes you can add artificial timestamps with e.g.
 * `identity datarate=2560` for example (then each 256 byte packet will be timestamped 100ms apart).
 *
 * Since: plugins-rs-0.13.0
 */
use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basepay::{RtpBasePay2Ext, RtpBasePay2Impl};

use crate::klv::klv_utils;

#[derive(Default)]
pub struct RtpKlvPay;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpklvpay2",
        gst::DebugColorFlags::empty(),
        Some("RTP KLV Metadata Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpKlvPay {
    const NAME: &'static str = "GstRtpKlvPay2";
    type Type = super::RtpKlvPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpKlvPay {}

impl GstObjectImpl for RtpKlvPay {}

impl ElementImpl for RtpKlvPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP KLV Metadata Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an SMPTE ST 336 KLV metadata stream into RTP packets (RFC 6597)",
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
                &gst::Caps::builder("meta/x-klv")
                    .field("parsed", true)
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "application")
                            .field("encoding-name", "SMPTE336M")
                            .field("clock-rate", 90000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl RtpBasePay2Impl for RtpKlvPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

    fn set_sink_caps(&self, _caps: &gst::Caps) -> bool {
        let src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "application")
            .field("encoding-name", "SMPTE336M")
            .field("clock-rate", 90000i32)
            .build();

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc6597.html#section-4.2
    //
    // We either fit our KLV unit(s) into a single RTP packet or have to split up the KLV unit(s).
    //
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        if map.size() == 0 {
            gst::log!(CAT, imp = self, "Empty buffer, skipping");
            self.obj().drop_buffers(id..=id);
            return Ok(gst::FlowSuccess::Ok);
        }

        let max_payload_size = self.obj().max_payload_size() as usize;

        let mut data = map.as_slice();

        // KLV coding shall use and only use a fixed 16-byte SMPTE-administered
        // Universal Label, according to SMPTE 298M as Key (Rec. ITU R-BT.1653-1)
        let unit_len = match klv_utils::peek_klv(data) {
            Ok(unit_len) => unit_len,
            Err(err) => {
                // Also post warning message?
                gst::warning!(
                    CAT,
                    imp = self,
                    "Input doesn't look like a KLV unit, ignoring. {err:?}",
                );
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        if unit_len != data.len() {
            gst::error!(
                CAT,
                imp = self,
                "Input is not properly framed: KLV unit of size {unit_len} but buffer is {} bytes",
                data.len(),
            );

            if unit_len > data.len() {
                // Also post warning or error message?
                return Ok(gst::FlowSuccess::Ok);
            }

            data = &data[0..unit_len];
        }

        // Data now contains exactly one KLV unit

        while data.len() > max_payload_size {
            self.obj().queue_packet(
                id.into(),
                rtp_types::RtpPacketBuilder::new().payload(&data[0..max_payload_size]),
            )?;

            data = &data[max_payload_size..];
        }

        // Single packet or last packet
        self.obj().queue_packet(
            id.into(),
            rtp_types::RtpPacketBuilder::new()
                .payload(data)
                .marker_bit(true),
        )
    }
}

impl RtpKlvPay {}
