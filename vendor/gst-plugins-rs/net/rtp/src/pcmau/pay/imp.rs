//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::{
    baseaudiopay::RtpBaseAudioPay2Ext,
    basepay::{RtpBasePay2Ext, RtpBasePay2ImplExt},
};

#[derive(Default)]
pub struct RtpPcmauPay;

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmauPay {
    const ABSTRACT: bool = true;
    const NAME: &'static str = "GstRtpPcmauPay2";
    type Type = super::RtpPcmauPay;
    type ParentType = crate::baseaudiopay::RtpBaseAudioPay2;
}

impl ObjectImpl for RtpPcmauPay {}

impl GstObjectImpl for RtpPcmauPay {}

impl ElementImpl for RtpPcmauPay {}

impl crate::basepay::RtpBasePay2Impl for RtpPcmauPay {
    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let rate = u32::try_from(s.get::<i32>("rate").unwrap()).unwrap();

        let src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "audio")
            .field(
                "encoding-name",
                if self.obj().type_() == super::RtpPcmaPay::static_type() {
                    "PCMA"
                } else {
                    "PCMU"
                },
            )
            .field("clock-rate", rate as i32)
            .build();

        self.obj().set_src_caps(&src_caps);
        self.obj().set_bpf(1);

        true
    }

    #[allow(clippy::single_match)]
    fn sink_query(&self, query: &mut gst::QueryRef) -> bool {
        match query.view_mut() {
            gst::QueryViewMut::Caps(query) => {
                let mut caps = self.obj().sink_pad().pad_template_caps();

                // If the payload type is 0 or 8 then only 8000Hz are supported.
                if [0, 8].contains(&self.obj().property::<u32>("pt")) {
                    let caps = caps.make_mut();
                    caps.set("rate", 8000);
                }

                if let Some(filter) = query.filter() {
                    caps = filter.intersect_with_mode(&caps, gst::CapsIntersectMode::First);
                }

                query.set_result(&caps);

                return true;
            }
            _ => (),
        }

        self.parent_sink_query(query)
    }
}

impl crate::baseaudiopay::RtpBaseAudioPay2Impl for RtpPcmauPay {}

/**
 * SECTION:element-rtppcmapay2
 * @see_also: rtppcmadepay2, rtppcmupay2, rtppcmudepay2, alawenc, alawdec
 *
 * Payloads A-law encoded audio into RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! audio/x-raw,rate=8000,channels=1 ! alawenc ! rtppcmapay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will generate an A-law audio test signal and payload it as RTP and send it out
 * as UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */

#[derive(Default)]
pub struct RtpPcmaPay;

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmaPay {
    const NAME: &'static str = "GstRtpPcmaPay2";
    type Type = super::RtpPcmaPay;
    type ParentType = super::RtpPcmauPay;
}

impl ObjectImpl for RtpPcmaPay {}

impl GstObjectImpl for RtpPcmaPay {}

impl ElementImpl for RtpPcmaPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP PCMA Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload A-law Audio into RTP packets (RFC 3551)",
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
                &gst::Caps::builder("audio/x-alaw")
                    .field("channels", 1i32)
                    .field("rate", gst::IntRange::new(1i32, i32::MAX))
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
                            .field("media", "audio")
                            .field("payload", 8i32)
                            .field("clock-rate", 8000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", "PCMA")
                            .field("clock-rate", gst::IntRange::new(1, i32::MAX))
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

impl crate::basepay::RtpBasePay2Impl for RtpPcmaPay {
    const DEFAULT_PT: u8 = 8;
}

impl crate::baseaudiopay::RtpBaseAudioPay2Impl for RtpPcmaPay {}

impl super::RtpPcmauPayImpl for RtpPcmaPay {}

/**
 * SECTION:element-rtppcmupay2
 * @see_also: rtppcmudepay2, rtppcmapay2, rtppcmadepay2, mulawenc, mulawdec
 *
 * Payloads µ-law encoded audio into RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! audio/x-raw,rate=8000,channels=1 ! mulawenc ! rtppcmupay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will generate a µ-law audio test signal and payload it as RTP and send it out
 * as UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */

#[derive(Default)]
pub struct RtpPcmuPay;

#[glib::object_subclass]
impl ObjectSubclass for RtpPcmuPay {
    const NAME: &'static str = "GstRtpPcmuPay2";
    type Type = super::RtpPcmuPay;
    type ParentType = super::RtpPcmauPay;
}

impl ObjectImpl for RtpPcmuPay {}

impl GstObjectImpl for RtpPcmuPay {}

impl ElementImpl for RtpPcmuPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP PCMU Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload µ-law Audio into RTP packets (RFC 3551)",
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
                &gst::Caps::builder("audio/x-mulaw")
                    .field("channels", 1i32)
                    .field("rate", gst::IntRange::new(1i32, i32::MAX))
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
                            .field("media", "audio")
                            .field("payload", 0i32)
                            .field("clock-rate", 8000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", "PCMU")
                            .field("clock-rate", gst::IntRange::new(1, i32::MAX))
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

impl crate::basepay::RtpBasePay2Impl for RtpPcmuPay {
    const DEFAULT_PT: u8 = 0;
}

impl crate::baseaudiopay::RtpBaseAudioPay2Impl for RtpPcmuPay {}

impl super::RtpPcmauPayImpl for RtpPcmuPay {}
