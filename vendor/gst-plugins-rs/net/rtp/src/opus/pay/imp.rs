// GStreamer RTP Opus Payloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpopuspay2
 * @see_also: rtpopusdepay2, rtpopuspay, rtpopusdepay, opusdec, opusenc
 *
 * Payloads an Opus audio stream into RTP packets as per [RFC 7587][rfc-7587] or
 * [libwebrtc's multiopus extension][libwebrtc-multiopus].
 *
 * The multi-channel extension adds extra fields to the output caps and the SDP in line with
 * what libwebrtc expects, e.g.
 * |[
 *  a=rtpmap:96 multiopus/48000/6
 *  a=fmtp:96 num_streams=4;coupled_streams=2;channel_mapping=0,4,1,2,3,5
 * ]|
 * for 5.1 surround sound audio.
 *
 * [rfc-7587]: https://www.rfc-editor.org/rfc/rfc7587.html
 * [libwebrtc-multiopus]: https://webrtc-review.googlesource.com/c/src/+/129768
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! audio/x-raw,channels=2 ! opusenc ! rtpopuspay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will encode and audio test signal as Opus audio and payload it as RTP and send it out
 * over UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basepay::{RtpBasePay2Ext, RtpBasePay2Impl, RtpBasePay2ImplExt};

use std::sync::atomic::AtomicBool;

struct State {
    marker_pending: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            marker_pending: true,
        }
    }
}

impl State {
    fn marker_pending(&mut self) -> bool {
        std::mem::replace(&mut self.marker_pending, false)
    }
}

const DEFAULT_DTX: bool = false;

#[derive(Default)]
struct Settings {
    dtx: AtomicBool,
}

#[derive(Default)]
pub struct RtpOpusPay {
    // Streaming state
    state: AtomicRefCell<State>,

    // Settings
    settings: Settings,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpopuspay2",
        gst::DebugColorFlags::empty(),
        Some("RTP Opus Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpOpusPay {
    const NAME: &'static str = "GstRtpOpusPay2";
    type Type = super::RtpOpusPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpOpusPay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("dtx")
                .nick("Discontinuous Transmission")
                .blurb("Do not send out empty packets for transmission (requires opusenc dtx=true)")
                .default_value(DEFAULT_DTX)
                .mutable_playing()
                .build()]
        });

        PROPERTIES.as_ref()
    }
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "dtx" => self.settings.dtx.store(
                value.get().expect("type checked upstream"),
                std::sync::atomic::Ordering::Relaxed,
            ),
            name => unimplemented!("Property '{name}'"),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "dtx" => self
                .settings
                .dtx
                .load(std::sync::atomic::Ordering::Relaxed)
                .to_value(),
            name => unimplemented!("Property '{name}'"),
        }
    }
}

impl GstObjectImpl for RtpOpusPay {}

