// GStreamer RTP L8 / L16 / L24 linear raw audio payloader
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;

use gst::{glib, prelude::*, subclass::prelude::*};
use gst_audio::{AudioCapsBuilder, AudioChannelPosition, AudioFormat};

use std::sync::LazyLock;

use std::num::NonZeroU32;

use crate::{
    baseaudiopay::{RtpBaseAudioPay2Ext, RtpBaseAudioPay2Impl},
    basepay::{RtpBasePay2Ext, RtpBasePay2ImplExt},
};

use crate::linear_audio::common::channel_positions;

#[derive(Default)]
pub struct RtpLinearAudioPay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    width: Option<NonZeroU32>,
    channel_reorder_map: Option<Vec<usize>>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtplinearaudiopay",
        gst::DebugColorFlags::empty(),
        Some("RTP L8/L16/L24 Raw Audio Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpLinearAudioPay {
    const NAME: &'static str = "GstRtpLinearAudioPay";
    type Type = super::RtpLinearAudioPay;
    type ParentType = crate::baseaudiopay::RtpBaseAudioPay2;
}

impl ObjectImpl for RtpLinearAudioPay {}

impl GstObjectImpl for RtpLinearAudioPay {}

impl ElementImpl for RtpLinearAudioPay {}

impl crate::basepay::RtpBasePay2Impl for RtpLinearAudioPay {
    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let Ok(info) = gst_audio::AudioInfo::from_caps(caps) else {
            gst::error!(
                CAT,
                imp = self,
                "Can't parse input caps {caps} into audio info"
            );
            return false;
        };

        gst::info!(CAT, imp = self, "Got caps, audio info: {info:?}");

        let encoding_name = match info.format() {
            AudioFormat::U8 => "L8",
            AudioFormat::S16be => "L16", // and/or pt 10/11
            AudioFormat::S24be => "L24",
            _ => unreachable!(), // Input caps will have been checked against template caps
        };

        let n_channels = info.channels();
        let rate = info.rate();

        // pt 10 = L16 stereo @ 44.1kHz, pt 11 = L16 mono @ 44.1kHz
        let prop_pt = self.obj().property::<u32>("pt");

