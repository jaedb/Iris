// GStreamer RTP MPEG-4 Generic elementary streams Depayloader
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp4gdepay2
 * @see_also: rtpmp4gpay2, rtpmp4gdepay, rtpmp4gpay
 *
 * Depayload an MPEG-4 Generic elementary stream from RTP packets as per [RFC 3640][rfc-3640].
 *
 * [rfc-3640]: https://www.rfc-editor.org/rfc/rfc3640.html#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp,media=audio,clock-rate=44100,encoding-name=MPEG4-GENERIC,payload=96,encoding-params=1,streamtype=5,profile-level-id=2,mode=AAC-hbr,config=(string)1208,sizelength=13,indexlength=3,indexdeltalength=3' ! rtpjitterbuffer ! rtpmp4gdepay2 ! decodebin3 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP MPEG-4 generic elementary stream AAC-hbr with
 * 1 channel @ 44100 sampling rate (default `audiotestsrc ! fdkaacenc` negotiation).
 * You can use the #rtpmp4gpay2 or #rtpmp4gpay elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use anyhow::Context;
use atomic_refcell::AtomicRefCell;
use std::sync::LazyLock;

use gst::{glib, prelude::*, subclass::prelude::*};

use std::ops::{ControlFlow, RangeInclusive};

use crate::basedepay::{Packet, PacketToBufferRelation, RtpBaseDepay2Ext, TimestampOffset};

use crate::mp4g::{ModeConfig, RtpTimestamp};

use super::parsers::PayloadParser;
use super::{
    AccessUnit, DeinterleaveAuBuffer, MaybeSingleAuOrList, Mpeg4GenericDepayError, SingleAuOrList,
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp4gdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-4 generic Depayloader"),
    )
});