impl ElementImpl for RtpOpusPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP Opus Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an Opus audio stream into RTP packets (RFC 7587)",
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
                        gst::Structure::builder("audio/x-opus")
                            .field("channel-mapping-family", 0i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/x-opus")
                            .field("channel-mapping-family", 0i32)
                            .field("channels", gst::IntRange::new(1i32, 2i32))
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/x-opus")
                            .field("channel-mapping-family", 1i32)
                            .field("channels", gst::IntRange::new(3i32, 255i32))
                            .build(),
                    )
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
                            .field("encoding-name", gst::List::new(["OPUS", "MULTIOPUS"]))
                            .field("clock-rate", 48000i32)
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
impl RtpBasePay2Impl for RtpOpusPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let mut src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "audio")
            .field("clock-rate", 48000i32);

        let s = caps.structure(0).unwrap();

        let channels_field = s.get::<i32>("channels").ok();
        let rate_field = s.get::<i32>("rate").ok();

        let channel_mapping_family = s.get::<i32>("channel-mapping-family").unwrap();

        let encoding_name = match channel_mapping_family {
            // Normal Opus, mono or stereo
            0 => {
                if channels_field == Some(1) {
                    src_caps = src_caps.field("sprop-stereo", "0");
                } else {
                    src_caps = src_caps.field("sprop-stereo", "1");
                };

                "OPUS"
            }

            // MULTIOPUS mapping is a Google libwebrtc concoction, see
            // https://webrtc-review.googlesource.com/c/src/+/129768
            //
            // Stereo and Mono must always be payloaded using the normal OPUS mapping,
            // so this is only for multi-channel Opus.
            1 => {
                if let Ok(stream_count) = s.get::<i32>("stream-count") {
                    src_caps = src_caps.field("num_streams", stream_count.to_string());
                }

                if let Ok(coupled_count) = s.get::<i32>("coupled-count") {
                    src_caps = src_caps.field("coupled_streams", coupled_count.to_string());
                }

                if let Ok(channel_mapping) = s.get::<gst::ArrayRef>("channel-mapping") {
                    let comma_separated_channel_nums = {
                        let res = channel_mapping
                            .iter()
                            .map(|v| v.get::<i32>().map(|i| i.to_string()))
                            .collect::<Result<Vec<_>, _>>();

                        // Can't use .collect().map_err()? because it doesn't work for funcs with bool returns
                        match res {
                            Err(_) => {
                                gst::error!(
                                    CAT,
                                    imp = self,
                                    "Invalid 'channel-mapping' field types"
                                );
                                return false;
                            }
                            Ok(num_strings) => num_strings.join(","),
                        }
                    };
                    src_caps = src_caps.field("channel_mapping", comma_separated_channel_nums);
                }

                "MULTIOPUS"
            }

            _ => unreachable!(),
        };

        let channels = channels_field.unwrap_or(2);

        src_caps = src_caps
            .field("encoding-name", encoding_name)
            .field("encoding-params", channels.to_string());

        if let Some(rate) = rate_field {
            src_caps = src_caps.field("sprop-maxcapturerate", rate.to_string());
        }

        self.obj().set_src_caps(&src_caps.build());

        true
    }

    // https://www.rfc-editor.org/rfc/rfc7587.html#section-4.2
    //
    // We just payload whatever the Opus encoder gives us, ptime constraints and
    // such will have to be configured on the encoder side.
    //
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        let data = map.as_slice();

        let dtx = self.settings.dtx.load(std::sync::atomic::Ordering::Relaxed);

        // Don't output DTX packets if discontinuous transmission was enabled (in encoder and here)
        // (Although seeing that it's opt-in in the encoder already one wonders whether we
        // shouldn't just do it automatically here)
        //
        // Even in DTX mode there will still be a non-DTX packet going through every 400ms.
        if dtx && data.len() <= 2 {
            gst::log!(
                CAT,
                imp = self,
                "Not sending out empty DTX packet {:?}",
                buffer
            );
            // The first non-DTX packet will be the start of a talkspurt
            state.marker_pending = true;
            self.obj().drop_buffers(..=id);
            return Ok(gst::FlowSuccess::Ok);
        }

        let marker_pending = state.marker_pending();

        self.obj().queue_packet(
            id.into(),
            rtp_types::RtpPacketBuilder::new()
                .payload(data)
                .marker_bit(marker_pending),
        )
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

                let rtp_opus_caps = gst::Caps::builder("application/x-rtp")
                    .field("encoding-name", "OPUS")
                    .build();

                let rtp_multiopus_caps = gst::Caps::builder("application/x-rtp")
                    .field("encoding-name", "MULTIOPUS")
                    .build();

                // Baseline: sink pad template caps (normal opus and multi-channel opus)
                let mut ret_caps = self.obj().sink_pad().pad_template_caps();

                // Downstream doesn't support plain opus?
                // Only multi-channel opus options left then
                if !peer_caps.can_intersect(&rtp_opus_caps) {
                    ret_caps = gst::Caps::builder("audio/x-opus")
                        .field("channel-mapping-family", 1i32)
                        .field("channels", gst::IntRange::new(3i32, 255i32))
                        .build();
                }

                // Downstream doesn't support multi-channel opus?
                // Only mono/stereo Opus left then
                if !peer_caps.can_intersect(&rtp_multiopus_caps) {
                    ret_caps = gst::Caps::builder("audio/x-opus")
                        .field("channel-mapping-family", 0i32)
                        .field("channels", gst::IntRange::new(1i32, 2i32))
                        .build();
                }

                // If downstream has a preference re. mono/stereo, try to express that
                // in the returned caps by appending a first structure with the preference

                let s = ret_caps.structure(0).unwrap();

                if s.get::<i32>("channel-mapping-family") == Ok(0) {
                    let peer_s = peer_caps.structure(0).unwrap();

                    gst::trace!(CAT, imp = self, "Peer preference structure: {peer_s}");

                    let pref_chans = peer_s
                        .get::<&str>("stereo")
                        .ok()
                        .and_then(|params| params.trim().parse::<i32>().ok())
                        .map(|v| match v {
                            0 => 1, // mono
                            1 => 2, // stereo
                            _ => {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Unexpected stereo value {v} in peer caps {s}"
                                );
                                2 // default is stereo
                            }
                        });

                    if let Some(pref_chans) = pref_chans {
                        gst::trace!(CAT, imp = self, "Peer preference: channels={pref_chans}");

                        let mut pref_caps = gst::Caps::builder("audio/x-opus")
                            .field("channel-mapping-family", 0i32)
                            .field("channels", pref_chans)
                            .build();
                        pref_caps.merge(ret_caps);
                        ret_caps = pref_caps;
                    }
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

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }
}

impl RtpOpusPay {}
