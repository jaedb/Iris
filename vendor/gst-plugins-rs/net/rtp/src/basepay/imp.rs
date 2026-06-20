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
use gst_rtp::prelude::*;

use smallvec::SmallVec;
use std::sync::LazyLock;

use std::{
    collections::{BTreeMap, VecDeque},
    num::Wrapping,
    ops::{Bound, RangeBounds},
    sync::Mutex,
};

use super::PacketToBufferRelation;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpbasepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP Base Payloader 2"),
    )
});

#[derive(Clone, Debug)]
struct Settings {
    mtu: u32,
    pt: u8,
    pt_set: bool, // If the pt was set after creation
    ssrc: Option<u32>,
    timestamp_offset: Option<u32>,
    seqnum_offset: Option<u16>,
    onvif_no_rate_control: bool,
    scale_rtptime: bool,
    source_info: bool,
    auto_header_extensions: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // TODO: Should we use a different default here? libwebrtc uses 1200, for example
            mtu: 1400,
            pt: 96,
            pt_set: false,
            ssrc: None,
            timestamp_offset: None,
            seqnum_offset: None,
            onvif_no_rate_control: false,
            scale_rtptime: true,
            source_info: false,
            auto_header_extensions: true,
        }
    }
}

#[derive(Debug)]
struct Stream {
    pt: u8,
    ssrc: u32,
    timestamp_offset: u32,
    #[allow(dead_code)]
    seqnum_offset: u16,
    /// Set if onvif_no_rate_control || !scale_rtptime
    use_stream_time: bool,

    /// Last seqnum that was put on a packet. This is initialized with stream.seqnum_offset.
    last_seqnum: Wrapping<u16>,

    /// Last RTP timestamp that was put on a packet. This is initialized with stream.timestamp_offset.
    last_timestamp: Wrapping<u32>,
}

/// Metadata of a pending buffer for tracking purposes.
struct PendingBuffer {
    /// Buffer id of this buffer.
    id: u64,
    buffer: gst::Buffer,
}

/// A pending packet that still has to be sent downstream.
struct PendingPacket {
    /// If no PTS is set then this packet also has no timestamp yet
    /// and must be pushed together with the next packet where
    /// it is actually known, or at EOS with the last known one.
    buffer: gst::Buffer,
}

struct State {
    segment: Option<(gst::Seqnum, gst::FormattedSegment<gst::ClockTime>)>,
    /// Set when a new segment event should be sent downstream before the next buffer.
    pending_segment: bool,

    sink_caps: Option<gst::Caps>,
    src_caps: Option<gst::Caps>,
    negotiated_src_caps: Option<gst::Caps>,

    /// Set when the srcpad caps are known.
    clock_rate: Option<u32>,
    /// Stream configuration. Set in Ready->Paused.
    stream: Option<Stream>,

    /// Set when the next outgoing buffer should have the discont flag set.
    discont_pending: bool,

    /// Buffer id of the next buffer.
    current_buffer_id: u64,
    /// Last buffer id that was used for a packet.
    last_used_buffer_id: u64,

    /// Last PTS that was put on an outgoing buffer.
    last_pts: Option<gst::ClockTime>,

    /// Last PTS / RTP time mapping.
    ///
    /// This is reset when draining and used if the subclass provides RTP timestamp offsets
    /// to get the base mapping.
    last_pts_rtp_mapping: Option<(gst::ClockTime, u32)>,

    /// Pending input buffers that were not completely used up for outgoing packets yet.
    pending_buffers: VecDeque<PendingBuffer>,
    /// Pending packets that have to be pushed downstream. Some of them might not have a PTS or RTP
    /// timestamp yet.
    pending_packets: VecDeque<PendingPacket>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            segment: None,
            pending_segment: false,

            sink_caps: None,
            src_caps: None,
            negotiated_src_caps: None,

            clock_rate: None,
            stream: None,

            discont_pending: true,

            current_buffer_id: 0,
            last_used_buffer_id: 0,

            last_pts: None,

            last_pts_rtp_mapping: None,

            pending_buffers: VecDeque::new(),
            pending_packets: VecDeque::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct Stats {
    ssrc: u32,
    pt: u8,
    // Only known once the caps are known
    clock_rate: Option<u32>,
    running_time: Option<gst::ClockTime>,
    seqnum: u16,
    timestamp: u32,
    seqnum_offset: u16,
    timestamp_offset: u32,
}

pub struct RtpBasePay2 {
    sink_pad: gst::Pad,
    src_pad: gst::Pad,
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
    stats: Mutex<Option<Stats>>,

    /// If Some then there was an SSRC collision and we should switch to the
    /// provided SSRC here.
    ssrc_collision: Mutex<Option<u32>>,
    /// Currently configured header extensions
    extensions: Mutex<BTreeMap<u8, gst_rtp::RTPHeaderExtension>>,
}

/// Wrappers for public methods and associated helper functions.
#[allow(dead_code)]
impl RtpBasePay2 {
    pub(super) fn mtu(&self) -> u32 {
        self.settings.lock().unwrap().mtu
    }

    pub(super) fn max_payload_size(&self) -> u32 {
        let settings = self.settings.lock().unwrap();

        // FIXME: This does not consider the space needed for header extensions. Doing so would
        // require knowing the buffer id range so we can calculate the maximum here.
        settings
            .mtu
            .saturating_sub(if settings.source_info {
                rtp_types::RtpPacket::MAX_N_CSRCS as u32 * 4
            } else {
                0
            })
            .saturating_sub(rtp_types::RtpPacket::MIN_RTP_PACKET_LEN as u32)
    }

    pub(super) fn set_src_caps(&self, src_caps: &gst::Caps) {
        gst::debug!(CAT, imp = self, "Setting src caps {src_caps:?}");

        let s = src_caps.structure(0).unwrap();
        assert!(
            s.has_name("application/x-rtp"),
            "Non-RTP caps {src_caps:?} not supported"
        );

        let mut state = self.state.borrow_mut();
        state.src_caps = Some(src_caps.clone());
        drop(state);

        self.negotiate();
    }

    fn negotiate(&self) {
        let state = self.state.borrow_mut();
        let Some(ref src_caps) = state.src_caps else {
            gst::debug!(CAT, imp = self, "No src caps set yet, can't negotiate");
            self.src_pad.mark_reconfigure();
            return;
        };
        let mut src_caps = src_caps.clone();
        drop(state);

        self.src_pad.check_reconfigure();
        gst::debug!(CAT, imp = self, "Configured src caps: {src_caps:?}");

        let peer_caps = self.src_pad.peer_query_caps(Some(&src_caps));
        if !peer_caps.is_empty() {
            gst::debug!(CAT, imp = self, "Peer caps: {peer_caps:?}");
            src_caps = peer_caps;
        } else {
            gst::debug!(CAT, imp = self, "Empty peer caps");
        }
        gst::debug!(CAT, imp = self, "Negotiating with caps {src_caps:?}");

        src_caps.make_mut();
        let obj = self.obj();
        (obj.class().as_ref().negotiate)(&obj, src_caps);
    }

    pub(super) fn drop_buffers(&self, ids: impl RangeBounds<u64>) {
        gst::trace!(
            CAT,
            imp = self,
            "Dropping buffers up to {:?}",
            ids.end_bound()
        );

        let mut state = self.state.borrow_mut();
        let end = match ids.end_bound() {
            Bound::Included(end) => *end,
            Bound::Excluded(end) if *end == 0 => return,
            Bound::Excluded(end) => *end - 1,
            Bound::Unbounded => {
                state.pending_buffers.clear();
                return;
            }
        };

        if let Some(back) = state.pending_buffers.back() {
            if back.id <= end {
                state.pending_buffers.clear();
                return;
            }
        } else {
            return;
        }

        while state.pending_buffers.front().is_some_and(|b| b.id <= end) {
            let _ = state.pending_buffers.pop_front();
        }
    }

