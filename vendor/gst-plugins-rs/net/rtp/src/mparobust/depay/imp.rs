// GStreamer RTP MPEG Audio Robust elementary streams Depayloader
//
// Copyright (C) 2023 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmparobustdepay2
 * @see_also: rtpmparobustdepay
 *
 * Depayload an MPEG Audio Robust elementary stream from RTP packets as per [RFC 5219][rfc-5219].
 *
 * [rfc-5219]: https://www.rfc-editor.org/rfc/rfc5219.html#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 filesrc location=tests/files/mpa_robust.2ch.interleaved.gdp ! gdpdepay ! rtpmparobustdepay2 ! decodebin3 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload the RTP MPEG Application Data Unit stream and output MP3 frames.
 *
 * The RTP stream was generated using live555's [`liveCaster`] with option `-a 96 "0 2 1 3"`. Please note
 * that this software is no longer supported and that it is hosted on an insecure website.
 *
 * Since: plugins-rs-0.15.0
 *
 * [`liveCaster`]: http://www.live555.com/liveCaster
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, prelude::*, subclass::prelude::*};

use crate::basedepay::{
    PacketToBufferRelation, RtpBaseDepay2Ext, RtpBaseDepay2ImplExt, TimestampOffset,
};

use std::{collections::BTreeMap, ops::RangeInclusive, sync::LazyLock};

use super::{
    super::FrameHeader, AduAccumulator, AduCompleteUnparsed, AduQueue, DataProvenance,
    DeinterleavingAduBuffer, MaybeSingleAduOrList, Mp3Frame, SingleMp3FrameOrList, CAT,
};

