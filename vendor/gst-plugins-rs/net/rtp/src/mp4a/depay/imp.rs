// GStreamer RTP MPEG-4 Audio Depayloader
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp4adepay2
 * @see_also: rtpmp4apay2, rtpmp4adepay, rtpmp4apay
 *
 * Depayload an MPEG-4 Audio bitstream from RTP packets as per [RFC 3016][rfc-3016].
 *
 * [rfc-3016]: https://www.rfc-editor.org/rfc/rfc3016.html#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp,media=audio,clock-rate=90000,encoding-name=MP4A-LATM,payload=96,config=(string)40002410' ! rtpjitterbuffer ! rtpmp4adepay2 ! decodebin3 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP MPEG-4 Audio bitstream (AAC) with
 * 1 channel @ 44100 sampling rate (default `audiotestsrc ! fdkaacenc` negotiation).
 * You can use the #rtpmp4apay2 or #rtpmp4apay elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, BitRead, BitReader};
use std::sync::LazyLock;

use gst::{glib, prelude::*, subclass::prelude::*};

use std::ops::ControlFlow;

use crate::basedepay::{Packet, PacketToBufferRelation, RtpBaseDepay2Ext, TimestampOffset};
use crate::mp4a::parsers::{StreamMuxConfig, Subframes};
use crate::mp4a::{DEFAULT_CLOCK_RATE, ENCODING_NAME};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp4adepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-4 Audio Depayloader"),
    )
});

#[derive(Default)]
pub struct RtpMpeg4AudioDepay {
    state: AtomicRefCell<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpMpeg4AudioDepay {
    const NAME: &'static str = "GstRtpMpeg4AudioDepay";
    type Type = super::RtpMpeg4AudioDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpMpeg4AudioDepay {}

impl GstObjectImpl for RtpMpeg4AudioDepay {}

impl ElementImpl for RtpMpeg4AudioDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-4 Audio Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an MPEG-4 Audio bitstream (e.g. AAC) from RTP packets (RFC 3016)",
                "François Laignel <francois centricular com>",
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
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "audio")
                    .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                    .field("encoding-name", ENCODING_NAME)
                    /* All optional parameters
                     *
                     * "profile-level-id=[1,MAX]"
                     * "config="
                     */
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/mpeg")
                    .field("mpegversion", 4i32)
                    .field("framed", true)
                    .field("stream-format", "raw")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

#[derive(Debug, Default)]
struct State {
    config: Option<StreamMuxConfig>,
    frame_acc: Option<FrameAccumulator>,
    seqnum_base: Option<u32>,
    can_parse: bool,
}

impl State {
    fn flush(&mut self) {
        self.frame_acc = None;
        self.can_parse = false;
    }
}

#[derive(Debug)]
pub struct FrameAccumulator {
    buf: Option<Vec<u8>>,
    start_ext_seqnum: u64,
}

impl FrameAccumulator {
    pub fn new(packet: &Packet) -> Self {
        FrameAccumulator {
            buf: Some(packet.payload().to_owned()),
            start_ext_seqnum: packet.ext_seqnum(),
        }
    }

    /// Extends this `FrameAccumulator` with the provided `Packet` payload.
    ///
    /// # Panic
    ///
    /// Panics if the subframes have already been taken.
    #[track_caller]
    pub fn extend(&mut self, packet: &Packet) {
        self.buf
            .as_mut()
            .expect("subframes already taken")
            .extend_from_slice(packet.payload());
    }