    pub(super) fn next_seqnum(&self) -> u16 {
        let state = self.state.borrow();
        let Some(stream) = state.stream.as_ref() else {
            return 0;
        };
        let mut seqnum = stream.last_seqnum;
        seqnum += 1;
        seqnum.0
    }

    pub(super) fn queue_packet(
        &self,
        packet_to_buffer_relation: PacketToBufferRelation,
        mut packet: rtp_types::RtpPacketBuilder<&[u8], &[u8]>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(
            CAT,
            imp = self,
            "Queueing packet for {packet_to_buffer_relation:?}",
        );

        let settings = self.settings.lock().unwrap().clone();

        let mut state = self.state.borrow_mut();
        if state.negotiated_src_caps.is_none() {
            gst::error!(CAT, imp = self, "No source pad caps negotiated yet");
            return Err(gst::FlowError::NotNegotiated);
        }

        let stream = state.stream.as_mut().unwrap();
        stream.last_seqnum += 1;
        let seqnum = stream.last_seqnum.0;
        gst::trace!(CAT, imp = self, "Using seqnum {seqnum}");
        packet = packet
            .payload_type(stream.pt)
            .ssrc(stream.ssrc)
            .sequence_number(seqnum);
        let use_stream_time = stream.use_stream_time;

        // Queue up packet for later if it doesn't have any associated buffers that could be used
        // for figuring out the timestamp.
        if matches!(packet_to_buffer_relation, PacketToBufferRelation::OutOfBand) {
            gst::trace!(
                CAT,
                imp = self,
                "Keeping packet without associated buffer until next packet or EOS"
            );

            // FIXME: Use more optimal packet writing API once available
            // https://github.com/ystreet/rtp-types/issues/4
            // https://github.com/ystreet/rtp-types/issues/5
            // TODO: Maybe use an MTU-sized buffer pool?
            let packet_buffer = packet.write_vec().map_err(|err| {
                gst::error!(CAT, imp = self, "Can't write packet: {err}");
                gst::FlowError::Error
            })?;
            let packet_len = packet_buffer.len();

            if (settings.mtu as usize) < packet_len {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Generated packet has bigger size {packet_len} than MTU {}",
                    settings.mtu
                );
            }

            gst::trace!(CAT, imp = self, "Queueing packet of size {packet_len}");

            let marker = {
                let packet = rtp_types::RtpPacket::parse(&packet_buffer).unwrap();
                packet.marker_bit()
            };

            let mut buffer = gst::Buffer::from_mut_slice(packet_buffer);
            {
                let buffer = buffer.get_mut().unwrap();

                if marker {
                    buffer.set_flags(gst::BufferFlags::MARKER);
                }

                if state.discont_pending {
                    buffer.set_flags(gst::BufferFlags::DISCONT);
                    state.discont_pending = false;
                }
            }

            state.pending_packets.push_back(PendingPacket { buffer });

            return Ok(gst::FlowSuccess::Ok);
        };

        let (ids, timestamp_offset) = match packet_to_buffer_relation {
            PacketToBufferRelation::Ids(ids) => (ids.clone(), None),
            PacketToBufferRelation::IdsWithOffset {
                ids,
                timestamp_offset,
            } => (ids.clone(), Some(timestamp_offset)),
            PacketToBufferRelation::OutOfBand => unreachable!(),
        };

        if ids.is_empty() {
            gst::error!(CAT, imp = self, "Empty buffer id range provided");
            return Err(gst::FlowError::Error);
        }

        let id_start = *ids.start();
        let id_end = *ids.end();

        // Drop all older buffers
        while state
            .pending_buffers
            .front()
            .is_some_and(|b| b.id < id_start)
        {
            let b = state.pending_buffers.pop_front().unwrap();
            gst::trace!(CAT, imp = self, "Dropping buffer with id {}", b.id);
        }

        let Some(front) = state.pending_buffers.front() else {
            gst::error!(
                CAT,
                imp = self,
                "Queueing packet for future buffer ids not allowed"
            );
            return Err(gst::FlowError::Error);
        };

        if id_end > state.pending_buffers.back().unwrap().id {
            gst::error!(
                CAT,
                imp = self,
                "Queueing packet for future buffer ids not allowed"
            );
            return Err(gst::FlowError::Error);
        }

        // Set when caps are set
        let clock_rate = state.clock_rate.unwrap();

        // Clip PTS to the segment boundaries
        let segment = state.segment.as_ref().unwrap().1.clone();
        let pts = front.buffer.pts().unwrap();
        let pts = {
            let start = segment.start().expect("no segment start");
            let stop = segment.stop();

            if pts <= start {
                start
            } else if let Some(stop) = stop {
                if pts >= stop {
                    stop
                } else {
                    pts
                }
            } else {
                pts
            }
        };

        let rtptime_base = if use_stream_time {
            segment.to_stream_time(pts).unwrap()
        } else {
            segment.to_running_time(pts).unwrap()
        };

        let packet_pts;
        let packet_rtptime;
        if let Some(timestamp_offset) = timestamp_offset {
            match timestamp_offset {
                crate::basepay::TimestampOffset::Pts(pts_diff) => {
                    if let Some(stop) = segment.stop() {
                        if pts + pts_diff > stop {
                            packet_pts = stop
                        } else {
                            packet_pts = pts + pts_diff
                        }
                    } else {
                        packet_pts = pts + pts_diff
                    }

                    let packet_rtptime_base = if use_stream_time {
                        segment.to_stream_time(packet_pts).unwrap()
                    } else {
                        segment.to_running_time(packet_pts).unwrap()
                    };

                    packet_rtptime = (*packet_rtptime_base
                        .mul_div_ceil(clock_rate as u64, *gst::ClockTime::SECOND)
                        .unwrap()
                        & 0xffff_ffff) as u32;

                    state.last_pts_rtp_mapping = Some((packet_pts, packet_rtptime));
                }
                crate::basepay::TimestampOffset::Rtp(rtp_diff) => {
                    let Some((base_pts, base_rtptime)) = state.last_pts_rtp_mapping else {
                        gst::error!(CAT, imp = self, "Have no base PTS / RTP time mapping");
                        return Err(gst::FlowError::Error);
                    };

                    packet_pts = base_pts
                        + gst::ClockTime::from_nseconds(
                            rtp_diff
                                .mul_div_ceil(*gst::ClockTime::SECOND, clock_rate as u64)
                                .unwrap(),
                        );

                    let rate = if use_stream_time {
                        segment.applied_rate()
                    } else {
                        1.0 / segment.rate()
                    };

                    let rtp_diff = if rate != 0.0 {
                        (rtp_diff as f64 * rate) as u64
                    } else {
                        rtp_diff
                    };

                    packet_rtptime = base_rtptime + (rtp_diff & 0xffff_ffff) as u32;
                }
            }
        } else {
            packet_pts = pts;
            packet_rtptime = (*rtptime_base
                .mul_div_ceil(clock_rate as u64, *gst::ClockTime::SECOND)
                .unwrap()
                & 0xffff_ffff) as u32;

            state.last_pts_rtp_mapping = Some((packet_pts, packet_rtptime));
        }

        // Check if any discont is pending here
        let mut discont_pending = state.discont_pending
            || (state.last_used_buffer_id < id_end
                && state
                    .pending_buffers
                    .iter()
                    .skip_while(|b| b.id <= state.last_used_buffer_id)
                    .take_while(|b| b.id <= id_end)
                    .any(|b| b.buffer.flags().contains(gst::BufferFlags::DISCONT)));
        state.discont_pending = false;

        let stream = state.stream.as_ref().unwrap();
        let packet_rtptime = stream.timestamp_offset.wrapping_add(packet_rtptime);