        if prop_pt == 10 && (n_channels != 2 || rate != 44100 || encoding_name != "L16") {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Static payload type 10 is reserved for stereo 16-bit audio @ 44100 Hz"]
            );
            return false;
        }

        if prop_pt == 11 && (n_channels != 1 || rate != 44100 || encoding_name != "L16") {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Static payload type 11 is reserved for mono 16-bit audio @ 44100 Hz"]
            );
            return false;
        }

        let mut src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "audio")
            .field("encoding-name", encoding_name)
            .field("clock-rate", rate as i32)
            .field("channels", n_channels as i32)
            .field("encoding-params", info.channels().to_string());

        let mut reorder_map = None;

        // Figure out channel order for multi-channel audio and if channel reordering is required
        if n_channels > 2 {
            if let Some(positions) = info.positions() {
                match channel_positions::find_channel_order_from_positions(positions) {
                    Some(name) => {
                        gst::info!(
                            CAT,
                            imp = self,
                            "Using {name} channel order mapping for {n_channels} channels"
                        );

                        if name != "default" {
                            src_caps = src_caps.field("channel-order", name);
                        }

                        let rtp_positions =
                            channel_positions::get_channel_order(Some(name), n_channels as i32)
                                .unwrap();

                        let mut gst_positions = rtp_positions.to_vec();

                        // Re-order channel positions according to GStreamer conventions. This should always
                        // succeed because the input channel positioning comes from internal tables.
                        AudioChannelPosition::positions_to_valid_order(&mut gst_positions).unwrap();

                        // Is channel re-ordering actually required?
                        if rtp_positions != gst_positions {
                            let mut map = vec![0usize; n_channels as usize];

                            gst_audio::channel_reorder_map(&gst_positions, rtp_positions, &mut map)
                                .unwrap();

                            gst::info!(
                                CAT,
                                imp = self,
                                "Channel positions (GStreamer) : {gst_positions:?}"
                            );
                            gst::info!(
                                CAT,
                                imp = self,
                                "Channel positions (RTP)       : {rtp_positions:?}"
                            );
                            gst::info!(CAT, imp = self, "Channel reorder map           : {map:?}");

                            reorder_map = Some(map);
                        }
                    }
                    _ => {
                        gst::element_imp_warning!(
                            self,
                            gst::StreamError::Encode,
                            ["Couldn't find canonical channel order mapping for {positions:?}"]
                        );
                    }
                }
            }
        }

        self.obj().set_src_caps(&src_caps.build());

        let mut state = self.state.borrow_mut();
        state.width = NonZeroU32::new(info.width());
        state.channel_reorder_map = reorder_map;
        self.obj().set_bpf(info.bpf() as usize);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
    //
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut buffer = buffer.clone();

        let state = self.state.borrow_mut();

        // Re-order channels from GStreamer layout to RTP layout if needed
        if let Some(reorder_map) = &state.channel_reorder_map {
            let buffer_ref = buffer.make_mut();

            let width = state.width.expect("width").get();

            type I24 = [u8; 3];

            match width {
                8 => channel_positions::reorder_channels::<u8>(buffer_ref, reorder_map)?,
                16 => channel_positions::reorder_channels::<i16>(buffer_ref, reorder_map)?,
                24 => channel_positions::reorder_channels::<I24>(buffer_ref, reorder_map)?,
                _ => unreachable!(),
            }
        }

        self.parent_handle_buffer(&buffer, id)
    }

    #[allow(clippy::single_match)]
    fn sink_query(&self, query: &mut gst::QueryRef) -> bool {
        match query.view_mut() {
            gst::QueryViewMut::Caps(query) => {
                let src_tmpl_caps = self.obj().src_pad().pad_template_caps();

                let peer_caps = self.obj().src_pad().peer_query_caps(Some(&src_tmpl_caps));

                if peer_caps.is_empty() {
                    query.set_result(&peer_caps);
                    return true;
                }

                // Baseline: sink pad template caps
                let mut ret_caps = self.obj().sink_pad().pad_template_caps();

                let format = ret_caps
                    .structure(0)
                    .unwrap()
                    .get::<&str>("format")
                    .unwrap();

                // If downstream has restrictions re. sample rate or number of channels,
                // proxy that upstream (we assume the restriction is a single fixed value
                // and not something fancy like a list or array of values).

                let peer_s = peer_caps.structure(0).unwrap();

                let (implied_channels, implied_rate): (Option<i32>, Option<i32>) = {
                    let peer_pt = peer_s.get::<i32>("payload").ok().filter(|&v| v > 0);
                    let prop_pt = self.obj().property::<u32>("pt");

                    // pt 10 = L16 stereo @ 44.1kHz, pt 11 = L16 mono @ 44.1kHz
                    match (peer_pt, prop_pt) {
                        (Some(10), _) | (_, 10) => {
                            if format == "S16BE" {
                                (Some(2), Some(44100))
                            } else {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "pt 10 only supported for S16BE/L16!"
                                );
                                query.set_result(&gst::Caps::new_empty());
                                return true;
                            }
                        }
                        (Some(11), _) | (_, 11) => {
                            if format == "S16BE" {
                                (Some(1), Some(44100))
                            } else {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "pt 10 only supported for S16BE/L16!"
                                );
                                query.set_result(&gst::Caps::new_empty());
                                return true;
                            }
                        }
                        _ => (None, None),
                    }
                };

                let peer_rate = peer_s.get::<i32>("clock-rate").ok().filter(|&r| r > 0);

                // We're strict and enforce the implied 44100Hz requirement for pt=10/11
                if let Some(pref_rate) = implied_rate.or(peer_rate) {
                    let caps = ret_caps.make_mut();
                    caps.set("rate", pref_rate);
                }

                let peer_chans = {
                    let encoding_params = peer_s
                        .get::<&str>("encoding-params")
                        .ok()
                        .and_then(|params| params.parse::<i32>().ok())
                        .filter(|&v| v > 0);

                    let channels = peer_s.get::<i32>("channels").ok().filter(|&v| v > 0);

                    encoding_params.or(channels)
                };

                // We're strict and enforce the stereo/mono channel requirement for pt=10/11
                if let Some(pref_chans) = implied_channels.or(peer_chans) {
                    let caps = ret_caps.make_mut();
                    caps.set("channels", pref_chans);
                }

                if let Some(filter) = query.filter() {
                    ret_caps = ret_caps.intersect_with_mode(filter, gst::CapsIntersectMode::First);
                }

                query.set_result(&ret_caps);

                return true;
            }

            _ => (),
        }

        self.parent_sink_query(query)
    }
}

impl RtpBaseAudioPay2Impl for RtpLinearAudioPay {}

impl RtpLinearAudioPay {}

trait RtpLinearAudioPayImpl: RtpBaseAudioPay2Impl {}

unsafe impl<T: RtpLinearAudioPayImpl> IsSubclassable<T> for super::RtpLinearAudioPay {}

