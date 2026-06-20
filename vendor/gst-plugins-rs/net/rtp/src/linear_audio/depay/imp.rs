// GStreamer RTP L8 / L16 / L24 linear raw audio depayloader
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
use gst_audio::{AudioCapsBuilder, AudioChannelPosition, AudioFormat, AudioInfo, AudioLayout};

use std::sync::LazyLock;

use std::num::NonZeroU32;

use crate::basedepay::{RtpBaseDepay2Ext, RtpBaseDepay2ImplExt};

use crate::linear_audio::common::channel_positions;

#[derive(Default)]
pub struct RtpLinearAudioDepay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    clock_rate: Option<NonZeroU32>,
    bpf: Option<NonZeroU32>,
    width: Option<NonZeroU32>,
    channel_reorder_map: Option<Vec<usize>>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtplinearaudiodepay",
        gst::DebugColorFlags::empty(),
        Some("RTP L8/L16/L24 Raw Audio Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpLinearAudioDepay {
    const NAME: &'static str = "GstRtpLinearAudioDepay";
    type Type = super::RtpLinearAudioDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpLinearAudioDepay {}

impl GstObjectImpl for RtpLinearAudioDepay {}

impl ElementImpl for RtpLinearAudioDepay {}

impl crate::basedepay::RtpBaseDepay2Impl for RtpLinearAudioDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let pt = s.get::<i32>("payload").ok().filter(|&r| r > 0);
        let encoding_name = s.get::<&str>("encoding-name").ok();

        // pt 10 = L16 stereo, pt 11 = L16 mono
        let (implied_clock_rate, implied_channels) = match pt {
            Some(10) => (Some(44100), Some(2)),
            Some(11) => (Some(44100), Some(1)),
            _ => (None, None),
        };

        if (pt == Some(10) || pt == Some(11))
            && encoding_name.is_some()
            && encoding_name != Some("L16")
        {
            self.post_error_message(gst::error_msg!(
                gst::StreamError::Format,
                [
                    "pt 10-11 require encoding-name=L16 but found {}",
                    encoding_name.unwrap()
                ]
            ));
            return false;
        }

        let mut state = self.state.borrow_mut();

        // We currently require a clock-rate in the template caps, since rtpbin/rtpjitterbuffer
        // won't work well without one, so the implied fallback case can't actually be triggered
        // at the moment. Keeping the code around for now in case we find a use case where it
        // makes sense to allow it in future.
        let clock_rate = s
            .get::<i32>("clock-rate")
            .ok()
            .filter(|&r| r > 0)
            .or(implied_clock_rate)
            .unwrap();

        state.clock_rate = NonZeroU32::new(clock_rate as u32);

        let audio_format = match encoding_name {
            Some("L8") => AudioFormat::U8,
            Some("L16") => AudioFormat::S16be,
            Some("L24") => AudioFormat::S24be,
            None => AudioFormat::S16be, // pt 10/11
            _ => unreachable!(),        // Input caps will have been checked against template caps
        };

        let n_channels = {
            let encoding_params = s
                .get::<&str>("encoding-params")
                .ok()
                .and_then(|params| params.parse::<i32>().ok())
                .filter(|&v| v > 0);

            let channels = s
                .get::<&str>("channels")
                .ok()
                .and_then(|chans| chans.parse::<i32>().ok())
                .filter(|&v| v > 0);

            let channels = channels.or(s.get::<i32>("channels").ok().filter(|&v| v > 0));

            encoding_params
                .or(channels)
                .or(implied_channels)
                .unwrap_or(1i32)
        };

        if pt == Some(10) && n_channels != 2 {
            self.post_error_message(gst::error_msg!(
                gst::StreamError::Format,
                ["pt 10 implies stereo but found {n_channels} channels specified"]
            ));
            return false;
        }

        if pt == Some(11) && n_channels != 1 {
            self.post_error_message(gst::error_msg!(
                gst::StreamError::Format,
                ["pt 11 implies mono but found {n_channels} channels specified"]
            ));
            return false;
        }

        let channel_order_name = s.get::<&str>("channel-order").ok();

        let order = channel_positions::get_channel_order(channel_order_name, n_channels);

        let gst_positions = if let Some(rtp_positions) = order {
            let mut channel_positions = rtp_positions.to_vec();

            // Re-order channel positions according to GStreamer conventions. This should always
            // succeed because the input channel positioning comes from internal tables.
            AudioChannelPosition::positions_to_valid_order(&mut channel_positions).unwrap();

            // Is channel re-ordering actually required?
            if rtp_positions != channel_positions {
                let mut reorder_map = vec![0usize; n_channels as usize];

                gst_audio::channel_reorder_map(rtp_positions, &channel_positions, &mut reorder_map)
                    .unwrap();

                gst::info!(
                    CAT,
                    imp = self,
                    "Channel positions (RTP)       : {rtp_positions:?}"
                );
                gst::info!(
                    CAT,
                    imp = self,
                    "Channel positions (GStreamer) : {channel_positions:?}"
                );
                gst::info!(
                    CAT,
                    imp = self,
                    "Channel reorder map           : {reorder_map:?}"
                );

                state.channel_reorder_map = Some(reorder_map);
            }

            channel_positions
        } else {
            vec![AudioChannelPosition::None; n_channels as usize]
        };

        let audio_info = AudioInfo::builder(audio_format, clock_rate as u32, n_channels as u32)
            .layout(AudioLayout::Interleaved)
            .positions(&gst_positions)
            .build()
            .unwrap();

        state.bpf = NonZeroU32::new(audio_info.bpf());
        state.width = NonZeroU32::new(audio_info.width());

        let src_caps = audio_info.to_caps().unwrap();

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let state = self.state.borrow();

        let clock_rate = state.clock_rate.expect("clock-rate").get();
        let bpf = state.bpf.expect("bpf").get();

        if packet.payload().is_empty() {
            gst::warning!(CAT, imp = self, "Empty packet {packet:?}, dropping");
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        if packet.payload().len() % (bpf as usize) != 0 {
            gst::warning!(
                CAT,
                imp = self,
                "Wrong payload size: expected multiples of {bpf}, but have {}",
                packet.payload().len()
            );
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        let mut buffer = packet.payload_buffer();

        let buffer_ref = buffer.get_mut().unwrap();

        buffer_ref.set_duration(
            (buffer_ref.size() as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, bpf as u64 * clock_rate as u64)
                .map(gst::ClockTime::from_nseconds),
        );

        // Re-order channels from RTP layout to GStreamer layout if needed
        if let Some(reorder_map) = &state.channel_reorder_map {
            let width = state.width.expect("width").get();

            type I24 = [u8; 3];

            match width {
                8 => channel_positions::reorder_channels::<u8>(buffer_ref, reorder_map)?,
                16 => channel_positions::reorder_channels::<i16>(buffer_ref, reorder_map)?,
                24 => channel_positions::reorder_channels::<I24>(buffer_ref, reorder_map)?,
                _ => unreachable!(),
            }
        }

        // Mark start of talkspurt with RESYNC flag
        if packet.marker_bit() {
            buffer_ref.set_flags(gst::BufferFlags::RESYNC);
        }

        gst::trace!(
            CAT,
            imp = self,
            "Finishing buffer {buffer:?} for packet {packet:?}"
        );

        self.obj().queue_buffer(packet.into(), buffer)
    }
}

impl RtpLinearAudioDepay {}

trait RtpLinearAudioDepayImpl: RtpBaseDepay2ImplExt {}

unsafe impl<T: RtpLinearAudioDepayImpl> IsSubclassable<T> for super::RtpLinearAudioDepay {}

/**
 * SECTION:element-rtpL8depay2
 * @see_also: rtpL8pay2, rtpL16depay2, rtpL24depay2, rtpL8pay
 *
 * Extracts raw 8-bit audio from RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.10
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=48000, encoding-name=L8, encoding-params=(string)1, channels=1, payload=96' ! rtpjitterbuffer latency=50 ! rtpL8depay2 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP 8-bit raw audio stream. You can use the #rtpL8pay2
 * element to create such an RTP stream.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL8Depay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL8Depay {
    const NAME: &'static str = "GstRtpL8Depay2";
    type Type = super::RtpL8Depay;
    type ParentType = super::RtpLinearAudioDepay;
}

impl ObjectImpl for RtpL8Depay {}

impl GstObjectImpl for RtpL8Depay {}

impl ElementImpl for RtpL8Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 8-bit Raw Audio Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload 8-bit raw audio (L8) from RTP packets",
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
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "L8")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::U8)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpL8Depay {}

impl RtpLinearAudioDepayImpl for RtpL8Depay {}

/**
 * SECTION:element-rtpL16depay2
 * @see_also: rtpL16pay2, rtpL8depay2, rtpL24depay2, rtpL16pay
 *
 * Extracts raw 16-bit audio from RTP packets as per [RFC 3551][rfc-3551].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.11
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=48000, encoding-name=L16, encoding-params=(string)1, channels=1, payload=96' ! rtpjitterbuffer latency=50 ! rtpL16depay2 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP 16-bit raw audio stream. You can use the #rtpL16pay2
 * element to create such an RTP stream.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL16Depay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL16Depay {
    const NAME: &'static str = "GstRtpL16Depay2";
    type Type = super::RtpL16Depay;
    type ParentType = super::RtpLinearAudioDepay;
}

impl ObjectImpl for RtpL16Depay {}

impl GstObjectImpl for RtpL16Depay {}

impl ElementImpl for RtpL16Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 16-bit Raw Audio Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload 16-bit raw audio (L16) from RTP packets",
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
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "L16")
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("payload", gst::List::new([10i32, 11]))
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::S16be)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpL16Depay {}

impl RtpLinearAudioDepayImpl for RtpL16Depay {}

/**
 * SECTION:element-rtpL24depay2
 * @see_also: rtpL24pay2, rtpL8depay2, rtpL16depay2, rtpL24pay
 *
 * Extracts raw 24-bit audio from RTP packets as per [RFC 3551][rfc-3551] and
 * [RFC 3190][rfc-3190].
 *
 * [rfc-3551]: https://www.rfc-editor.org/rfc/rfc3551.html#section-4.5.11
 * [rfc-3190]: https://www.rfc-editor.org/rfc/rfc3190.html#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=48000, encoding-name=L24, encoding-params=(string)1, channels=1, payload=96' ! rtpjitterbuffer latency=50 ! rtpL24depay2 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP 24-bit raw audio stream. You can use the #rtpL24pay2
 * element to create such an RTP stream.
 *
 * Since: plugins-rs-0.14.0
 */

#[derive(Default)]
pub(crate) struct RtpL24Depay;

#[glib::object_subclass]
impl ObjectSubclass for RtpL24Depay {
    const NAME: &'static str = "GstRtpL24Depay2";
    type Type = super::RtpL24Depay;
    type ParentType = super::RtpLinearAudioDepay;
}

impl ObjectImpl for RtpL24Depay {}

impl GstObjectImpl for RtpL24Depay {}

impl ElementImpl for RtpL24Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP 24-bit Raw Audio Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload 24-bit raw audio (L24) from RTP packets",
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
                            .field("media", "audio")
                            .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                            .field("encoding-name", "L24")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &AudioCapsBuilder::new_interleaved()
                    .format(AudioFormat::S24be)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpL24Depay {}

impl RtpLinearAudioDepayImpl for RtpL24Depay {}

#[cfg(test)]
mod tests {
    use byte_slice_cast::*;
    use gst_check::Harness;

    #[test]
    fn test_channel_reorder_l8() {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtp plugin");

        let mut h = Harness::new("rtpL8depay2");
        h.play();

        let caps = gst::Caps::builder("application/x-rtp")
            .field("media", "audio")
            .field("payload", 96)
            .field("clock-rate", 48000)
            .field("encoding-name", "L8")
            .field("channels", "6") // can be string or int
            .field("channel-order", "DV.LRLsRsCS")
            .build();

        h.set_src_caps(caps);

        let input_data = [1u8, 2, 3, 4, 5, 6, 11, 12, 13, 14, 15, 16];

        let builder = rtp_types::RtpPacketBuilder::new()
            .marker_bit(false)
            .timestamp(48000)
            .payload_type(96)
            .sequence_number(456)
            .payload(input_data.as_slice());

        let buf = builder.write_vec().unwrap();
        let buf = gst::Buffer::from_mut_slice(buf);
        h.push(buf).unwrap();
        h.push_event(gst::event::Eos::new());

        let outbuf = h.pull().unwrap();

        let out_map = outbuf.map_readable().unwrap();
        let out_data = out_map.as_slice_of::<u8>().unwrap();

        // input:   [ 1,  2,  3,  4,  5,  6 | 11, 12, 13, 14, 15, 16]
        //          @ FrontLeft, FrontRight, SideLeft, SideRight, FrontCenter, RearCenter
        //
        // output: [ 1,  2,  5,  6,  3,  4 | 11, 12, 15, 16, 13, 14]
        //         @ FrontLeft, FrontRight, FrontCenter, RearCenter, SideLeft, SideRight
        assert_eq!(out_data, [1, 2, 5, 6, 3, 4, 11, 12, 15, 16, 13, 14]);
    }
}