        // update pts and rtp timestamp of all pending packets that have none yet
        for packet in state
            .pending_packets
            .iter_mut()
            .skip_while(|p| p.buffer.pts().is_some())
        {
            assert!(packet.buffer.pts().is_none());
            let buffer = packet.buffer.get_mut().unwrap();
            buffer.set_pts(pts);

            if discont_pending {
                buffer.set_flags(gst::BufferFlags::DISCONT);
                discont_pending = false;
            }

            let mut map = buffer.map_writable().unwrap();
            let mut packet = rtp_types::RtpPacketMut::parse(&mut map).unwrap();
            packet.set_timestamp(packet_rtptime);
        }

        packet = packet.timestamp(packet_rtptime);

        if settings.source_info
        /* || header_extensions */
        {
            let mut csrc = [0u32; rtp_types::RtpPacket::MAX_N_CSRCS];
            let mut n_csrc = 0;

            // Copy over metas and other metadata from the packets that made up this buffer
            for front in state.pending_buffers.iter().take_while(|b| b.id <= id_end) {
                if n_csrc < rtp_types::RtpPacket::MAX_N_CSRCS {
                    if let Some(meta) = front.buffer.meta::<gst_rtp::RTPSourceMeta>() {
                        if let Some(ssrc) = meta.ssrc() {
                            if !csrc[..n_csrc].contains(&ssrc) {
                                csrc[n_csrc] = ssrc;
                                n_csrc += 1;
                            }
                        }

                        for c in meta.csrc() {
                            if n_csrc >= rtp_types::RtpPacket::MAX_N_CSRCS {
                                break;
                            }

                            if !csrc[..n_csrc].contains(c) {
                                csrc[n_csrc] = *c;
                                n_csrc += 1;
                            }
                        }
                    }
                }
            }

            if n_csrc > 0 {
                for c in &csrc[..n_csrc] {
                    packet = packet.add_csrc(*c);
                }
            }
        }

        // Calculate size and flags for header extensions

        // FIXME: We're only considering the first buffer that makes up this packet for RTP header
        // extensions.
        let extension_input_buffer = state
            .pending_buffers
            .iter()
            .take_while(|b| b.id <= id_end)
            .next()
            .expect("no input buffer for this packet");

        let extensions = self.extensions.lock().unwrap();
        let mut extension_flags =
            gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE | gst_rtp::RTPHeaderExtensionFlags::TWO_BYTE;
        let mut extension_size = 0;
        for extension in extensions.values() {
            extension_flags &= extension.supported_flags();

            let max_size = extension.max_size(&extension_input_buffer.buffer);
            let ext_id = extension.id();
            if max_size > 16 || ext_id > 14 {
                extension_flags -= gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE;
            }
            if max_size > 255 || ext_id > 255 {
                extension_flags -= gst_rtp::RTPHeaderExtensionFlags::TWO_BYTE;
            }
            extension_size += max_size;
        }

        let extension_pattern =
            if extension_flags.contains(gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE) {
                extension_flags = gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE;
                extension_size += extensions.len();
                0xBEDE
            } else if extension_flags.contains(gst_rtp::RTPHeaderExtensionFlags::TWO_BYTE) {
                extension_flags = gst_rtp::RTPHeaderExtensionFlags::TWO_BYTE;
                extension_size += 2 * extensions.len();
                0x1000
            } else {
                extension_flags = gst_rtp::RTPHeaderExtensionFlags::empty();
                extension_size = 0;
                0
            };

        // Round up to a multiple of 4 bytes
        extension_size = extension_size.next_multiple_of(4);

        // If there are extensions, write an empty extension area of the required size. If this is
        // not filled then it would be considered as padding inside the extension because of the
        // zeroes, which are not a valid extension ID.
        let mut extension_data = SmallVec::<[u8; 256]>::with_capacity(extension_size);
        extension_data.resize(extension_size, 0);

        let mut packet = packet;

        if !extension_data.is_empty() && extension_pattern != 0 {
            packet = packet.extension(extension_pattern, extension_data.as_slice());
        }

        // Queue up packet and timestamp all others without timestamp that are before it.
        // FIXME: Use more optimal packet writing API once available
        // https://github.com/ystreet/rtp-types/issues/4
        // https://github.com/ystreet/rtp-types/issues/5
        // TODO: Maybe use an MTU-sized buffer pool?
        let packet_buffer = packet.write_vec().map_err(|err| {
            gst::error!(CAT, imp = self, "Can't write packet: {err}");
            gst::FlowError::Error
        })?;
        let packet_len = packet_buffer.len();

        // FIXME: See comment in `max_payload_size()`. We currently don't provide a way to the
        // subclass to know how much space will be used up by extensions.
        if (settings.mtu as usize) < packet_len - extension_size {
            gst::warning!(
                CAT,
                imp = self,
                "Generated packet has bigger size {packet_len} than MTU {}",
                settings.mtu
            );
        }

        gst::trace!(
            CAT,
            imp = self,
            "Queueing packet of size {packet_len} with RTP timestamp {packet_rtptime} and PTS {packet_pts}",
        );

        let marker = {
            let packet = rtp_types::RtpPacket::parse(&packet_buffer).unwrap();
            packet.marker_bit()
        };

        let mut buffer = gst::Buffer::from_mut_slice(packet_buffer);

        {
            let buffer = buffer.get_mut().unwrap();

            buffer.set_pts(packet_pts);
            if marker {
                buffer.set_flags(gst::BufferFlags::MARKER);
            }

            if discont_pending {
                buffer.set_flags(gst::BufferFlags::DISCONT);
            }

            // Copy over metas and other metadata from the packets that made up this buffer
            let obj = self.obj();
            for front in state.pending_buffers.iter().take_while(|b| b.id <= id_end) {
                use std::ops::ControlFlow::*;

                front.buffer.foreach_meta(|meta| {
                    // Do not copy metas that are memory specific
                    if meta.has_tag::<gst::meta::tags::Memory>()
                        || meta.has_tag::<gst::meta::tags::MemoryReference>()
                    {
                        return Continue(());
                    }

                    (obj.class().as_ref().transform_meta)(&obj, &front.buffer, &meta, buffer);

                    Continue(())
                });
            }
        }

        // Finally write extensions into the otherwise complete packet
        if extension_size != 0 && extension_pattern != 0 {
            let buffer = buffer.get_mut().unwrap();
            // FIXME Get a mutable reference to the output buffer via a pointer to
            // work around bug in the C API.
            // See https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/issues/375
            let buffer_ptr = buffer.as_mut_ptr();

            let mut map = buffer.map_writable().unwrap();
            let mut packet = rtp_types::RtpPacketMut::parse(&mut map).unwrap();

            let mut extension_data = packet.extension_mut().unwrap();

            for (ext_id, extension) in extensions.iter() {
                let offset = if extension_flags == gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE {
                    1
                } else {
                    2
                };

                if extension_data.len() < offset {
                    gst::error!(
                        CAT,
                        imp = self,
                        "No space left for writing RTP header extension {ext_id}"
                    );
                    break;
                }

                if let Ok(written) = extension.write(
                    &extension_input_buffer.buffer,
                    extension_flags,
                    unsafe { gst::BufferRef::from_mut_ptr(buffer_ptr) },
                    &mut extension_data[offset..],
                ) {
                    // Nothing written, can just continue
                    if written == 0 {
                        continue;
                    }

                    if extension_flags == gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE {
                        assert!(written <= 16);
                        extension_data[0] = (*ext_id << 4) | (written as u8 - 1);
                    } else {
                        assert!(written <= 255);
                        extension_data[0] = *ext_id;
                        extension_data[1] = written as u8;
                    }

                    extension_data = &mut extension_data[offset + written..];
                } else {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Writing RTP header extension {ext_id} failed"
                    );
                }
            }
        }

        drop(extensions);

        state.pending_packets.push_back(PendingPacket { buffer });