/**
 * SECTION:element-rtpL8pay2
 * @see_also: rtpL8depay2, rtpL16pay2, rtpL24pay2, rtpL8pay
 *
 * Payloads raw 8-bit audio into RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! rtpL8pay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will generate an 8-bit raw audio test signal and payload it as RTP and send it out
 * as UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL8Pay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL8Pay {
    const NAME: &'static str = "GstRtpL8Pay2";
    type Type = super::RtpL8Pay;
    type ParentType = super::RtpLinearAudioPay;
}

impl ObjectImpl for RtpL8Pay {}

impl GstObjectImpl for RtpL8Pay {}

impl ElementImpl for RtpL8Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 8-bit Raw Audio Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload 8-bit raw audio (L8) into RTP packets (RFC 3551)",
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
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::U8)
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
                            .field("encoding-name", "L8")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
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

impl crate::basepay::RtpBasePay2Impl for RtpL8Pay {}

impl RtpLinearAudioPayImpl for RtpL8Pay {}

impl RtpBaseAudioPay2Impl for RtpL8Pay {}

/**
 * SECTION:element-rtpL16pay2
 * @see_also: rtpL16depay2, rtpL8pay2, rtpL24pay2, rtpL16pay
 *
 * Payloads raw 16-bit audio into RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.11
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! rtpL16pay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will generate an 16-bit raw audio test signal and payload it as RTP and send it out
 * as UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL16Pay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL16Pay {
    const NAME: &'static str = "GstRtpL16Pay2";
    type Type = super::RtpL16Pay;
    type ParentType = super::RtpLinearAudioPay;
}

impl ObjectImpl for RtpL16Pay {}

impl GstObjectImpl for RtpL16Pay {}

impl ElementImpl for RtpL16Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 16-bit Raw Audio Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload 16-bit raw audio (L16) into RTP packets (RFC 3551)",
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
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::S16be)
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
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "L16")
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("clock-rate", 44100i32)
                            .field("payload", gst::List::new([10i32, 11]))
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

impl crate::basepay::RtpBasePay2Impl for RtpL16Pay {}

impl RtpLinearAudioPayImpl for RtpL16Pay {}

impl RtpBaseAudioPay2Impl for RtpL16Pay {}

/**
 * SECTION:element-rtpL24pay2
 * @see_also: rtpL24depay2, rtpL8pay2, rtpL16pay2, rtpL24pay
 *
 * Payloads raw 24-bit audio into RTP packets as per [RFC 3551][rfc-3551] and
 * [RFC 3190][rfc-3190].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.11
 * [rfc-3190]: https://www.rfc-editor.org/rfc/rfc3190.html#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! audioconvert ! rtpL24pay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will generate a 24-bit raw audio test signal and payload it as RTP and send it out
 * as UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL24Pay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL24Pay {
    const NAME: &'static str = "GstRtpL24Pay2";
    type Type = super::RtpL24Pay;
    type ParentType = super::RtpLinearAudioPay;
}

impl ObjectImpl for RtpL24Pay {}

impl GstObjectImpl for RtpL24Pay {}

impl ElementImpl for RtpL24Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 24-bit Raw Audio Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload 24-bit raw audio (L24) into RTP packets (RFC 3551)",
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
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::S24be)
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
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "L24")
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

impl crate::basepay::RtpBasePay2Impl for RtpL24Pay {}

impl RtpLinearAudioPayImpl for RtpL24Pay {}

impl RtpBaseAudioPay2Impl for RtpL24Pay {}

#[cfg(test)]
mod tests {
    use byte_slice_cast::*;
    use gst_check::Harness;

    // Same test as in the depayloader, just in reverse for the payloader
    #[test]
    fn test_channel_reorder_l8() {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtp plugin");

        let mut h = Harness::new("rtpL8pay2");
        h.play();

        use gst_audio::AudioChannelPosition::*;
        let pos = &[
            FrontLeft,
            FrontRight,
            FrontCenter,
            RearCenter,
            SideLeft,
            SideRight,
        ];
        let mask = gst_audio::AudioChannelPosition::positions_to_mask(pos, true).unwrap();

        let input_caps = gst_audio::AudioCapsBuilder::new_interleaved()
            .format(gst_audio::AudioFormat::U8)
            .rate(48000)
            .channels(6)
            .channel_mask(mask)
            .build();

        h.set_src_caps(input_caps);

        let input_data = [1u8, 2, 3, 4, 5, 6, 11, 12, 13, 14, 15, 16];
        let mut buf = gst::Buffer::from_slice(input_data);
        buf.get_mut().unwrap().set_pts(gst::ClockTime::ZERO);
        h.push(buf).unwrap();
        h.push_event(gst::event::Eos::new());

        let outbuf = h.pull().unwrap();

        let out_map = outbuf.map_readable().unwrap();
        let out_data = out_map.as_slice_of::<u8>().unwrap();

        let packet = rtp_types::RtpPacket::parse(out_data).unwrap();
        let out_data = packet.payload();

        // input:   [ 1,  2,  3,  4,  5,  6 | 11, 12, 13, 14, 15, 16]
        //          @ FrontLeft, FrontRight, FrontCenter, RearCenter, SideLeft, SideRight
        //
        // output:  [ 1,  2,  5,  6,  3,  4, | 11, 12, 15, 16, 13, 14]
        //          @ FrontLeft, FrontRight, SideLeft, SideRight, FrontCenter, RearCenter
        assert_eq!(out_data, [1, 2, 5, 6, 3, 4, 11, 12, 15, 16, 13, 14]);
    }
}
