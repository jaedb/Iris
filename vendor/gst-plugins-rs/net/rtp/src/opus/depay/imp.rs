// GStreamer RTP Opus Depayloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpopusdepay2
 * @see_also: rtpopuspay2, rtpopuspay, rtpopusdepay, opusdec, opusenc
 *
 * Extracts an Opus audio stream from RTP packets as per [RFC 7587][rfc-7587] or libwebrtc's
 * multiopus extension.
 *
 * [rfc-7587]: https://www.rfc-editor.org/rfc/rfc7587.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=48000, encoding-name=OPUS, encoding-params=(string)2, sprop-stereo=(string)1, payload=96' ! rtpjitterbuffer latency=50 ! rtpopusdepay2 ! opusdec ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP Opus audio stream. You can use the #opusenc and
 * #rtpopuspay2 elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{RtpBaseDepay2Ext, RtpBaseDepay2Impl};

#[derive(Default)]
pub struct RtpOpusDepay {}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpopusdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP Opus Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpOpusDepay {
    const NAME: &'static str = "GstRtpOpusDepay2";
    type Type = super::RtpOpusDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpOpusDepay {}

impl GstObjectImpl for RtpOpusDepay {}

impl ElementImpl for RtpOpusDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP Opus Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an Opus audio stream from RTP packets (RFC 7587)",
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
                        // Note: not advertising X-GST-OPUS-DRAFT-SPITTKA-00 any longer
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", gst::List::new(["OPUS", "MULTIOPUS"]))
                            .field("clock-rate", 48000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/x-opus")
                    .field("channel-mapping-family", gst::IntRange::new(0i32, 1i32))
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}
impl RtpBaseDepay2Impl for RtpOpusDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let encoding_name = s.get::<&str>("encoding-name").unwrap();

        let res = match encoding_name {
            "OPUS" => self.handle_sink_caps_opus(s),
            "MULTIOPUS" => self.handle_sink_caps_multiopus(s),
            _ => unreachable!(),
        };

        let Ok(src_caps) = res else {
            gst::warning!(
                CAT,
                imp = self,
                "Failed to parse {encoding_name} RTP input caps {s}: {}",
                res.unwrap_err()
            );
            return false;
        };

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc7587.html
    //
    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // Parse frames in Opus packet to figure out duration
        let duration = self.parse_opus_packet(packet.payload());

        let mut outbuf = packet.payload_buffer();
        let outbuf_ref = outbuf.get_mut().unwrap();

        if let Some(duration) = duration {
            outbuf_ref.set_duration(duration);
        }

        // Mark start of talkspurt with RESYNC flag
        if packet.marker_bit() {
            outbuf_ref.set_flags(gst::BufferFlags::RESYNC);
        }

        gst::trace!(CAT, imp = self, "Finishing buffer {outbuf:?}");

        self.obj().queue_buffer(packet.into(), outbuf)
    }
}

// Default is mono according to the RFC, but we still default to stereo here since there's
// no guarantee that it will always be mono then, and the stream may switch between stereo
// and mono at any time, so stereo is a slightly better default (even if possible more expensive
// in terms of processing overhead down the line), and it's always safe to downmix to mono.
const DEFAULT_CHANNELS: i32 = 2;