        state.last_used_buffer_id = id_end;
        state.stream.as_mut().unwrap().last_timestamp = Wrapping(packet_rtptime);
        state.last_pts = Some(pts);

        // Update stats
        if let Some(stats) = self.stats.lock().unwrap().as_mut() {
            stats.running_time = segment.to_running_time(packet_pts);
            stats.seqnum = seqnum;
            stats.timestamp = packet_rtptime;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    pub(super) fn finish_pending_packets(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if state.segment.is_none() {
            if !state.pending_buffers.is_empty() {
                // The queued buffers must be based on the caps only. They can be forwarded at a later
                // time, if there is one.
                gst::debug!(CAT, imp = self, "Can't finish buffers yet without segment");
            }
            return Ok(gst::FlowSuccess::Ok);
        }

        // As long as there are packets that can be finished, take all with the same PTS and put
        // them into a buffer list.
        while state
            .pending_packets
            .iter()
            .any(|p| p.buffer.pts().is_some())
        {
            let pts = state.pending_packets.front().unwrap().buffer.pts().unwrap();

            // First count number of pending packets so we can allocate a buffer list with the correct
            // size instead of re-allocating for every added buffer, or simply push a single buffer
            // downstream instead of allocating a buffer list just for one buffer.
            let num_buffers = state
                .pending_packets
                .iter()
                .take_while(|p| p.buffer.pts() == Some(pts))
                .count();

            assert_ne!(num_buffers, 0);

            gst::trace!(CAT, imp = self, "Flushing {num_buffers} packets");

            let segment_event = self.retrieve_pending_segment_event(&mut state);

            enum BufferOrList {
                Buffer(gst::Buffer),
                List(gst::BufferList),
            }

            let packets = if num_buffers == 1 {
                let buffer = state.pending_packets.pop_front().unwrap().buffer;
                gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}");
                BufferOrList::Buffer(buffer)
            } else {
                let mut list = gst::BufferList::new_sized(num_buffers);
                {
                    let list = list.get_mut().unwrap();
                    while state
                        .pending_packets
                        .front()
                        .is_some_and(|p| p.buffer.pts() == Some(pts))
                    {
                        let buffer = state.pending_packets.pop_front().unwrap().buffer;
                        gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}");
                        list.add(buffer);
                    }
                }

                BufferOrList::List(list)
            };

            drop(state);

            if let Some(segment_event) = segment_event {
                let _ = self.src_pad.push_event(segment_event);
            }

            let res = match packets {
                BufferOrList::Buffer(buffer) => self.src_pad.push(buffer),
                BufferOrList::List(list) => self.src_pad.push_list(list),
            };

            if let Err(err) = res {
                if ![gst::FlowError::Flushing, gst::FlowError::Eos].contains(&err) {
                    gst::warning!(CAT, imp = self, "Failed pushing packets: {err:?}");
                } else {
                    gst::debug!(CAT, imp = self, "Failed pushing packets: {err:?}");
                }
                return Err(err);
            }

            state = self.state.borrow_mut();
        }

        Ok(gst::FlowSuccess::Ok)
    }

    pub(super) fn sink_pad(&self) -> &gst::Pad {
        &self.sink_pad
    }

    pub(super) fn src_pad(&self) -> &gst::Pad {
        &self.src_pad
    }
}

/// Default virtual method implementations.
impl RtpBasePay2 {
    fn start_default(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn stop_default(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn set_sink_caps_default(&self, _caps: &gst::Caps) -> bool {
        true
    }

    fn negotiate_header_extensions(
        &self,
        state: &State,
        src_caps: &mut gst::Caps,
    ) -> Result<(), gst::FlowError> {
        let s = src_caps.structure(0).unwrap();

        // Check which header extensions to enable/disable based on the caps.
        let mut caps_extensions = BTreeMap::new();
        for (k, v) in s.iter() {
            let Some(ext_id) = k.strip_prefix("extmap-") else {
                continue;
            };
            let Ok(ext_id) = ext_id.parse::<u8>() else {
                gst::error!(
                    CAT,
                    imp = self,
                    "Can't parse RTP header extension id from caps {src_caps:?}"
                );
                return Err(gst::FlowError::NotNegotiated);
            };

            let uri = if let Ok(uri) = v.get::<String>() {
                uri
            } else if let Ok(arr) = v.get::<gst::ArrayRef>() {
                if let Some(uri) = arr.get(1).and_then(|v| v.get::<String>().ok()) {
                    uri
                } else {
                    gst::error!(CAT, imp = self, "Couldn't get URI for RTP header extension id {ext_id} from caps {src_caps:?}");
                    return Err(gst::FlowError::NotNegotiated);
                }
            } else {
                gst::error!(
                    CAT,
                    imp = self,
                    "Couldn't get URI for RTP header extension id {ext_id} from caps {src_caps:?}"
                );
                return Err(gst::FlowError::NotNegotiated);
            };

            caps_extensions.insert(ext_id, uri);
        }

        let mut extensions = self.extensions.lock().unwrap();
        let mut extensions_changed = false;
        for (ext_id, uri) in &caps_extensions {
            if let Some(extension) = extensions.get(ext_id) {
                if extension.uri().as_deref() == Some(uri) {
                    // Same extension, update it with the new caps in case the attributes changed
                    if extension.set_attributes_from_caps(src_caps) {
                        continue;
                    }

                    // Try to get a new one for this extension ID instead
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to configure extension {ext_id} from caps {src_caps}"
                    );
                    extensions_changed |= true;
                    extensions.remove(ext_id);
                } else {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Extension ID {ext_id} changed from {:?} to {uri}",
                        extension.uri(),
                    );
                    extensions_changed |= true;
                    extensions.remove(ext_id);
                }
            }

            gst::debug!(
                CAT,
                imp = self,
                "Requesting extension {uri} for ID {ext_id}"
            );
            let ext = self
                .obj()
                .emit_by_name::<Option<gst_rtp::RTPHeaderExtension>>(
                    "request-extension",
                    &[&(*ext_id as u32), &uri],
                );

            let Some(ext) = ext else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Couldn't create extension for {uri} with ID {ext_id}"
                );
                continue;
            };

            if ext.id() != *ext_id as u32 {
                gst::warning!(CAT, imp = self, "Created extension has wrong ID");
                continue;
            }