#[derive(Default)]
pub struct RtpMpegAudioRobustDepay {
    state: AtomicRefCell<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpMpegAudioRobustDepay {
    const NAME: &'static str = "GstRtpMpegAudioRobustDepay";
    type Type = super::RtpMpegAudioRobustDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpMpegAudioRobustDepay {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl GstObjectImpl for RtpMpegAudioRobustDepay {}

impl ElementImpl for RtpMpegAudioRobustDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG Audio Robust ES Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload MPEG Audio Robust elementary streams from RTP packets (RFC 5219)",
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
                    .field("clock-rate", 90_000i32)
                    .field("encoding-name", "MPA-ROBUST")
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/mpeg")
                    .field("mpegversion", 1i32)
                    .field("parsed", true)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

#[derive(Debug)]
struct State {
    deint_buf: DeinterleavingAduBuffer,
    adu_acc: AduAccumulator,
    pending_adus: AduQueue,
    last_frame_header: Option<FrameHeader>,
    /// Set to `true` if the next outgoing buffer should have the `DISCONT` flag set.
    needs_discont: bool,
    pts_offset_hdlr: PtsOffsetHandler,
}

impl Default for State {
    fn default() -> Self {
        State {
            deint_buf: DeinterleavingAduBuffer::default(),
            adu_acc: AduAccumulator::default(),
            pending_adus: AduQueue::default(),
            last_frame_header: None,
            needs_discont: true,
            pts_offset_hdlr: PtsOffsetHandler::default(),
        }
    }
}

impl State {
    fn flush(&mut self) {
        self.deint_buf.flush();
        self.adu_acc.flush();
        self.pending_adus.flush();
        self.last_frame_header = None;
        self.needs_discont = true;
        self.pts_offset_hdlr.flush();
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpMpegAudioRobustDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        self.state.borrow_mut().flush();

        Ok(())
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if let Some(adus) = state.deint_buf.drain(&self.obj()).into() {
            if let Some(mp3frames) = state
                .pending_adus
                .push_adus_pop_mp3_frames(&self.obj(), adus)
                .into()
            {
                self.finish_buffer_or_list(&mut state, mp3frames)?;
            }
        }

        if let Some(mp3frames) = state.pending_adus.drain(&self.obj()).into() {
            self.finish_buffer_or_list(&mut state, mp3frames)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn flush(&self) {
        gst::debug!(CAT, imp = self, "Flushing");
        self.state.borrow_mut().flush();
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        self.obj().set_src_caps(
            &gst::Caps::builder("audio/mpeg")
                .field("mpegversion", 1i32)
                .field("parsed", true)
                .build(),
        );

        self.parent_set_sink_caps(caps)
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        // strip off descriptor
        //
        //  0                   1
        //  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |C|T|            ADU size         |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        //
        // C: if 1, data is continuation
        // T: if 1, size is 14 bits, otherwise 6 bits
        // ADU size: size of following packet (not including descriptor)

        let ext_seqnum = packet.ext_seqnum();
        let pts = packet.pts();
        let mut data = packet.payload();

        gst::debug!(
            CAT,
            imp = self,
            "Handling incoming packet {ext_seqnum} @ {} with payload len {}",
            pts.display(),
            data.len()
        );

        state
            .pts_offset_hdlr
            .register_packet(ext_seqnum, pts.unwrap_or(gst::ClockTime::ZERO));

        let mut ready_adus = MaybeSingleAduOrList::default();
        let mut cont;
        let mut idx = 0;
        while !data.is_empty() {
            let provenance = DataProvenance { ext_seqnum, idx };

            // ADU Descriptor parsing

            cont = (data[0] & 0x80) == 0x80;

            let (offset, total_size) =
                if (data[0] & 0x40) == 0x40 {
                    // 2-byte descriptor => 14-bit size
                    if data.len() < 3 {
                        gst::element_imp_warning!(self, gst::ResourceError::Read, [
                        "Invalid size for ADU {provenance}. Expected at least 3, available {}",
                        data.len(),
                    ]);

                        state.needs_discont = true;

                        return Err(gst::FlowError::Error);
                    }

                    let size = (data[0] as usize & 0x3f) << 8 | data[1] as usize;
                    (2, size)
                } else {
                    // 6-bit size
                    if data.len() < 2 {
                        gst::element_imp_warning!(self, gst::ResourceError::Read, [
                        "Invalid size for ADU {provenance}. Expected at least 2, available {}",
                        data.len(),
                    ]);

                        state.needs_discont = true;

                        return Err(gst::FlowError::Error);
                    }

                    let size = data[0] as usize & 0x3f;
                    (1, size)
                };

            gst::trace!(
                CAT,
                imp = self,
                "Found ADU {provenance} @ offset {offset} total size {total_size} cont: {cont}"
            );

            let mut adu = if cont {
                // Fragmented ADU handling

                // § 4.3
                // If, however, a single "ADU descriptor" + "ADU frame" is too large to
                // fit within an RTP packet, then the "ADU frame" is split across two or
                // more successive RTP packets. Each such packet begins with an ADU
                // descriptor. The first packet’s descriptor has a "C" (continuation)
                // flag of 0; the following packets’ descriptors each have a "C" flag of 1.
                // Each descriptor, in this case, has the same "ADU size" value: the
                // size of the entire "ADU frame" (not just the portion that will fit
                // within a single RTP packet). Each such packet (even the last one)
                // contains only one "ADU descriptor".
                if idx != 0 {
                    gst::element_imp_warning!(
                        self,
                        gst::ResourceError::Read,
                        ["Skipping unexpected non-first continuation ADU fragment {provenance}"]
                    );

                    state.needs_discont = true;

                    return Err(gst::FlowError::Error);
                }

                match state
                    .adu_acc
                    .try_accumulate(provenance, total_size, &data[offset..])
                {
                    Ok(Some(adu)) => {
                        gst::trace!(CAT, imp = self, "Completed fragmented {adu}");
                        data = &[];

                        adu
                    }
                    Ok(None) => {
                        gst::trace!(
                            CAT,
                            imp = self,
                            "Added non-terminal continuation ADU fragment {provenance}"
                        );
                        return Ok(gst::FlowSuccess::Ok);
                    }
                    Err(err) => {
                        gst::element_imp_warning!(self, gst::ResourceError::Read, ["{err}"]);

                        state.needs_discont = true;

                        return Err(gst::FlowError::Error);
                    }
                }
            } else if total_size > data.len() {
                if let Err(err) = state.adu_acc.start(provenance, total_size, &data[offset..]) {
                    gst::element_imp_warning!(self, gst::ResourceError::Read, ["{err}"]);

                    state.needs_discont = true;

                    return Err(gst::FlowError::Error);
                }

                gst::trace!(
                    CAT,
                    imp = self,
                    "Started new ADU accumulator with ADU {provenance}"
                );

                return Ok(gst::FlowSuccess::Ok);
            } else {
                let adu =
                    match AduCompleteUnparsed::try_new(provenance, &data[offset..][..total_size]) {
                        Ok(adu) => adu,
                        Err(err) => {
                            gst::element_imp_warning!(self, gst::ResourceError::Read, ["{err}"]);

                            state.needs_discont = true;

                            return Err(gst::FlowError::Error);
                        }
                    };

                if total_size == data.len() {
                    data = &[];
                } else {
                    data = &data[offset..][total_size..];
                }

                adu
            };

            if state.needs_discont {
                gst::log!(CAT, imp = self, "Forcing discont for incoming {adu}");

                adu.set_discont();
                state.needs_discont = false;
            }

            // ADU deinterleaving
            if let Err(err) = state
                .deint_buf
                .try_push_and_pop(&self.obj(), adu, &mut ready_adus)
            {
                gst::element_imp_warning!(self, gst::ResourceError::Read, ["{err}"]);

                state.needs_discont = true;

                return Err(gst::FlowError::Error);
            }

            idx += 1;
        }

        if let Some(ready_adus) = ready_adus.into() {
            if let Some(mp3frames) = state
                .pending_adus
                .push_adus_pop_mp3_frames(&self.obj(), ready_adus)
                .into()
            {
                self.finish_buffer_or_list(&mut state, mp3frames)?;
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

#[derive(Debug)]
struct PtsOffsetHandler {
    packet_pts: BTreeMap<u64, gst::ClockTime>,
    base_ext_seqnum: Option<u64>,
    base_pts: gst::Signed<gst::ClockTime>,
    pts: gst::ClockTime,
}

impl Default for PtsOffsetHandler {
    fn default() -> Self {
        PtsOffsetHandler {
            packet_pts: BTreeMap::new(),
            base_ext_seqnum: None,
            base_pts: gst::ClockTime::ZERO.into_positive(),
            pts: gst::ClockTime::ZERO,
        }
    }
}

impl PtsOffsetHandler {
    fn register_packet(&mut self, ext_seqnum: u64, pts: gst::ClockTime) {
        let _ = self.packet_pts.insert(ext_seqnum, pts);
    }

    fn flush(&mut self) {
        self.packet_pts.clear();
        self.base_ext_seqnum = None;
        self.base_pts = gst::ClockTime::ZERO.into_positive();
        self.pts = gst::ClockTime::ZERO;
    }

    fn rebase_from(&mut self, ext_seqnum: u64) {
        match self.base_ext_seqnum {
            Some(ref mut base_ext_seqnum) if ext_seqnum > *base_ext_seqnum => {
                *base_ext_seqnum = ext_seqnum;
                self.packet_pts.retain(|k, _| k >= base_ext_seqnum);
                self.base_pts = self
                    .packet_pts
                    .get(base_ext_seqnum)
                    .expect("packet not registered")
                    .into_positive();
            }
            Some(ref mut base_ext_seqnum) if ext_seqnum == *base_ext_seqnum => (),
            None => {
                self.base_ext_seqnum = Some(ext_seqnum);
                let base_pts = self
                    .packet_pts
                    .get(&ext_seqnum)
                    .expect("packet not registered");
                self.base_pts = base_pts.into_positive();
                self.pts = *base_pts;
            }
            _ => unreachable!("ext_seqnum going backwards"),
        }
    }

    fn get_and_crank(&mut self, duration: gst::ClockTime) -> gst::Signed<gst::ClockTime> {
        let pts_offset = self.pts.into_positive() - self.base_pts;
        self.pts += duration;

        pts_offset
    }
}

impl RtpMpegAudioRobustDepay {
    fn queue_mp3frame(
        &self,
        state: &mut State,
        seqnums: RangeInclusive<u64>,
        mp3frame: Mp3Frame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(
            CAT,
            imp = self,
            "Finishing {mp3frame} DISCONT: {}",
            mp3frame.buf.flags().contains(gst::BufferFlags::DISCONT),
        );

        let Mp3Frame {
            header: frame_header,
            buf,
            ..
        } = mp3frame;

        if state.last_frame_header.as_ref() != Some(&frame_header) {
            let src_caps = gst::Caps::builder("audio/mpeg")
                .field("mpegversion", 1i32)
                .field("mpegaudioversion", frame_header.version as i32)
                .field("layer", frame_header.layer as i32)
                .field("rate", frame_header.sample_rate as i32)
                .field("channels", frame_header.channels as i32)
                .field("parsed", true)
                .build();

            gst::info!(CAT, imp = self, "Setting output caps {src_caps}");
            self.obj().set_src_caps(&src_caps);
        }

        state.last_frame_header = Some(frame_header);

        let pts_offset = state
            .pts_offset_hdlr
            .get_and_crank(buf.duration().expect("set when terminating MP3 frame"));

        let packet_to_buffer_relation = PacketToBufferRelation::SeqnumsWithOffset {
            seqnums,
            timestamp_offset: TimestampOffset::Pts(pts_offset),
        };

        self.obj().queue_buffer(packet_to_buffer_relation, buf)
    }

    fn finish_buffer_or_list(
        &self,
        state: &mut State,
        mp3frames: SingleMp3FrameOrList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        match mp3frames {
            SingleMp3FrameOrList::Single(mp3frame) => {
                let ext_seqnum = mp3frame.adu_id.provenance.ext_seqnum;
                state.pts_offset_hdlr.rebase_from(ext_seqnum);

                self.queue_mp3frame(state, ext_seqnum..=ext_seqnum, mp3frame)?;
            }
            SingleMp3FrameOrList::List(mp3frames) => {
                let mut seqnums = None;
                for mp3frame in &mp3frames {
                    let ext_seqnum = mp3frame.adu_id.provenance.ext_seqnum;
                    match seqnums {
                        None => {
                            seqnums = Some(ext_seqnum..=ext_seqnum);
                        }
                        Some(ref mut seqnums) => {
                            if ext_seqnum < *seqnums.start() {
                                *seqnums = ext_seqnum..=*seqnums.end();
                            } else if ext_seqnum >= *seqnums.end() {
                                *seqnums = *seqnums.start()..=ext_seqnum;
                            }
                        }
                    }
                }

                if let Some(seqnums) = seqnums {
                    state.pts_offset_hdlr.rebase_from(*seqnums.start());

                    for mp3frame in mp3frames {
                        self.queue_mp3frame(state, seqnums.clone(), mp3frame)?;
                    }
                } else {
                    return Ok(gst::FlowSuccess::Ok);
                }
            }
        }

        self.obj().finish_pending_buffers()
    }
}