    /// Takes the `Subframes` out of this `FrameAccumulator`.
    ///
    /// # Panic
    ///
    /// Panics if the subframes have already been taken.
    #[track_caller]
    pub fn take_subframes<'a>(&'a mut self, config: &'a StreamMuxConfig) -> Subframes<'a> {
        let buf = self.buf.take().expect("subframes already taken");
        Subframes::new(buf, config)
    }
}

#[derive(Debug)]
struct ConfigWithCodecData {
    config: StreamMuxConfig,
    codec_data: gst::Buffer,
}

impl ConfigWithCodecData {
    fn from_caps_structure(s: &gst::StructureRef) -> anyhow::Result<Option<Self>> {
        use anyhow::Context;

        let conf_str = s.get_optional::<&str>("config").context("config field")?;
        let Some(conf_str) = conf_str else {
            return Ok(None);
        };

        let mut data = hex::decode(conf_str).context("decoding config")?;

        let mut reader = BitReader::endian(data.as_slice(), BigEndian);
        let config = reader.parse::<StreamMuxConfig>()?;

        // Shift buffer for codec_data
        for i in 0..(data.len() - 2) {
            data[i] = ((data[i + 1] & 1) << 7) | ((data[i + 2] & 0xfe) >> 1);
        }

        let codec_data = gst::Buffer::from_mut_slice(data);

        Ok(Some(ConfigWithCodecData { config, codec_data }))
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpMpeg4AudioDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let mut caps_builder = gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("framed", true)
            .field("stream-format", "raw");

        let mut config = match ConfigWithCodecData::from_caps_structure(s) {
            Ok(Some(c)) => {
                gst::log!(CAT, imp = self, "{:?}", c.config);

                caps_builder = caps_builder
                    .field("channels", c.config.prog.channel_conf as i32)
                    .field("rate", c.config.prog.sampling_freq as i32)
                    .field("codec_data", c.codec_data);

                c.config
            }
            Ok(None) => {
                // In-band StreamMuxConfig not supported yet
                gst::log!(CAT, imp = self, "config field not found");
                return false;
            }
            Err(err) => {
                gst::error!(CAT, imp = self, "Error parsing StreamMuxConfig: {err}");
                return false;
            }
        };

        let clock_rate = s.get::<i32>("clock-rate").expect("Required by Caps");
        debug_assert!(clock_rate.is_positive()); // constrained by Caps
        let clock_rate = clock_rate as u32;

        let audio = &config.prog;
        if clock_rate != DEFAULT_CLOCK_RATE && clock_rate != audio.sampling_freq {
            if (audio.audio_object_type == 5 || audio.audio_object_type == 29)
                && clock_rate == 2 * audio.sampling_freq
            {
                // FIXME this is a workaround for forward compatibility with AAC SBR & HE
                // see also comment in the parsers module.
                gst::warning!(CAT, imp = self, concat!(
                        "Found audio object type {}, which uses a specific extension for samplingFrequency. ",
                        "This extension is not supported yet. ",
                        "Will use 'clock-rate' {} as a workaround.",
                    ),
                    audio.audio_object_type,
                    clock_rate,
                );
            } else {
                gst::error!(
                    CAT,
                    imp = self,
                    concat!(
                        "Caps 'clock-rate' {} and 'codec-data' sample rate {} mismatch. ",
                        "Will use 'clock-rate'",
                    ),
                    clock_rate,
                    audio.sampling_freq,
                );
            }

            config.prog.sampling_freq = clock_rate;
        }

        {
            let mut state = self.state.borrow_mut();
            state.seqnum_base = s.get_optional::<u32>("seqnum-base").unwrap();
            state.config = Some(config);
        }

        self.obj().set_src_caps(&caps_builder.build());

        true
    }

    // Can't push incomplete frames, so draining is the same as flushing.
    fn flush(&self) {
        gst::debug!(CAT, imp = self, "Flushing");
        self.state.borrow_mut().flush();
    }

    /// Packetization of MPEG-4 audio bitstreams:
    /// https://www.rfc-editor.org/rfc/rfc3016.html#section-4
    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if !state.can_parse && self.check_initial_packet(&mut state, packet).is_break() {
            self.obj().drop_packets(..=packet.ext_seqnum());
            return Ok(gst::FlowSuccess::Ok);
        }

        if let Some(ref mut frame_acc) = state.frame_acc {
            frame_acc.extend(packet);
        } else {
            state.frame_acc = Some(FrameAccumulator::new(packet));
        }

        // RTP marker bit indicates the last packet of the AudioMuxElement
        if !packet.marker_bit() {
            return Ok(gst::FlowSuccess::Ok);
        }

        let mut frame = state.frame_acc.take().expect("frame_acc ");

        // Extract and push subframes from the accumulated buffers.

        // Payload is AudioMuxElement - ISO/IEC 14496-3 sub 1 table 1.20

        // FIXME StreamMuxConfig may be present in the payload if muxConfigPresent is set
        //       in which case the audioMuxElement SHALL include an indication bit useSameStreamMux
        // Current implementation is on par with rtpmp4adepay
        // See also: https://gitlab.freedesktop.org/gstreamer/gstreamer/-/merge_requests/1173

        let Some(config) = state.config.as_ref() else {
            gst::error!(CAT, imp = self, "In-band StreamMuxConfig not supported");
            return Err(gst::FlowError::NotSupported);
        };

        let range = frame.start_ext_seqnum..=packet.ext_seqnum();

        let mut accumulated_duration = gst::ClockTime::ZERO;