            if !ext.set_attributes_from_caps(src_caps) {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to configure extension {ext_id} from caps {src_caps}"
                );
                continue;
            }

            extensions.insert(*ext_id, ext);
            extensions_changed |= true;
        }

        let sink_caps = state.sink_caps.as_ref().unwrap();
        let mut to_remove = vec![];
        for (ext_id, ext) in extensions.iter() {
            if !ext.set_non_rtp_sink_caps(sink_caps) {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to configure extension {ext_id} from sink caps {sink_caps}"
                );
                to_remove.push(*ext_id);
            }
        }
        for ext_id in to_remove {
            extensions.remove(&ext_id);
            extensions_changed |= true;
        }

        // Add extension information for all actually selected extensions to the caps.
        //
        // This will override the values for extensions that are already represented in the caps
        // but will leave extensions in the caps that couldn't be configured above and are not in
        // the extensions list anymore. This is necessary so that the created caps stay compatible.
        {
            let caps = src_caps.make_mut();
            for (_, ext) in extensions.iter() {
                ext.set_caps_from_attributes(caps);
            }
        }
        drop(extensions);

        if extensions_changed {
            self.obj().notify("extensions");
        }

        Ok(())
    }

    fn negotiate_default(&self, mut src_caps: gst::Caps) {
        gst::debug!(CAT, imp = self, "Negotiating caps {src_caps:?}");

        // Fixate what is left to fixate.
        src_caps.fixate();

        let s = src_caps.structure(0).unwrap();
        assert!(
            s.has_name("application/x-rtp"),
            "Non-RTP caps {src_caps:?} not supported"
        );

        let clock_rate = match s.get::<i32>("clock-rate") {
            Ok(clock_rate) if clock_rate > 0 => clock_rate as u32,
            _ => {
                panic!("RTP caps {src_caps:?} without 'clock-rate'");
            }
        };

        let caps_ssrc = s.get::<u32>("ssrc").ok();
        let caps_pt = s
            .get::<i32>("payload")
            .ok()
            .filter(|pt| *pt > 0)
            .map(|pt| pt as u8);

        // We're not negotiating seqnum-offset and timestamp-offset with downstream
        // and also don't set it in the caps. The old payloader base class does this
        // but this is not actually used anywhere.

        let mut state = self.state.borrow_mut();
        if self
            .negotiate_header_extensions(&state, &mut src_caps)
            .is_err()
        {
            state.negotiated_src_caps = None;
            return;
        }

        let ssrc_collision = self.ssrc_collision.lock().unwrap().take();
        if state.stream.is_none() {
            use rand::prelude::*;
            let mut rng = rand::rng();
            let settings = self.settings.lock().unwrap();

            let pt = if settings.pt_set {
                settings.pt
            } else {
                caps_pt.unwrap_or(settings.pt)
            };
            let ssrc = ssrc_collision
                .or(settings.ssrc)
                .or(caps_ssrc)
                .unwrap_or_else(|| rng.random::<u32>());
            let timestamp_offset = settings
                .timestamp_offset
                .unwrap_or_else(|| rng.random::<u32>());
            let seqnum_offset = settings
                .seqnum_offset
                .unwrap_or_else(|| rng.random::<u16>());
            let stream = Stream {
                pt,
                ssrc,
                timestamp_offset,
                seqnum_offset,
                use_stream_time: settings.onvif_no_rate_control || !settings.scale_rtptime,
                last_seqnum: Wrapping(seqnum_offset) - Wrapping(1),
                last_timestamp: Wrapping(timestamp_offset),
            };

            gst::info!(CAT, imp = self, "Configuring {stream:?}");

            state.stream = Some(stream);
            *self.stats.lock().unwrap() = Some(Stats {
                ssrc,
                pt,
                clock_rate: None,
                running_time: None,
                seqnum: seqnum_offset,
                timestamp: timestamp_offset,
                seqnum_offset,
                timestamp_offset,
            });
        } else if let Some(ssrc_collision) = ssrc_collision {
            let stream = state.stream.as_mut().unwrap();

            gst::debug!(
                CAT,
                imp = self,
                "Switching from SSRC {} to {} because of SSRC collision",
                stream.ssrc,
                ssrc_collision,
            );

            stream.ssrc = ssrc_collision;
            self.stats.lock().unwrap().as_mut().unwrap().ssrc = ssrc_collision;
        }

        let stream = state.stream.as_ref().unwrap();
        let ssrc = stream.ssrc;
        let pt = stream.pt;

        // Set SSRC and payload type. We don't negotiate these with downstream!
        {
            let caps = src_caps.make_mut();
            caps.set("ssrc", ssrc);
            caps.set("payload", pt as i32);
        }

        if Some(&src_caps) == state.negotiated_src_caps.as_ref() {
            gst::debug!(CAT, imp = self, "Setting same caps {src_caps:?} again");
            return;
        }

        let clock_rate_changed = state
            .clock_rate
            .is_some_and(|old_clock_rate| old_clock_rate != clock_rate);
        state.negotiated_src_caps = Some(src_caps.clone());
        state.clock_rate = Some(clock_rate);
        self.stats.lock().unwrap().as_mut().unwrap().clock_rate = Some(clock_rate);

        let seqnum = if let Some((seqnum, _)) = state.segment {
            seqnum
        } else {
            gst::Seqnum::next()
        };

        let mut segment_event = self.retrieve_pending_segment_event(&mut state);
        drop(state);

        // FIXME: Drain also in other conditions?
        if clock_rate_changed || ssrc_collision.is_some() {
            // First push the pending segment event downstream before draining
            if let Some(segment_event) = segment_event.take() {
                let _ = self.src_pad.push_event(segment_event);
            }

            if let Err(err) = self.drain() {
                // Just continue here. The payloader is now flushed and can proceed below,
                // and if there was a serious error downstream it will happen on the next
                // buffer pushed downstream
                gst::debug!(CAT, imp = self, "Draining failed: {err:?}");
            }
        }

        // Rely on the actual flow return later when pushing a buffer downstream. The bool
        // return from pushing the event does not allow to distinguish between different kinds of
        // errors.
        let _ = self
            .src_pad
            .push_event(gst::event::Caps::builder(&src_caps).seqnum(seqnum).build());

        if let Some(segment_event) = segment_event {
            let _ = self.src_pad.push_event(segment_event);
        }
    }

    fn handle_buffer_default(
        &self,
        _buffer: &gst::Buffer,
        _id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        unimplemented!()
    }

    fn drain_default(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        Ok(gst::FlowSuccess::Ok)
    }

    fn flush_default(&self) {}

    fn sink_event_default(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        match event.view() {
            gst::EventView::Caps(event) => {
                let caps = event.caps_owned();

                return self.set_sink_caps(caps);
            }
            gst::EventView::Segment(event) => {
                let segment = event.segment();
                let seqnum = event.seqnum();

                let state = self.state.borrow();
                if state.sink_caps.is_none() {
                    gst::warning!(CAT, imp = self, "Received segment before caps");
                    return Err(gst::FlowError::NotNegotiated);
                }

                let same_segment = state
                    .segment
                    .as_ref()
                    .map(|(old_seqnum, old_segment)| {
                        (&seqnum, segment) == (old_seqnum, old_segment.upcast_ref())
                    })
                    .unwrap_or(false);
                // Only drain if the segment changed and we actually had one before
                let drain = !same_segment && state.segment.is_some();
                drop(state);

                if !same_segment {
                    gst::debug!(CAT, imp = self, "Received segment {segment:?}");

                    if drain {
                        if let Err(err) = self.drain() {
                            // Just continue here. The payloader is now flushed and can proceed below,
                            // and if there was a serious error downstream it will happen on the next
                            // buffer pushed downstream
                            gst::debug!(CAT, imp = self, "Draining failed: {err:?}");
                        }
                    }

                    let mut state = self.state.borrow_mut();
                    if let Some(segment) = segment.downcast_ref::<gst::ClockTime>() {
                        state.segment = Some((event.seqnum(), segment.clone()));
                    } else {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Segments in non-TIME format are not supported"
                        );
                        state.segment = None;
                        // FIXME: Forget everything here?
                        return Err(gst::FlowError::Error);
                    }

                    if state.negotiated_src_caps.is_none() {
                        state.pending_segment = true;
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Received segment before knowing src caps, delaying"
                        );
                        return Ok(gst::FlowSuccess::Ok);
                    }

                    drop(state);
                }
            }
            gst::EventView::Eos(_) => {
                if let Err(err) = self.drain() {
                    gst::debug!(CAT, imp = self, "Draining on EOS failed: {err:?}");
                }
            }
            gst::EventView::FlushStop(_) => {
                self.flush();

                let mut state = self.state.borrow_mut();
                state.segment = None;
                state.pending_segment = false;

                // RTP timestamp and seqnum are not reset here
                state.last_pts = None;

                drop(state);
            }
            _ => (),
        }

        gst::debug!(CAT, imp = self, "Forwarding event: {event:?}");
        if self.src_pad.push_event(event) {
            Ok(gst::FlowSuccess::Ok)
        } else if self.src_pad.pad_flags().contains(gst::PadFlags::FLUSHING) {
            Err(gst::FlowError::Flushing)
        } else {
            Err(gst::FlowError::Error)
        }
    }

    fn handle_ssrc_collision_event(&self, s: &gst::StructureRef) {
        let Ok(ssrc) = s.get::<u32>("ssrc") else {
            return;
        };

        let stats = self.stats.lock().unwrap();
        let Some(ref stats) = &*stats else {
            return;
        };

        if stats.ssrc != ssrc {
            return;
        }

        let new_ssrc = if let Some(suggested_ssrc) = s
            .get::<u32>("suggested-ssrc")
            .ok()
            .filter(|suggested_ssrc| *suggested_ssrc != stats.ssrc)
        {
            suggested_ssrc
        } else {
            use rand::prelude::*;
            let mut rng = rand::rng();

            loop {
                let new_ssrc = rng.random::<u32>();
                if new_ssrc != stats.ssrc {
                    break new_ssrc;
                }
            }
        };

        {
            let mut ssrc_collision = self.ssrc_collision.lock().unwrap();
            *ssrc_collision = Some(new_ssrc);
        }
    }

    fn src_event_default(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        if let gst::EventView::CustomUpstream(ev) = event.view() {
            if let Some(s) = ev.structure() {
                if s.name() == "GstRTPCollision" {
                    self.handle_ssrc_collision_event(s);
                }
            }
        }

        if gst::Pad::event_default(&self.src_pad, Some(&*self.obj()), event) {
            Ok(gst::FlowSuccess::Ok)
        } else if self.sink_pad.pad_flags().contains(gst::PadFlags::FLUSHING) {
            Err(gst::FlowError::Flushing)
        } else {
            Err(gst::FlowError::Error)
        }
    }

    fn sink_query_default(&self, query: &mut gst::QueryRef) -> bool {
        gst::Pad::query_default(&self.sink_pad, Some(&*self.obj()), query)
    }

    fn src_query_default(&self, query: &mut gst::QueryRef) -> bool {
        gst::Pad::query_default(&self.src_pad, Some(&*self.obj()), query)
    }

    fn transform_meta_default(
        &self,
        _in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ) {
        let tags = meta.tags();

        if tags.len() > 1 {
            gst::trace!(
                CAT,
                imp = self,
                "Not copying meta {}: has multiple tags {tags:?}",
                meta.api(),
            );
            return;
        }

        let allowed_tags = self.obj().class().as_ref().allowed_meta_tags;
        if tags.len() == 1 {
            let meta_tag = &tags[0];

            if !allowed_tags.iter().copied().any(|tag| tag == *meta_tag) {
                gst::trace!(
                    CAT,
                    imp = self,
                    "Not copying meta {}: tag '{meta_tag}' not allowed",
                    meta.api(),
                );
                return;
            }
        }

        gst::trace!(CAT, imp = self, "Copying meta {}", meta.api());

        if let Err(err) = meta.transform(out_buf, &gst::meta::MetaTransformCopy::new(..)) {
            gst::trace!(CAT, imp = self, "Could not copy meta {}: {err}", meta.api());
        }
    }

    fn add_extension(&self, ext: &gst_rtp::RTPHeaderExtension) {
        assert_ne!(ext.id(), 0);

        let mut extensions = self.extensions.lock().unwrap();
        extensions.insert(ext.id() as u8, ext.clone());
        self.src_pad.mark_reconfigure();
        drop(extensions);

        self.obj().notify("extensions");
    }

    fn clear_extensions(&self) {
        let mut extensions = self.extensions.lock().unwrap();
        extensions.clear();
        self.src_pad.mark_reconfigure();
        drop(extensions);

        self.obj().notify("extensions");
    }

    fn request_extension(&self, ext_id: u32, uri: &str) -> Option<gst_rtp::RTPHeaderExtension> {
        let settings = self.settings.lock().unwrap();
        if !settings.auto_header_extensions {
            return None;
        }
        drop(settings);

        let Some(ext) = gst_rtp::RTPHeaderExtension::create_from_uri(uri) else {
            gst::debug!(
                CAT,
                imp = self,
                "Didn't find any extension implementing URI {uri}",
            );
            return None;
        };

        gst::debug!(
            CAT,
            imp = self,
            "Automatically enabling extension {} for URI {uri}",
            ext.name(),
        );

        ext.set_id(ext_id);

        Some(ext)
    }
}