#[derive(Default)]
pub struct RtpMpeg4GenericDepay {
    state: AtomicRefCell<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpMpeg4GenericDepay {
    const NAME: &'static str = "GstRtpMpeg4GenericDepay";
    type Type = super::RtpMpeg4GenericDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpMpeg4GenericDepay {}

impl GstObjectImpl for RtpMpeg4GenericDepay {}

impl ElementImpl for RtpMpeg4GenericDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-4 Generic ES Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload MPEG-4 Generic elementary streams from RTP packets (RFC 3640)",
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
                    // TODO "application" is also present in rtpmp4gdepay caps template
                    //       but it doesn't handle it in gst_rtp_mp4g_depay_setcaps
                    .field("media", gst::List::new(["audio", "video"]))
                    .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                    .field("encoding-name", "MPEG4-GENERIC")
                    // Required string params:
                    // "streamtype = { \"4\", \"5\" }, "  Not set by Wowza    4 = video, 5 = audio
                    // "profile-level-id = [1,MAX], "
                    // "config = (string)"
                    .field(
                        "mode",
                        gst::List::new(["generic", "AAC-lbr", "AAC-hbr", "aac-hbr"]),
                    )
                    // Optional general parameters:
                    // "objecttype = [1,MAX], "
                    // "constantsize = [1,MAX], "     // constant size of each AU
                    // "constantduration = [1,MAX], " // constant duration of each AU
                    // "maxdisplacement = [1,MAX], "
                    // "de-interleavebuffersize = [1,MAX], "
                    // Optional configuration parameters:
                    // "sizelength = [1, 32], "
                    // "indexlength = [1, 32], "
                    // "indexdeltalength = [1, 32], "
                    // "ctsdeltalength = [1, 32], "
                    // "dtsdeltalength = [1, 32], "
                    // "randomaccessindication = {0, 1}, "
                    // "streamstateindication = [0, 32], "
                    // "auxiliarydatasizelength = [0, 32]" )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("video/mpeg")
                            .field("mpegversion", 4i32)
                            .field("systemstream", false)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/mpeg")
                            .field("mpegversion", 4i32)
                            .field("stream-format", "raw")
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

#[derive(Debug, Default)]
struct State {
    parser: PayloadParser,
    deint_buf: Option<DeinterleaveAuBuffer>,
    au_acc: Option<AuAccumulator>,
    seqnum_base: Option<u32>,
    clock_rate: u32,
    can_parse: bool,
    max_au_index: Option<usize>,
    prev_au_index: Option<usize>,
    prev_rtptime: Option<u64>,
    last_au_index: Option<usize>,
}

impl State {
    fn flush(&mut self) {
        self.parser.reset();

        if let Some(deint_buf) = self.deint_buf.as_mut() {
            deint_buf.flush();
        }
        self.can_parse = false;
        self.max_au_index = None;
        self.prev_au_index = None;
        self.prev_rtptime = None;
        self.last_au_index = None;
    }
}

struct CodecData;
impl CodecData {
    fn from_caps(s: &gst::StructureRef) -> anyhow::Result<Option<gst::Buffer>> {
        let conf_str = s.get_optional::<&str>("config").context("config field")?;
        let Some(conf_str) = conf_str else {
            return Ok(None);
        };
        let data = hex::decode(conf_str).context("decoding config")?;

        Ok(Some(gst::Buffer::from_mut_slice(data)))
    }
}

/// Accumulates packets for a fragmented AU.
///
/// Used for packets containing fragments for a single AU.
///
/// From https://www.rfc-editor.org/rfc/rfc3640.html#section-3.2.3:
///
/// > The Access Unit Data Section contains an integer number of complete
/// > Access Units or a single fragment of one AU.
#[derive(Debug)]
struct AuAccumulator(AccessUnit);

impl AuAccumulator {
    #[inline]
    fn new(au: AccessUnit) -> Self {
        AuAccumulator(au)
    }
    #[inline]
    fn try_append(&mut self, mut au: AccessUnit) -> Result<(), Mpeg4GenericDepayError> {
        use Mpeg4GenericDepayError::*;

        // FIXME add comment about fragments having the same RTP timestamp
        if self.0.cts_delta.opt_ne(au.cts_delta).unwrap_or(false) {
            return Err(FragmentedAuRtpTsMismatch {
                expected: self.0.cts_delta.unwrap(),
                found: au.cts_delta.unwrap(),
                ext_seqnum: au.ext_seqnum,
            });
        }

        if self.0.dts_delta.opt_ne(au.dts_delta).unwrap_or(false) {
            // § 3.2.1.1
            // > The DTS-delta field MUST have the same value
            // > for all fragments of an Access Unit
            return Err(FragmentedAuDtsMismatch {
                expected: self.0.dts_delta.unwrap(),
                found: au.dts_delta.unwrap(),
                ext_seqnum: au.ext_seqnum,
            });
        }

        self.0.data.append(&mut au.data);

        Ok(())
    }

    #[inline]
    fn try_into_au(self) -> Result<AccessUnit, Mpeg4GenericDepayError> {
        let au = self.0;
        if let Some(expected) = au.size {
            if expected as usize != au.data.len() {
                return Err(Mpeg4GenericDepayError::FragmentedAuSizeMismatch {
                    expected,
                    found: au.data.len(),
                    ext_seqnum: au.ext_seqnum,
                });
            }
        }

        Ok(au)
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpMpeg4GenericDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio", "video"];

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if let Some(ref mut deint_buf) = state.deint_buf {
            if let Some(aus) = deint_buf.drain().take() {
                self.finish_buffer_or_list(&state, None, aus)?;
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn flush(&self) {
        gst::debug!(CAT, imp = self, "Flushing");
        self.state.borrow_mut().flush();
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let mode = s.get::<&str>("mode").expect("Required by Caps");
        if mode.starts_with("CELP") {
            gst::error!(CAT, imp = self, "{mode} not supported yet");
            return false;
        }

        let mut caps_builder = match s.get::<&str>("media").expect("Required by Caps") {
            "audio" => gst::Caps::builder("audio/mpeg")
                .field("mpegversion", 4i32)
                .field("stream-format", "raw"),
            "video" => gst::Caps::builder("video/mpeg")
                .field("mpegversion", 4i32)
                .field("systemstream", false),
            // TODO handle "application"
            _ => unreachable!(),
        };

        let mode_config = match ModeConfig::from_caps(s) {
            Ok(h) => h,
            Err(err) => {
                gst::error!(CAT, imp = self, "Error parsing Header in Caps: {err:#}");
                return false;
            }
        };

        match CodecData::from_caps(s) {
            Ok(codec_data) => {
                caps_builder = caps_builder.field("codec_data", codec_data);
            }
            Err(err) => {
                gst::error!(CAT, imp = self, "Error parsing Caps: {err:#}");
                return false;
            }
        }

        let clock_rate = s.get::<i32>("clock-rate").expect("Required by Caps");
        debug_assert!(clock_rate.is_positive()); // constrained by Caps
        let clock_rate = clock_rate as u32;

        {
            let mut state = self.state.borrow_mut();
            state.seqnum_base = s.get_optional::<u32>("seqnum-base").unwrap();
            if let Some(seqnum_base) = state.seqnum_base {
                gst::info!(CAT, imp = self, "Got seqnum_base {seqnum_base}");
            }
            state.clock_rate = clock_rate;

            if let Some(max_displacement) = mode_config.max_displacement() {
                state.deint_buf = Some(DeinterleaveAuBuffer::new(max_displacement));
            }

            state.parser.set_config(mode_config);
        }

        self.obj().set_src_caps(&caps_builder.build());

        true
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if self.check_initial_packet(&mut state, packet).is_break() {
            self.obj().drop_packets(..=packet.ext_seqnum());
            return Ok(gst::FlowSuccess::Ok);
        }

        let State {
            parser,
            au_acc,
            deint_buf,
            ..
        } = &mut *state;

        let payload = packet.payload();
        let ext_seqnum = packet.ext_seqnum();
        let packet_ts = RtpTimestamp::from_ext(packet.ext_timestamp());
        let au_iter = match parser.parse(payload, ext_seqnum, packet_ts) {
            Ok(au_iter) => au_iter,
            Err(err) => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to parse payload for packet {ext_seqnum}: {err:#}"
                );
                *au_acc = None;
                self.obj().drop_packets(..=packet.ext_seqnum());

                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let mut aus = MaybeSingleAuOrList::default();
        for au in au_iter {
            let au = match au {
                Ok(au) => au,
                Err(err) => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to parse AU from packet {}: {err:#}",
                        packet.ext_seqnum(),
                    );

                    continue;
                }
            };

            // § 3.1: The marker indicates that:
            // > the RTP packet payload contains either the final fragment of
            // > a fragmented Access Unit or one or more complete Access Units
            if !packet.marker_bit() {
                if !au.is_fragment {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Dropping non fragmented AU {au} in un-marked packet"
                    );
                    continue;
                }

                if let Some(ref mut acc) = au_acc {
                    if let Err(err) = acc.try_append(au) {
                        gst::warning!(CAT, imp = self, "Discarding pending fragmented AU: {err}");
                        *au_acc = None;
                        parser.reset();
                        self.obj().drop_packets(..=packet.ext_seqnum());

                        return Ok(gst::FlowSuccess::Ok);
                    }
                } else {
                    *au_acc = Some(AuAccumulator::new(au));
                }

                gst::trace!(CAT, imp = self, "Non-final fragment");

                return Ok(gst::FlowSuccess::Ok);
            }

            // Packet marker set

            let au = match au_acc.take() {
                Some(mut acc) => {
                    if au.is_fragment {
                        if let Err(err) = acc.try_append(au) {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Discarding pending fragmented AU: {err}"
                            );
                            parser.reset();
                            self.obj().drop_packets(..=packet.ext_seqnum());

                            return Ok(gst::FlowSuccess::Ok);
                        }

                        match acc.try_into_au() {
                            Ok(au) => au,
                            Err(err) => {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Discarding pending fragmented AU: {err}"
                                );
                                let Mpeg4GenericDepayError::FragmentedAuSizeMismatch { .. } = err
                                else {
                                    unreachable!();
                                };
                                parser.reset();
                                self.obj().drop_packets(..=packet.ext_seqnum());

                                return Ok(gst::FlowSuccess::Ok);
                            }
                        }
                    } else {
                        gst::warning!(CAT, imp = self,
                            "Discarding pending fragmented AU {} due to incoming non fragmented AU {au}",
                            acc.0,
                        );
                        self.obj().drop_packets(..au.ext_seqnum);

                        au
                    }
                }
                None => au,
            };

            if let Some(ref mut deint_buf) = deint_buf {
                if let Err(err) = deint_buf.push_and_pop(au, &mut aus) {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to push AU to deinterleave buffer: {err}"
                    );
                    // The AU has been dropped, just keep going
                    // Packet will be dropped eventually
                }

                continue;
            }

            if au.is_interleaved {
                // From gstrtpmp4gdepay.c:616:
                // > some broken non-interleaved streams have AU-index jumping around
                // > all over the place, apparently assuming receiver disregards

                gst::warning!(
                    CAT,
                    imp = self,
                    "Interleaved AU, but no `max_displacement` was defined"
                );
            }

            aus.push(au);
        }

        if let Some(aus) = aus.take() {
            self.finish_buffer_or_list(&state, Some(packet.ext_seqnum()), aus)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl RtpMpeg4GenericDepay {
    #[inline]
    fn check_initial_packet(&self, state: &mut State, packet: &Packet) -> ControlFlow<()> {
        if state.can_parse {
            return ControlFlow::Continue(());
        }

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

        // Wait until a marked packet is found and start parsing from the next packet
        if packet.marker_bit() {
            gst::debug!(CAT, imp = self,
                "Found first marked packet {seqnum} (ext seqnum {}). Will start parsing from next packet",
                packet.ext_seqnum(),
            );

            assert!(state.au_acc.is_none());
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

    fn finish_buffer_or_list(
        &self,
        state: &State,
        packet_ext_seqnum: Option<u64>,
        aus: SingleAuOrList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        use SingleAuOrList::*;

        fn get_packet_to_buffer_relation(
            au: &AccessUnit,
            clock_rate: u32,
            range: RangeInclusive<u64>,
        ) -> PacketToBufferRelation {
            if let Some((cts_delta, dts_delta)) = Option::zip(au.cts_delta, au.dts_delta) {
                let pts_offset = gst::Signed::<gst::ClockTime>::from(cts_delta as i64)
                    .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                    .unwrap();
                let dts_offset = gst::Signed::<gst::ClockTime>::from(dts_delta as i64)
                    .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                    .unwrap();
                PacketToBufferRelation::SeqnumsWithOffset {
                    seqnums: range,
                    timestamp_offset: TimestampOffset::PtsAndDts(pts_offset, dts_offset),
                }
            } else if let Some(cts_delta) = au.cts_delta {
                let pts_offset = gst::Signed::<gst::ClockTime>::from(cts_delta as i64)
                    .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                    .unwrap();
                PacketToBufferRelation::SeqnumsWithOffset {
                    seqnums: range,
                    timestamp_offset: TimestampOffset::Pts(pts_offset),
                }
            } else {
                PacketToBufferRelation::Seqnums(range)
            }
        }

        match aus {
            Single(au) => {
                let range = if let Some(packet_ext_seqnum) = packet_ext_seqnum {
                    au.ext_seqnum..=packet_ext_seqnum
                } else {
                    au.ext_seqnum..=au.ext_seqnum
                };

                let packet_to_buffer_relation =
                    get_packet_to_buffer_relation(&au, state.clock_rate, range);

                gst::trace!(
                    CAT,
                    imp = self,
                    "Finishing AU buffer {packet_to_buffer_relation:?}"
                );

                let buffer = Self::new_buffer(au, state);

                self.obj().queue_buffer(packet_to_buffer_relation, buffer)?;
            }
            List(au_list) => {
                for au in au_list {
                    let range = if let Some(packet_ext_seqnum) = packet_ext_seqnum {
                        au.ext_seqnum..=packet_ext_seqnum
                    } else {
                        au.ext_seqnum..=au.ext_seqnum
                    };

                    let packet_to_buffer_relation =
                        get_packet_to_buffer_relation(&au, state.clock_rate, range);

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Finishing AU buffer {packet_to_buffer_relation:?}"
                    );

                    let buffer = Self::new_buffer(au, state);

                    self.obj().queue_buffer(packet_to_buffer_relation, buffer)?;
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    #[inline]
    fn new_buffer(au: AccessUnit, state: &State) -> gst::Buffer {
        let mut buf = gst::Buffer::from_mut_slice(au.data);
        let buf_mut = buf.get_mut().unwrap();

        if au.maybe_random_access == Some(false) {
            buf_mut.set_flags(gst::BufferFlags::DELTA_UNIT)
        }

        if let Some(duration) = au.duration {
            let duration = (duration as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, state.clock_rate as u64)
                .map(gst::ClockTime::from_nseconds);

            buf_mut.set_duration(duration);
        }

        buf
    }
}