impl RtpOpusDepay {
    // Opus Media Type Registration:
    // https://www.rfc-editor.org/rfc/rfc7587.html#section-6.1
    //
    fn handle_sink_caps_opus(&self, s: &gst::StructureRef) -> Result<gst::Caps, &'static str> {
        let channels = s
            .get::<&str>("sprop-stereo")
            .ok()
            .and_then(|params| params.trim().parse::<i32>().ok())
            .map(|v| match v {
                0 => 1, // mono
                1 => 2, // stereo
                _ => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Unexpected sprop-stereo value {v} in input caps {s}"
                    );
                    DEFAULT_CHANNELS
                }
            })
            .unwrap_or(DEFAULT_CHANNELS);

        let rate = s
            .get::<&str>("sprop-maxcapturerate")
            .ok()
            .and_then(|params| params.trim().parse::<i32>().ok())
            .filter(|&v| v > 0 && v <= 48000)
            .unwrap_or(48000);

        let src_caps = gst::Caps::builder("audio/x-opus")
            .field("channel-mapping-family", 0i32)
            .field("channels", channels)
            .field("rate", rate)
            .build();

        Ok(src_caps)
    }

    // MULTIOPUS mapping is a Google libwebrtc concoction, see
    // https://webrtc-review.googlesource.com/c/src/+/129768
    //
    fn handle_sink_caps_multiopus(&self, s: &gst::StructureRef) -> Result<gst::Caps, &'static str> {
        let channels = s
            .get::<&str>("encoding-params")
            .map_err(|_| "Missing 'encoding-params' field")?
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&v| v > 0 && v <= 255)
            .ok_or("Invalid 'encoding-params' field")?;

        let num_streams = s
            .get::<&str>("num_streams")
            .map_err(|_| "Missing 'num_streams' field")?
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&v| v > 0 && v <= channels)
            .ok_or("Invalid 'num_streams' field")?;

        let coupled_streams = s
            .get::<&str>("coupled_streams")
            .map_err(|_| "Missing 'coupled_streams' field")?
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&v| v > 0 && v <= num_streams)
            .ok_or("Invalid 'coupled_streams' field")?;

        let channel_mapping: Vec<_> = s
            .get::<&str>("channel_mapping")
            .map_err(|_| "Missing 'channel_mapping' field")?
            .split(',')
            .map(|p| p.trim().parse::<i32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid 'channel_mapping' field")?;

        let src_caps = gst::Caps::builder("audio/x-opus")
            .field("channel-mapping-family", 1i32)
            .field("stream-count", num_streams)
            .field("coupled-count", coupled_streams)
            .field("channel-mapping", gst::Array::new(channel_mapping))
            .field("channels", channels)
            .field("rate", 48000i32)
            .build();

        Ok(src_caps)
    }

    // https://www.rfc-editor.org/rfc/rfc6716#section-3
    // (Note: bit numbering in diagram has bit 0 as the highest bit and bit 7 as the lowest.)
    //
    fn parse_opus_packet(&self, data: &[u8]) -> Option<gst::ClockTime> {
        if data.is_empty() {
            return gst::ClockTime::NONE;
        }

        let toc = data[0];
        let config = (toc >> 3) & 0x1f;

        let frame_duration_usecs = match config {
            // Silk NB / MB / WB
            0 | 4 | 8 => 10_000,
            1 | 5 | 9 => 20_000,
            2 | 6 | 10 => 40_000,
            3 | 7 | 11 => 60_000,
            // Hybrid SWB / FB
            12 | 14 => 10_000,
            13 | 15 => 20_000,
            // CELT NB / WB / SWB / FB
            16 | 20 | 24 | 28 => 2_500,
            17 | 21 | 25 | 29 => 5_000,
            18 | 22 | 26 | 30 => 10_000,
            19 | 23 | 27 | 31 => 20_000,
            _ => unreachable!(),
        };

        let frame_duration = gst::ClockTime::from_useconds(frame_duration_usecs);

        let n_frames = match toc & 0b11 {
            0 => 1,
            1 => 2,
            3 => {
                if data.len() < 2 {
                    return gst::ClockTime::NONE;
                }
                data[1] & 0b0011_1111
            }
            _ => unreachable!(),
        } as u64;

        let duration = frame_duration * n_frames;

        if duration > gst::ClockTime::from_mseconds(120) {
            gst::warning!(
                CAT,
                imp = self,
                "Opus packet with frame duration {duration:?} > 120ms"
            );
            return gst::ClockTime::NONE;
        }

        Some(duration)
    }
}