/// Pad functions and associated helper functions.
impl RtpBasePay2 {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Received buffer {buffer:?}");

        let settings = self.settings.lock().unwrap().clone();
        self.handle_buffer(&settings, buffer)
    }

    fn sink_chain_list(
        &self,
        _pad: &gst::Pad,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Received buffer list {list:?}");

        let settings = self.settings.lock().unwrap().clone();
        for buffer in list.iter_owned() {
            self.handle_buffer(&settings, buffer)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(
        &self,
        pad: &gst::Pad,
        event: gst::Event,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, obj = pad, "Received event: {event:?}");

        let obj = self.obj();
        (obj.class().as_ref().sink_event)(&obj, event)
    }

    fn src_event(
        &self,
        pad: &gst::Pad,
        event: gst::Event,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, obj = pad, "Received event: {event:?}");

        let obj = self.obj();
        (obj.class().as_ref().src_event)(&obj, event)
    }

    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::trace!(CAT, obj = pad, "Received query: {query:?}");

        let obj = self.obj();
        (obj.class().as_ref().sink_query)(&obj, query)
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::trace!(CAT, obj = pad, "Received query: {query:?}");

        let obj = self.obj();
        (obj.class().as_ref().src_query)(&obj, query)
    }

    fn retrieve_pending_segment_event(&self, state: &mut State) -> Option<gst::Event> {
        // If a segment event was pending, take it now and push it out later.
        if !state.pending_segment {
            return None;
        }

        state.negotiated_src_caps.as_ref()?;

        let (seqnum, segment) = state.segment.as_ref().unwrap();
        let segment_event = gst::event::Segment::builder(segment)
            .seqnum(*seqnum)
            .build();
        state.pending_segment = false;

        gst::debug!(
            CAT,
            imp = self,
            "Created segment event {segment:?} with seqnum {seqnum:?}"
        );

        Some(segment_event)
    }

    fn handle_buffer(
        &self,
        _settings: &Settings,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // Check if renegotiation is needed.
        if self.src_pad.check_reconfigure() {
            self.negotiate();
        }

        let mut state = self.state.borrow_mut();

        if state.sink_caps.is_none() {
            gst::error!(CAT, imp = self, "No sink pad caps");
            gst::element_imp_error!(
                self,
                gst::CoreError::Negotiation,
                [
                    "No input format was negotiated, i.e. no caps event was received. \
                     Perhaps you need a parser or typefind element before the payloader",
                ]
            );
            return Err(gst::FlowError::NotNegotiated);
        }

        if state.segment.is_none() {
            gst::error!(CAT, imp = self, "Received buffers without segment");
            return Err(gst::FlowError::Error);
        }

        if buffer.flags().contains(gst::BufferFlags::HEADER)
            && self.obj().class().as_ref().drop_header_buffers
        {
            gst::trace!(CAT, imp = self, "Dropping buffer with HEADER flag");
            return Ok(gst::FlowSuccess::Ok);
        }

        if buffer.pts().is_none() {
            if let Some(last_pts) = state.last_pts {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Buffer without PTS, reusing last PTS {last_pts}"
                );
                buffer.make_mut().set_pts(last_pts);
            } else {
                gst::error!(CAT, imp = self, "First buffer without PTS");
                return Err(gst::FlowError::Error);
            }
        };

        // TODO: Should we drain on DISCONT here, or leave it to the subclass?

        // Check for too many pending packets
        if let Some((front, back)) =
            Option::zip(state.pending_buffers.front(), state.pending_buffers.back())
        {
            let pts_diff = back.buffer.pts().opt_saturating_sub(front.buffer.pts());

            if let Some(pts_diff) = pts_diff {
                if pts_diff > gst::ClockTime::SECOND {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "More than {pts_diff} of PTS time queued, probably a bug in the subclass"
                    );
                }
            }
        }

        let id = state.current_buffer_id;
        state.current_buffer_id += 1;

        gst::trace!(CAT, imp = self, "Handling buffer {buffer:?} with id {id}");
        state.pending_buffers.push_back(PendingBuffer {
            id,
            buffer: buffer.clone(),
        });
        drop(state);

        let obj = self.obj();
        let mut res = (obj.class().as_ref().handle_buffer)(&obj, &buffer, id);
        if let Err(err) = res {
            gst::error!(CAT, imp = self, "Failed handling buffer: {err:?}");
        } else {
            res = self.finish_pending_packets();

            if let Err(err) = res {
                gst::debug!(CAT, imp = self, "Failed finishing pending packets: {err:?}");
            }
        }

        // Now drop all pending buffers that were used for producing packets
        // except for the very last one as that might still be used.
        let mut state = self.state.borrow_mut();
        while state
            .pending_buffers
            .front()
            .is_some_and(|b| b.id < state.last_used_buffer_id)
        {
            let _ = state.pending_buffers.pop_front();
        }

        res
    }
}

