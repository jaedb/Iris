//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;
use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::RtpBaseDepay2Ext;

#[derive(Default)]
pub struct RtpPcmauDepay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    clock_rate: Option<u32>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtppcmaudepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP PCMA/PCMU Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmauDepay {
    const NAME: &'static str = "GstRtpPcmauDepay2";
    type Type = super::RtpPcmauDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpPcmauDepay {}

impl GstObjectImpl for RtpPcmauDepay {}

impl ElementImpl for RtpPcmauDepay {}

impl crate::basedepay::RtpBaseDepay2Impl for RtpPcmauDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let clock_rate = s.get::<i32>("clock-rate").unwrap_or(8000);

        let src_caps = gst::Caps::builder(
            if self.obj().type_() == super::RtpPcmaDepay::static_type() {
                "audio/x-alaw"
            } else {
                "audio/x-mulaw"
            },
        )
        .field("channels", 1i32)
        .field("rate", clock_rate)
        .build();

        self.state.borrow_mut().clock_rate = Some(clock_rate as u32);

        self.obj().set_src_caps(&src_caps);

        true
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut buffer = packet.payload_buffer();

        let state = self.state.borrow();
        // Always set when caps are set
        let clock_rate = state.clock_rate.unwrap();

        let buffer_ref = buffer.get_mut().unwrap();
        buffer_ref.set_duration(
            (buffer_ref.size() as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                .map(gst::ClockTime::from_nseconds),
        );

        // mark start of talkspurt with RESYNC
        if packet.marker_bit() {
            buffer_ref.set_flags(gst::BufferFlags::RESYNC);
        }

        gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}");

        self.obj().queue_buffer(packet.into(), buffer)
    }
}

/**
 * SECTION:element-rtppcmadepay2
 * @see_also: rtppcmapay2, rtppcmupay2, rtppcmudepay2, alawenc, alawdec
 *
 * Extracts A-law encoded audio from RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.14
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=8000, payload=8' ! rtpjitterbuffer latency=50 ! rtppcmadepay2 ! alawdec ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP A-law audio stream. You can use the #rtppcmapay2 and
 * alawenc elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */

#[derive(Default)]
pub struct RtpPcmaDepay;

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmaDepay {
    const NAME: &'static str = "GstRtpPcmaDepay2";
    type Type = super::RtpPcmaDepay;
    type ParentType = super::RtpPcmauDepay;
}

impl ObjectImpl for RtpPcmaDepay {}

impl GstObjectImpl for RtpPcmaDepay {}

impl ElementImpl for RtpPcmaDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP PCMA Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload A-law from RTP packets (RFC 3551)",
                "Sebastian Dröge <sebastian@centricular.com>",
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
                            .field("media", "audio")
                            .field("payload", 8i32)
                            .field("clock-rate", 8000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "PCMA")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/x-alaw")
                    .field("channels", 1i32)
                    .field("rate", gst::IntRange::new(1i32, i32::MAX))
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpPcmaDepay {}

impl super::RtpPcmauDepayImpl for RtpPcmaDepay {}

/**
 * SECTION:element-rtppcmudepay2
 * @see_also: rtppcmupay2, rtppcmapay2, rtppcmadepay2, mulawenc, mulawdec
 *
 * Extracts µ-law encoded audio from RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.14
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=8000, payload=0' ! rtpjitterbuffer latency=50 ! rtppcmudepay2 ! mulawdec ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP µ-law audio stream. You can use the #rtppcmupay2 and
 * mulawenc elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */

#[derive(Default)]
pub struct RtpPcmuDepay;

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmuDepay {
    const NAME: &'static str = "GstRtpPcmuDepay2";
    type Type = super::RtpPcmuDepay;
    type ParentType = super::RtpPcmauDepay;
}

impl ObjectImpl for RtpPcmuDepay {}

impl GstObjectImpl for RtpPcmuDepay {}

impl ElementImpl for RtpPcmuDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP PCMU Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload µ-law from RTP packets (RFC 3551)",
                "Sebastian Dröge <sebastian@centricular.com>",
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
                            .field("media", "audio")
                            .field("payload", 0i32)
                            .field("clock-rate", 8000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "PCMU")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/x-mulaw")
                    .field("channels", 1i32)
                    .field("rate", gst::IntRange::new(1i32, i32::MAX))
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpPcmuDepay {}

impl super::RtpPcmauDepayImpl for RtpPcmuDepay {}