        for (idx, subframe) in frame.take_subframes(config).enumerate() {
            match subframe {
                Ok(subframe) => {
                    gst::log!(CAT, imp = self, "subframe {idx}: len {}", subframe.size());
                    // The duration is always set by the subframes iterator
                    let duration = subframe.duration().expect("no duration set");

                    self.obj().queue_buffer(
                        PacketToBufferRelation::SeqnumsWithOffset {
                            seqnums: range.clone(),
                            timestamp_offset: TimestampOffset::Pts(
                                accumulated_duration.into_positive(),
                            ),
                        },
                        subframe,
                    )?;

                    accumulated_duration.opt_add_assign(duration);
                }
                Err(err) if err.is_zero_length_subframe() => {
                    gst::warning!(CAT, imp = self, "{err}");
                    continue;
                }
                Err(err) => {
                    gst::warning!(CAT, imp = self, "{err}");
                    self.obj().drop_packets(..=packet.ext_seqnum());
                    break;
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl RtpMpeg4AudioDepay {
    #[inline]
    fn check_initial_packet(&self, state: &mut State, packet: &Packet) -> ControlFlow<()> {
        let seqnum = (packet.ext_seqnum() & 0xffff) as u16;

        if let Some(seqnum_base) = state.seqnum_base {
            let seqnum_base = (seqnum_base & 0xffff) as u16;

            // Assume seqnum_base and the initial ext_seqnum are in the same cycle
            // This should be guaranteed by the JitterBuffer
            let delta = crate::utils::seqnum_distance(seqnum, seqnum_base);

            if delta == 0 {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Got initial packet {seqnum_base} @ ext seqnum {}",
                    packet.ext_seqnum(),
                );
                state.can_parse = true;

                return ControlFlow::Continue(());
            }

            if delta < 0 {
                gst::log!(
                    CAT,
                    imp = self,
                    "Waiting for initial packet {seqnum_base}, got {seqnum} (ext seqnum {})",
                    packet.ext_seqnum(),
                );

                return ControlFlow::Break(());
            }

            gst::debug!(CAT, imp = self,
                "Packet {seqnum} (ext seqnum {}) passed expected initial packet {seqnum_base}, will sync on next marker",
                packet.ext_seqnum(),
            );

            state.seqnum_base = None;
        }

        // AudioMuxElement doesn't come with a frame start marker
        // so wait until a marked packet is found and start parsing from the next packet
        if packet.marker_bit() {
            gst::debug!(CAT, imp = self,
                "Found first marked packet {seqnum} (ext seqnum {}). Will start parsing from next packet",
                packet.ext_seqnum(),
            );

            assert!(state.frame_acc.is_none());
            state.can_parse = true;
        } else {
            gst::log!(
                CAT,
                imp = self,
                "First marked packet not found yet, skipping packet {seqnum} (ext seqnum {})",
                packet.ext_seqnum(),
            );
        }

        ControlFlow::Break(())
    }
}

#[cfg(test)]
mod tests {
    const RATE: u64 = 44_100;
    const FRAME_LEN: u64 = 1024;

    struct HarnessBuilder {
        subframes: u64,
        seqnum_base: Option<u32>,
    }

    impl HarnessBuilder {
        fn subframes(mut self, subframes: u64) -> Self {
            assert!(subframes > 0 && subframes <= 0b100_0000);
            self.subframes = subframes;
            self
        }

        fn seqnum_base(mut self, seqnum_base: u32) -> Self {
            self.seqnum_base = Some(seqnum_base);
            self
        }

        fn build_and_prepare(self) -> Harness {
            use gst::prelude::MulDiv;

            gst::init().unwrap();
            crate::plugin_register_static().expect("failed to register plugin");

            let depay = gst::ElementFactory::make("rtpmp4adepay2").build().unwrap();

            let mut h = gst_check::Harness::with_element(&depay, Some("sink"), Some("src"));
            h.play();

            let caps = gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("clock-rate", RATE as i32)
                .field("encoding-name", "MP4A-LATM")
                .field(
                    "config",
                    format!("{:02x}002410", 0x40 | (self.subframes - 1)),
                )
                .field_if_some("seqnum-base", self.seqnum_base)
                .build();

            assert!(h.push_event(gst::event::Caps::new(&caps)));

            let segment = gst::FormattedSegment::<gst::format::Time>::new();
            assert!(h.push_event(gst::event::Segment::new(&segment)));

            let frame_duration = FRAME_LEN
                .mul_div_floor(*gst::ClockTime::SECOND, RATE)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();

            Harness {
                h,
                frame_duration,
                pts: gst::ClockTime::ZERO,
            }
        }
    }

    struct Harness {
        h: gst_check::Harness,
        frame_duration: gst::ClockTime,
        pts: gst::ClockTime,
    }

    impl Harness {
        fn builder() -> HarnessBuilder {
            HarnessBuilder {
                subframes: 1,
                seqnum_base: None,
            }
        }

        /// Prepares a Harness with defaults.
        fn prepare() -> Harness {
            Self::builder().build_and_prepare()
        }

        fn crank_pts(&mut self) {
            self.pts += self.frame_duration;
        }

        #[track_caller]
        fn check_pts(&self, frame: &gst::Buffer) {
            assert_eq!(frame.pts().unwrap(), self.pts);
        }

        #[track_caller]
        fn push(&mut self, packet: &'static [u8]) {
            let mut buf = gst::Buffer::from_slice(packet);
            buf.get_mut().unwrap().set_pts(self.pts);

            self.h.push(buf).expect("Couldn't push buffer");
        }

        #[track_caller]
        fn push_and_ensure_no_frames(&mut self, packet: &'static [u8]) {
            self.push(packet);
            assert!(self.h.try_pull().is_none(), "Expecting no frames, got one");
        }

        #[track_caller]
        fn push_and_check_single_packet_frame(&mut self, packet: &'static [u8]) {
            self.push(packet);
            let frame = self.h.pull().unwrap();
            self.check_pts(&frame);
            assert_eq!(frame.map_readable().unwrap().as_slice(), &packet[13..]);
            self.crank_pts();
        }

        #[track_caller]
        fn flush_and_push_segment(&mut self) {
            self.h.push_event(gst::event::FlushStart::new());
            self.h.push_event(gst::event::FlushStop::new(false));

            let segment = gst::FormattedSegment::<gst::format::Time>::new();
            assert!(self.h.push_event(gst::event::Segment::new(&segment)));
        }
    }

    impl std::ops::Deref for Harness {
        type Target = gst_check::Harness;
        fn deref(&self) -> &Self::Target {
            &self.h
        }
    }

    impl std::ops::DerefMut for Harness {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.h
        }
    }

    #[test]
    fn two_frames_two_packets_skipping_first() {
        let mut h = Harness::prepare();

        let p0 = &[
            0x80, 0xe0, 0x73, 0x02, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Skipping first packet, but it comes with a marker,
        // so will start parsing from next packet.
        h.push_and_ensure_no_frames(p0);

        let p1 = &[
            0x80, 0xe0, 0x73, 0x03, 0xb3, 0x1f, 0x7e, 0x9a, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p1);
    }

    #[test]
    fn two_frames_three_packets_skipping_first() {
        let mut h = Harness::prepare();

        let p0 = &[
            0x80, 0x60, 0x04, 0x16, 0x76, 0xe8, 0x29, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0xff, 0x33,
            0x01, 0x3a, 0x99, 0x98, 0x3d, 0xbe, 0x2a, 0x29, 0xbe, 0x29, 0x42, 0x73, 0x7a, 0x9b,
            0x20, 0x2e, 0xbe, 0xb8, 0xd7, 0xb7, 0x9d, 0xba, 0xac, 0xff, 0xfa, 0xbf, 0xe7, 0xf1,
            0xd7, 0x1a, 0xf6, 0xa9, 0x4d, 0xff, 0xfd, 0x6f, 0xf1, 0xf8, 0xeb, 0x5e, 0x6e, 0xa5,
            0x52, 0x29, 0xa5, 0x20, 0x1a, 0x68, 0x80, 0x1e, 0x9a, 0x04, 0x49, 0xa6, 0x01, 0x03,
            0x4d, 0x02, 0x24, 0xbf, 0x7f, 0x16, 0xfd, 0xa5, 0x91, 0xfd, 0x0e, 0xa8, 0xfc, 0x07,
            0x60, 0x7d, 0xb3, 0xb0, 0x38, 0x41, 0xa9, 0x64, 0x68, 0x85, 0xd8, 0x1c, 0xa1, 0xf7,
            0x89, 0xb3, 0xa0, 0x30, 0xca, 0x18, 0x62, 0x0c, 0x58, 0x04, 0x9c, 0x13, 0x8c, 0x30,
            0xca, 0x0c, 0x62, 0x0c, 0x4e, 0x09, 0x18, 0x23, 0x40, 0x3e, 0x3f, 0xf1, 0x8e, 0x40,
            0xdf, 0x96, 0xc5, 0x70, 0xf1, 0xa3, 0x92, 0x55, 0x16, 0x17, 0x1e, 0xfd, 0xb6, 0x9e,
            0x95, 0x0d, 0x49, 0xea, 0x68, 0xf3, 0xfb, 0xbc, 0xc5, 0xe3, 0x9a, 0x9f, 0x92, 0x9a,
            0x3f, 0xbb, 0xf5, 0xee, 0x4c, 0xf7, 0xbf, 0x8a, 0x8e, 0xb2, 0x68, 0x3f, 0x05, 0xd1,
            0xba, 0x8a, 0x87, 0x06, 0x29, 0x16, 0x6e, 0x7d, 0x36, 0x63, 0xc2, 0xe2, 0xdc, 0xaa,
            0xf9, 0x55, 0x56, 0xa9, 0x81, 0xef, 0xbe, 0x5a, 0xfa, 0xf6, 0x1e, 0x6a, 0xd9, 0xba,
            0x3a, 0x35, 0x7f, 0x3f, 0x5c, 0x5f, 0x2d, 0x9b, 0x7b, 0x96, 0x1b, 0x6d, 0xca, 0xb1,
            0xb6, 0x27, 0xd5, 0x4a, 0x57, 0x7b, 0x96, 0x65, 0xe7, 0xd9, 0x2f, 0x3e, 0xc9, 0x63,
            0x6c, 0x56, 0x1a, 0xd4, 0xec, 0x71, 0x95, 0xc6, 0x4a, 0x24, 0xaf, 0xdd, 0xb2, 0xfd,
            0xdc, 0x2f, 0x6b, 0x85, 0x36, 0x75, 0x18, 0xcc, 0xb4, 0x11, 0x3c, 0x20, 0x9f, 0xda,
            0xe1, 0x7b, 0x5c, 0x2e,
        ];
        // Skipping first markerless packet
        h.push_and_ensure_no_frames(p0);

        let p1 = &[
            0x80, 0xe0, 0x04, 0x17, 0x76, 0xe8, 0x29, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0xeb, 0x6d,
            0x79, 0x22, 0x4a, 0x25, 0x22, 0x54, 0x65, 0x18, 0x5e, 0xfb, 0xd8, 0x65, 0xce, 0x11,
            0xb2, 0xe4, 0x22, 0x20, 0x17, 0xd7, 0xee, 0xea, 0x60, 0x53, 0x3f, 0xc6, 0xee, 0x9f,
            0xe3, 0x7a, 0xef, 0xeb, 0xbd, 0x76, 0x82, 0x23, 0xf3, 0x08, 0xb2, 0x76, 0xed, 0x77,
            0x1d, 0x8d, 0x8e, 0x8d, 0x7e, 0x52, 0xfc, 0xa5, 0x52, 0x95, 0x4a, 0x66, 0x92, 0x69,
            0x1a, 0x4a, 0x1d, 0x9d, 0x9f, 0x07,
        ];
        // Skipping p1, but it comes with a marker,
        // so will start parsing from next packet.
        h.push_and_ensure_no_frames(p1);

        let p2 = &[
            0x80, 0xe0, 0x04, 0x18, 0x76, 0xe8, 0x2d, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0x41, 0x01,
            0x38, 0xf4, 0x2d, 0x22, 0xd0, 0x91, 0x5d, 0xfe, 0x79, 0xff, 0x12, 0x9e, 0x5c, 0x4d,
            0x4b, 0x96, 0xe2, 0x35, 0xa2, 0x8c, 0x1c, 0x3e, 0x78, 0x84, 0x10, 0xc9, 0x9a, 0x96,
            0x8b, 0x61, 0x76, 0xdc, 0xae, 0x5f, 0xfe, 0xcc, 0xc0, 0x5a, 0xfe, 0xb7, 0x75, 0x71,
            0x76, 0x2c, 0xdb, 0x19, 0xe6, 0xfe, 0x1e, 0x25, 0x3f, 0x8f, 0x84, 0xfe, 0x18, 0x0c,
            0x5e, 0x13, 0xe9, 0x80, 0x0b, 0x7f, 0x01, 0xc0,
        ];
        // Packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p2);
    }

    #[test]
    fn seqnum_base_first_packet() {
        let mut h = Harness::builder().seqnum_base(0x7302).build_and_prepare();

        // ext_seqnum in packet: 0x1_7302
        let p0 = &[
            0x80, 0xe0, 0x73, 0x02, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // First packet matches `seqnum-base` => parsing starts from here
        // & packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p0);
    }

    #[test]
    fn two_frames_three_packets_seqnum_base_first_packet() {
        let mut h = Harness::builder().seqnum_base(0x0416).build_and_prepare();

        // ext_seqnum in packet: 0x1_0416
        let p0 = &[
            0x80, 0x60, 0x04, 0x16, 0x76, 0xe8, 0x29, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0xff, 0x33,
            0x01, 0x3a, 0x99, 0x98, 0x3d, 0xbe, 0x2a, 0x29, 0xbe, 0x29, 0x42, 0x73, 0x7a, 0x9b,
            0x20, 0x2e, 0xbe, 0xb8, 0xd7, 0xb7, 0x9d, 0xba, 0xac, 0xff, 0xfa, 0xbf, 0xe7, 0xf1,
            0xd7, 0x1a, 0xf6, 0xa9, 0x4d, 0xff, 0xfd, 0x6f, 0xf1, 0xf8, 0xeb, 0x5e, 0x6e, 0xa5,
            0x52, 0x29, 0xa5, 0x20, 0x1a, 0x68, 0x80, 0x1e, 0x9a, 0x04, 0x49, 0xa6, 0x01, 0x03,
            0x4d, 0x02, 0x24, 0xbf, 0x7f, 0x16, 0xfd, 0xa5, 0x91, 0xfd, 0x0e, 0xa8, 0xfc, 0x07,
            0x60, 0x7d, 0xb3, 0xb0, 0x38, 0x41, 0xa9, 0x64, 0x68, 0x85, 0xd8, 0x1c, 0xa1, 0xf7,
            0x89, 0xb3, 0xa0, 0x30, 0xca, 0x18, 0x62, 0x0c, 0x58, 0x04, 0x9c, 0x13, 0x8c, 0x30,
            0xca, 0x0c, 0x62, 0x0c, 0x4e, 0x09, 0x18, 0x23, 0x40, 0x3e, 0x3f, 0xf1, 0x8e, 0x40,
            0xdf, 0x96, 0xc5, 0x70, 0xf1, 0xa3, 0x92, 0x55, 0x16, 0x17, 0x1e, 0xfd, 0xb6, 0x9e,
            0x95, 0x0d, 0x49, 0xea, 0x68, 0xf3, 0xfb, 0xbc, 0xc5, 0xe3, 0x9a, 0x9f, 0x92, 0x9a,
            0x3f, 0xbb, 0xf5, 0xee, 0x4c, 0xf7, 0xbf, 0x8a, 0x8e, 0xb2, 0x68, 0x3f, 0x05, 0xd1,
            0xba, 0x8a, 0x87, 0x06, 0x29, 0x16, 0x6e, 0x7d, 0x36, 0x63, 0xc2, 0xe2, 0xdc, 0xaa,
            0xf9, 0x55, 0x56, 0xa9, 0x81, 0xef, 0xbe, 0x5a, 0xfa, 0xf6, 0x1e, 0x6a, 0xd9, 0xba,
            0x3a, 0x35, 0x7f, 0x3f, 0x5c, 0x5f, 0x2d, 0x9b, 0x7b, 0x96, 0x1b, 0x6d, 0xca, 0xb1,
            0xb6, 0x27, 0xd5, 0x4a, 0x57, 0x7b, 0x96, 0x65, 0xe7, 0xd9, 0x2f, 0x3e, 0xc9, 0x63,
            0x6c, 0x56, 0x1a, 0xd4, 0xec, 0x71, 0x95, 0xc6, 0x4a, 0x24, 0xaf, 0xdd, 0xb2, 0xfd,
            0xdc, 0x2f, 0x6b, 0x85, 0x36, 0x75, 0x18, 0xcc, 0xb4, 0x11, 0x3c, 0x20, 0x9f, 0xda,
            0xe1, 0x7b, 0x5c, 0x2e,
        ];
        // First packet matches `seqnum-base` => parsing starts from here
        // But packet is not marked => accumulating
        h.push_and_ensure_no_frames(p0);

        let p1 = &[
            0x80, 0xe0, 0x04, 0x17, 0x76, 0xe8, 0x29, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0xeb, 0x6d,
            0x79, 0x22, 0x4a, 0x25, 0x22, 0x54, 0x65, 0x18, 0x5e, 0xfb, 0xd8, 0x65, 0xce, 0x11,
            0xb2, 0xe4, 0x22, 0x20, 0x17, 0xd7, 0xee, 0xea, 0x60, 0x53, 0x3f, 0xc6, 0xee, 0x9f,
            0xe3, 0x7a, 0xef, 0xeb, 0xbd, 0x76, 0x82, 0x23, 0xf3, 0x08, 0xb2, 0x76, 0xed, 0x77,
            0x1d, 0x8d, 0x8e, 0x8d, 0x7e, 0x52, 0xfc, 0xa5, 0x52, 0x95, 0x4a, 0x66, 0x92, 0x69,
            0x1a, 0x4a, 0x1d, 0x9d, 0x9f, 0x07,
        ];
        // Packet is marked => will push frame to the src pad
        h.push(p1);

        let frame = h.pull().unwrap();
        h.check_pts(&frame);
        let frame = frame.map_readable().unwrap();
        assert_eq!(frame[..p0.len() - 14], p0[14..]);
        assert_eq!(frame[p0.len() - 14..], p1[12..]);

        let p2 = &[
            0x80, 0xe0, 0x04, 0x18, 0x76, 0xe8, 0x2d, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0x41, 0x01,
            0x38, 0xf4, 0x2d, 0x22, 0xd0, 0x91, 0x5d, 0xfe, 0x79, 0xff, 0x12, 0x9e, 0x5c, 0x4d,
            0x4b, 0x96, 0xe2, 0x35, 0xa2, 0x8c, 0x1c, 0x3e, 0x78, 0x84, 0x10, 0xc9, 0x9a, 0x96,
            0x8b, 0x61, 0x76, 0xdc, 0xae, 0x5f, 0xfe, 0xcc, 0xc0, 0x5a, 0xfe, 0xb7, 0x75, 0x71,
            0x76, 0x2c, 0xdb, 0x19, 0xe6, 0xfe, 0x1e, 0x25, 0x3f, 0x8f, 0x84, 0xfe, 0x18, 0x0c,
            0x5e, 0x13, 0xe9, 0x80, 0x0b, 0x7f, 0x01, 0xc0,
        ];
        // Packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p2);
    }

    #[test]
    fn one_frame_two_subframes() {
        let mut h = Harness::builder()
            .subframes(2)
            .seqnum_base(0x7302)
            .build_and_prepare();

        // ext_seqnum in packet: 0x1_7302
        let p0 = &[
            0x80, 0xe0, 0x73, 0x02, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07, 0x06, 0x01, 0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // First packet matches `seqnum-base` => parsing starts from here
        // & packet is marked => will push 2 subframes to the src pad
        h.push(p0);

        let subframe = h.pull().unwrap();
        h.check_pts(&subframe);
        let mut offset = 13usize;
        let mut len = p0[offset - 1] as usize;
        assert_eq!(
            subframe.map_readable().unwrap().as_slice(),
            &p0[offset..][..len]
        );

        // 2 subframes in one packet => reflect this on the actual pts
        h.crank_pts();

        let subframe = h.pull().unwrap();
        h.check_pts(&subframe);
        offset += len + 1;
        len = p0[offset - 1] as usize;
        assert_eq!(
            subframe.map_readable().unwrap().as_slice(),
            &p0[offset..][..len]
        );
    }

    #[test]
    fn seqnum_base_second_packet() {
        let mut h = Harness::builder().seqnum_base(0x7303).build_and_prepare();

        // ext_seqnum in packet: 0x1_7302
        let p0 = &[
            0x80, 0xe0, 0x73, 0x02, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Skipping first packet with seqnum 94978,
        h.push_and_ensure_no_frames(p0);

        let p1 = &[
            0x80, 0xe0, 0x73, 0x03, 0xb3, 0x1f, 0x7e, 0x9a, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // p1 matches `seqnum-base` => parsing starts from here
        // & packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p1);
    }

    #[test]
    fn seqnum_base_passed_first_packet() {
        let mut h = Harness::builder().seqnum_base(0x7300).build_and_prepare();

        // ext_seqnum in packet: 0x1_7302
        let p0 = &[
            0x80, 0xe0, 0x73, 0x02, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // First packet with seqnum 94978 passed `seqnum-base`,
        // but it comes with a marker, so will start parsing from next packet
        h.push_and_ensure_no_frames(p0);

        let p1 = &[
            0x80, 0xe0, 0x73, 0x03, 0xb3, 0x1f, 0x7e, 0x9a, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p1);
    }

    #[test]
    fn two_packets_frame_flush_more_packets() {
        let mut h = Harness::builder().seqnum_base(0x0416).build_and_prepare();

        // ext_seqnum in packet: 0x1_0416
        let p0 = &[
            0x80, 0x60, 0x04, 0x16, 0x76, 0xe8, 0x29, 0xc2, 0x16, 0xd8, 0x37, 0x68, 0xff, 0x33,
            0x01, 0x3a, 0x99, 0x98, 0x3d, 0xbe, 0x2a, 0x29, 0xbe, 0x29, 0x42, 0x73, 0x7a, 0x9b,
            0x20, 0x2e, 0xbe, 0xb8, 0xd7, 0xb7, 0x9d, 0xba, 0xac, 0xff, 0xfa, 0xbf, 0xe7, 0xf1,
            0xd7, 0x1a, 0xf6, 0xa9, 0x4d, 0xff, 0xfd, 0x6f, 0xf1, 0xf8, 0xeb, 0x5e, 0x6e, 0xa5,
            0x52, 0x29, 0xa5, 0x20, 0x1a, 0x68, 0x80, 0x1e, 0x9a, 0x04, 0x49, 0xa6, 0x01, 0x03,
            0x4d, 0x02, 0x24, 0xbf, 0x7f, 0x16, 0xfd, 0xa5, 0x91, 0xfd, 0x0e, 0xa8, 0xfc, 0x07,
            0x60, 0x7d, 0xb3, 0xb0, 0x38, 0x41, 0xa9, 0x64, 0x68, 0x85, 0xd8, 0x1c, 0xa1, 0xf7,
            0x89, 0xb3, 0xa0, 0x30, 0xca, 0x18, 0x62, 0x0c, 0x58, 0x04, 0x9c, 0x13, 0x8c, 0x30,
            0xca, 0x0c, 0x62, 0x0c, 0x4e, 0x09, 0x18, 0x23, 0x40, 0x3e, 0x3f, 0xf1, 0x8e, 0x40,
            0xdf, 0x96, 0xc5, 0x70, 0xf1, 0xa3, 0x92, 0x55, 0x16, 0x17, 0x1e, 0xfd, 0xb6, 0x9e,
            0x95, 0x0d, 0x49, 0xea, 0x68, 0xf3, 0xfb, 0xbc, 0xc5, 0xe3, 0x9a, 0x9f, 0x92, 0x9a,
            0x3f, 0xbb, 0xf5, 0xee, 0x4c, 0xf7, 0xbf, 0x8a, 0x8e, 0xb2, 0x68, 0x3f, 0x05, 0xd1,
            0xba, 0x8a, 0x87, 0x06, 0x29, 0x16, 0x6e, 0x7d, 0x36, 0x63, 0xc2, 0xe2, 0xdc, 0xaa,
            0xf9, 0x55, 0x56, 0xa9, 0x81, 0xef, 0xbe, 0x5a, 0xfa, 0xf6, 0x1e, 0x6a, 0xd9, 0xba,
            0x3a, 0x35, 0x7f, 0x3f, 0x5c, 0x5f, 0x2d, 0x9b, 0x7b, 0x96, 0x1b, 0x6d, 0xca, 0xb1,
            0xb6, 0x27, 0xd5, 0x4a, 0x57, 0x7b, 0x96, 0x65, 0xe7, 0xd9, 0x2f, 0x3e, 0xc9, 0x63,
            0x6c, 0x56, 0x1a, 0xd4, 0xec, 0x71, 0x95, 0xc6, 0x4a, 0x24, 0xaf, 0xdd, 0xb2, 0xfd,
            0xdc, 0x2f, 0x6b, 0x85, 0x36, 0x75, 0x18, 0xcc, 0xb4, 0x11, 0x3c, 0x20, 0x9f, 0xda,
            0xe1, 0x7b, 0x5c, 0x2e,
        ];
        // First packet matches `seqnum-base` => parsing starts from here
        // But packet is not marked => accumulating
        h.push_and_ensure_no_frames(p0);

        h.flush_and_push_segment();

        let p1 = &[
            0x80, 0xe0, 0x05, 0x00, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Skipping first packet after flush, but it comes with a marker,
        // so will start parsing from next packet.
        h.push_and_ensure_no_frames(p1);

        let p2 = &[
            0x80, 0xe0, 0x05, 0x01, 0xb3, 0x1f, 0x7a, 0x9b, 0x05, 0xd9, 0x9c, 0x33, 0x06, 0x01,
            0x40, 0x22, 0x80, 0xa3, 0x07,
        ];
        // Packet is marked => will push frame to the src pad
        h.push_and_check_single_packet_frame(p2);
    }
}