/// Other methods.
impl RtpBasePay2 {
    fn set_sink_caps(&self, caps: gst::Caps) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, imp = self, "Received caps {caps:?}");

        let mut state = self.state.borrow_mut();
        let same_caps = state
            .sink_caps
            .as_ref()
            .map(|old_caps| &caps == old_caps)
            .unwrap_or(false);
        state.sink_caps = Some(caps.clone());
        drop(state);

        if !same_caps {
            let obj = self.obj();
            let set_sink_caps_res = if (obj.class().as_ref().set_sink_caps)(&obj, &caps) {
                gst::debug!(CAT, imp = self, "Caps {caps:?} accepted");
                Ok(gst::FlowSuccess::Ok)
            } else {
                gst::warning!(CAT, imp = self, "Caps {caps:?} not accepted");
                Err(gst::FlowError::NotNegotiated)
            };

            let mut state = self.state.borrow_mut();
            if let Err(err) = set_sink_caps_res {
                state.sink_caps = None;
                // FIXME: Forget everything here?
                return Err(err);
            }
            drop(state);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let obj = self.obj();
        if let Err(err) = (obj.class().as_ref().drain)(&obj) {
            if ![gst::FlowError::Flushing, gst::FlowError::Eos].contains(&err) {
                gst::warning!(CAT, imp = self, "Draining failed: {err:?}");
            } else {
                gst::debug!(CAT, imp = self, "Draining failed: {err:?}");
            }
            self.flush();
            return Err(err);
        }

        let mut state = self.state.borrow_mut();
        state.pending_buffers.clear();

        if !state.pending_packets.is_empty() {
            gst::debug!(CAT, imp = self, "Pushing all pending packets");

            // Update PTS and RTP timestamp of all pending packets that have none yet
            let pts = state.last_pts;
            let rtptime = state.stream.as_ref().unwrap().last_timestamp.0;
            let mut discont_pending = state.discont_pending;
            state.discont_pending = false;

            for packet in state
                .pending_packets
                .iter_mut()
                .skip_while(|p| p.buffer.pts().is_some())
            {
                assert!(packet.buffer.pts().is_none());
                let buffer = packet.buffer.get_mut().unwrap();
                buffer.set_pts(pts);

                if discont_pending {
                    buffer.set_flags(gst::BufferFlags::DISCONT);
                    discont_pending = false;
                }

                let mut map = buffer.map_writable().unwrap();
                let mut packet = rtp_types::RtpPacketMut::parse(&mut map).unwrap();
                packet.set_timestamp(rtptime);
            }
        }

        drop(state);

        // Forward all packets
        let res = self.finish_pending_packets();

        self.flush();

        res
    }

    fn flush(&self) {
        let obj = self.obj();
        (obj.class().as_ref().flush)(&obj);

        let mut state = self.state.borrow_mut();
        state.pending_buffers.clear();

        // Drop any remaining pending packets, this can only really happen on errors
        state.pending_packets.clear();

        // Reset mapping and require the subclass to provide a new one next time
        state.last_pts_rtp_mapping = None;
        state.discont_pending = true;
    }

    fn create_stats(&self) -> gst::Structure {
        let stats = self.stats.lock().unwrap().clone();
        if let Some(stats) = stats {
            gst::Structure::builder("application/x-rtp-payload-stats")
                .field("ssrc", stats.ssrc)
                .field("clock-rate", stats.clock_rate.unwrap_or_default())
                .field("running-time", stats.running_time)
                .field("seqnum", stats.seqnum as u32)
                .field("timestamp", stats.timestamp)
                .field("pt", stats.pt as u32)
                .field("seqnum-offset", stats.seqnum_offset as u32)
                .field("timestamp-offset", stats.timestamp_offset)
                .build()
        } else {
            gst::Structure::builder("application/x-rtp-payload-stats").build()
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtpBasePay2 {
    const NAME: &'static str = "GstRtpBasePay2";
    const ABSTRACT: bool = true;
    type Type = super::RtpBasePay2;
    type ParentType = gst::Element;
    type Class = super::Class;

    fn class_init(class: &mut Self::Class) {
        class.start = |obj| obj.imp().start_default();
        class.stop = |obj| obj.imp().stop_default();
        class.set_sink_caps = |obj, caps| obj.imp().set_sink_caps_default(caps);
        class.negotiate = |obj, caps| obj.imp().negotiate_default(caps);
        class.handle_buffer = |obj, buffer, id| obj.imp().handle_buffer_default(buffer, id);
        class.drain = |obj| obj.imp().drain_default();
        class.flush = |obj| obj.imp().flush_default();
        class.sink_event = |obj, event| obj.imp().sink_event_default(event);
        class.src_event = |obj, event| obj.imp().src_event_default(event);
        class.sink_query = |obj, query| obj.imp().sink_query_default(query);
        class.src_query = |obj, query| obj.imp().src_query_default(query);
        class.transform_meta =
            |obj, in_buf, meta, out_buf| obj.imp().transform_meta_default(in_buf, meta, out_buf);

        class.allowed_meta_tags = &[];
        class.drop_header_buffers = false;
        class.default_pt = 96;
    }

    fn with_class(class: &Self::Class) -> Self {
        let templ = class
            .pad_template("sink")
            .expect("Subclass did not provide a \"sink\" pad template");
        let sink_pad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain(pad, buffer),
                )
            })
            .chain_list_function(|pad, parent, list| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain_list(pad, list),
                )
            })
            .event_full_function(|pad, parent, event| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                Self::catch_panic_pad_function(parent, || false, |imp| imp.sink_query(pad, query))
            })
            .build();

        let templ = class
            .pad_template("src")
            .expect("Subclass did not provide a \"src\" pad template");
        let src_pad = gst::Pad::builder_from_template(&templ)
            .event_full_function(|pad, parent, event| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                Self::catch_panic_pad_function(parent, || false, |imp| imp.src_query(pad, query))
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        let settings = Settings {
            pt: class.default_pt,
            ..Settings::default()
        };

        Self {
            src_pad,
            sink_pad,
            state: AtomicRefCell::default(),
            settings: Mutex::new(settings),
            stats: Mutex::default(),
            ssrc_collision: Mutex::new(None),
            extensions: Mutex::default(),
        }
    }
}

impl ObjectImpl for RtpBasePay2 {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("mtu")
                    .nick("MTU")
                    .blurb("Maximum size of one RTP packet")
                    .default_value(Settings::default().mtu)
                    .minimum(28)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("pt")
                    .nick("Payload Type")
                    .blurb("Payload type of the packets")
                    .default_value(u32::from(Settings::default().pt))
                    .minimum(0)
                    .maximum(0x7f)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt64::builder("ssrc")
                    .nick("SSRC")
                    .blurb("SSRC of the packets (-1 == random)")
                    .default_value(Settings::default().ssrc.map(i64::from).unwrap_or(-1))
                    .minimum(-1)
                    .maximum(u32::MAX as i64)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt64::builder("timestamp-offset")
                    .nick("Timestamp Offset")
                    .blurb("Offset that is added to all RTP timestamps (-1 == random)")
                    .default_value(
                        Settings::default()
                            .timestamp_offset
                            .map(i64::from)
                            .unwrap_or(-1),
                    )
                    .minimum(-1)
                    .maximum(u32::MAX as i64)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt::builder("seqnum-offset")
                    .nick("Sequence Number Offset")
                    .blurb("Offset that is added to all RTP sequence numbers (-1 == random)")
                    .default_value(
                        Settings::default()
                            .seqnum_offset
                            .map(i32::from)
                            .unwrap_or(-1),
                    )
                    .minimum(-1)
                    .maximum(u16::MAX as i32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("onvif-no-rate-control")
                    .nick("ONVIF No Rate Control")
                    .blurb("Enable ONVIF Rate-Control=no timestamping mode")
                    .default_value(Settings::default().onvif_no_rate_control)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("scale-rtptime")
                    .nick("Scale RTP Time")
                    .blurb("Whether the RTP timestamp should be scaled with the rate (speed)")
                    .default_value(Settings::default().scale_rtptime)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Statistics")
                    .blurb("Various statistics")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("seqnum")
                    .nick("Sequence Number")
                    .blurb("RTP sequence number of the last packet")
                    .minimum(0)
                    .maximum(u16::MAX as u32)
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("timestamp")
                    .nick("Timestamp")
                    .blurb("RTP timestamp of the last packet")
                    .minimum(0)
                    .maximum(u32::MAX)
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("source-info")
                    .nick("RTP Source Info")
                    .blurb("Add RTP source information as buffer metadata")
                    .default_value(Settings::default().source_info)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("auto-header-extension")
                    .nick("Automatic RTP Header Extensions")
                    .blurb("Whether RTP header extensions should be automatically enabled, if an implementation is available")
                    .default_value(Settings::default().auto_header_extensions)
                    .mutable_ready()
                    .build(),
                gst::ParamSpecArray::builder("extensions")
                    .nick("RTP Header Extensions")
                    .blurb("List of enabled RTP header extensions")
                    .element_spec(&glib::ParamSpecObject::builder::<gst_rtp::RTPHeaderExtension>("extension")
                        .nick("RTP Header Extension")
                        .blurb("Enabled RTP header extension")
                        .read_only()
                        .build()
                    )
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder("add-extension")
                    .action()
                    .param_types([gst_rtp::RTPHeaderExtension::static_type()])
                    .class_handler(|args| {
                        let s = args[0].get::<super::RtpBasePay2>().unwrap();
                        let ext = args[1].get::<&gst_rtp::RTPHeaderExtension>().unwrap();
                        s.imp().add_extension(ext);

                        None
                    })
                    .build(),
                glib::subclass::Signal::builder("request-extension")
                    .param_types([u32::static_type(), String::static_type()])
                    .return_type::<gst_rtp::RTPHeaderExtension>()
                    .accumulator(|_hint, _acc, value| {
                        if matches!(value.get::<Option<glib::Object>>(), Ok(Some(_))) {
                            std::ops::ControlFlow::Break(value.clone())
                        } else {
                            std::ops::ControlFlow::Continue(value.clone())
                        }
                    })
                    .class_handler(|args| {
                        let s = args[0].get::<super::RtpBasePay2>().unwrap();
                        let ext_id = args[1].get::<u32>().unwrap();
                        let uri = args[2].get::<&str>().unwrap();
                        let ext = s.imp().request_extension(ext_id, uri);

                        Some(ext.to_value())
                    })
                    .build(),
                glib::subclass::Signal::builder("clear-extensions")
                    .action()
                    .class_handler(|args| {
                        let s = args[0].get::<super::RtpBasePay2>().unwrap();
                        s.imp().clear_extensions();

                        None
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "mtu" => {
                self.settings.lock().unwrap().mtu = value.get().unwrap();
            }
            "pt" => {
                let mut settings = self.settings.lock().unwrap();
                settings.pt = value.get::<u32>().unwrap() as u8;
                settings.pt_set = true;
            }
            "ssrc" => {
                let v = value.get::<i64>().unwrap();
                self.settings.lock().unwrap().ssrc = (v != -1).then_some(v as u32);
            }
            "timestamp-offset" => {
                let v = value.get::<i64>().unwrap();
                self.settings.lock().unwrap().timestamp_offset = (v != -1).then_some(v as u32);
            }
            "seqnum-offset" => {
                let v = value.get::<i32>().unwrap();
                self.settings.lock().unwrap().seqnum_offset = (v != -1).then_some(v as u16);
            }
            "onvif-no-rate-control" => {
                self.settings.lock().unwrap().onvif_no_rate_control = value.get().unwrap();
            }
            "scale-rtptime" => {
                self.settings.lock().unwrap().scale_rtptime = value.get().unwrap();
            }
            "source-info" => {
                self.settings.lock().unwrap().source_info = value.get().unwrap();
            }
            "auto-header-extension" => {
                self.settings.lock().unwrap().auto_header_extensions = value.get().unwrap();
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "mtu" => self.settings.lock().unwrap().mtu.to_value(),
            "pt" => (u32::from(self.settings.lock().unwrap().pt)).to_value(),
            "ssrc" => self
                .settings
                .lock()
                .unwrap()
                .ssrc
                .map(i64::from)
                .unwrap_or(-1)
                .to_value(),
            "timestamp-offset" => self
                .settings
                .lock()
                .unwrap()
                .timestamp_offset
                .map(i64::from)
                .unwrap_or(-1)
                .to_value(),
            "seqnum-offset" => self
                .settings
                .lock()
                .unwrap()
                .seqnum_offset
                .map(i32::from)
                .unwrap_or(-1)
                .to_value(),
            "onvif-no-rate-control" => self
                .settings
                .lock()
                .unwrap()
                .onvif_no_rate_control
                .to_value(),
            "scale-rtptime" => self.settings.lock().unwrap().scale_rtptime.to_value(),
            "stats" => self.create_stats().to_value(),
            "seqnum" => (self
                .stats
                .lock()
                .unwrap()
                .as_ref()
                .map(|s| s.seqnum)
                .unwrap_or(0) as u32)
                .to_value(),
            "timestamp" => self
                .stats
                .lock()
                .unwrap()
                .as_ref()
                .map(|s| s.timestamp)
                .unwrap_or(0)
                .to_value(),
            "source-info" => self.settings.lock().unwrap().source_info.to_value(),
            "auto-header-extension" => self
                .settings
                .lock()
                .unwrap()
                .auto_header_extensions
                .to_value(),
            "extensions" => gst::Array::new(self.extensions.lock().unwrap().values()).to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sink_pad).unwrap();
        obj.add_pad(&self.src_pad).unwrap();
    }
}

impl GstObjectImpl for RtpBasePay2 {}

impl ElementImpl for RtpBasePay2 {
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::debug!(CAT, imp = self, "Changing state: {transition}");

        if transition == gst::StateChange::ReadyToPaused {
            *self.state.borrow_mut() = State::default();
            *self.stats.lock().unwrap() = None;

            let obj = self.obj();
            (obj.class().as_ref().start)(&obj).map_err(|err_msg| {
                self.post_error_message(err_msg);
                gst::StateChangeError
            })?;
        }

        let ret = self.parent_change_state(transition)?;

        if transition == gst::StateChange::PausedToReady {
            let obj = self.obj();
            (obj.class().as_ref().stop)(&obj).map_err(|err_msg| {
                self.post_error_message(err_msg);
                gst::StateChangeError
            })?;

            *self.state.borrow_mut() = State::default();
            *self.stats.lock().unwrap() = None;
        }

        Ok(ret)
    }
}
