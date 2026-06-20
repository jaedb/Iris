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

use std::sync::LazyLock;

use std::{
    collections::{BTreeMap, VecDeque},
    ops::{Bound, RangeBounds},
    sync::Mutex,
};

use super::{PacketToBufferRelation, TimestampOffset};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpbasedepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP Base Depayloader 2"),
    )
});

#[derive(Clone, Debug)]
struct Settings {
    max_reorder: u32,
    source_info: bool,
    auto_header_extensions: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_reorder: 100,
            source_info: false,
            auto_header_extensions: true,
        }
    }
}

/// Metadata of a pending packet for tracking purposes.
struct PendingPacket {
    /// Extended sequence number of the packet.
    ext_seqnum: u64,
    /// Extended RTP timestamp of the packet.
    ext_timestamp: u64,
    /// SSRC of the packet.
    ssrc: u32,
    /// CSRC of the packet.
    csrc: [u32; rtp_types::RtpPacket::MAX_N_CSRCS],
    n_csrc: u8,
    /// Packet contains header extensions.
    have_header_extensions: bool,
    /// Buffer containing the packet.
    buffer: gst::Buffer,
}

/// A pending buffer that still has to be sent downstream.
struct PendingBuffer {
    /// If no metadata is set then this buffer must be pushed together with
    /// the next buffer where it is actually known, or at EOS with the
    /// last known one.
    metadata_set: bool,
    buffer: gst::Buffer,
}

/// Information about the current stream that is handled.
struct CurrentStream {
    /// SSRC of the last received packet.
    ssrc: u32,
    /// Payload type of the last received packet.
    pt: u8,
    /// Extended sequence number of the last received packet.
    ext_seqnum: u64,
    /// Extended RTP time of the last received packet.
    ext_rtptime: u64,
}

struct State {
    segment: Option<(gst::Seqnum, gst::FormattedSegment<gst::ClockTime>)>,
    /// Set when a new segment event should be sent downstream before the next buffer.
    pending_segment: bool,

    sink_caps: Option<gst::Caps>,
    src_caps: Option<gst::Caps>,

    /// Set when the sinkpad caps are known.
    clock_rate: Option<u32>,

    // NPT start from old-style RTSP caps.
    npt_start: Option<gst::ClockTime>,
    // NPT stop from old-style RTSP caps.
    npt_stop: Option<gst::ClockTime>,
    // Play scale from old-style RTSP caps.
    play_speed: f64,
    // Play speed from old-style RTSP caps.
    play_scale: f64,
    // Clock base from old-style RTSP caps.
    clock_base: Option<u32>,
    // Initial PTS / ext_rtptime after a caps event where NPT start was set.
    npt_start_times: Option<(Option<gst::ClockTime>, u64)>,

    /// Set when the next outgoing buffer should have the discont flag set.
    discont_pending: bool,

    /// Current stream configuration. Set on first packet and whenever it changes.
    current_stream: Option<CurrentStream>,

    /// Last extended seqnum that was used for a buffer.
    last_used_ext_seqnum: Option<u64>,

    /// PTS of the last outgoing buffer.
    last_pts: Option<gst::ClockTime>,
    /// DTS of the last outgoing buffer.
    last_dts: Option<gst::ClockTime>,

    /// Pending input packets that were not completely used up for outgoing buffers yet.
    pending_packets: VecDeque<PendingPacket>,

    /// Pending buffers that have to be pushed downstream. Some of them might not have a PTS yet,
    /// i.e. were created without corresponding seqnums.
    pending_buffers: VecDeque<PendingBuffer>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            segment: None,
            pending_segment: false,

            sink_caps: None,
            src_caps: None,

            clock_rate: None,

            npt_start: None,
            npt_stop: None,
            play_scale: 1.0,
            play_speed: 1.0,
            clock_base: None,
            npt_start_times: None,

            discont_pending: true,

            current_stream: None,

            last_used_ext_seqnum: None,

            last_pts: None,
            last_dts: None,

            pending_packets: VecDeque::new(),
            pending_buffers: VecDeque::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct Stats {
    ssrc: u32,
    clock_rate: u32,
    running_time_dts: Option<gst::ClockTime>,
    running_time_pts: Option<gst::ClockTime>,
    seqnum: u16,
    timestamp: u32,
    npt_start: Option<gst::ClockTime>,
    npt_stop: Option<gst::ClockTime>,
    play_speed: f64,
    play_scale: f64,
}

pub struct RtpBaseDepay2 {
    sink_pad: gst::Pad,
    src_pad: gst::Pad,
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
    stats: Mutex<Option<Stats>>,
    extensions: Mutex<BTreeMap<u8, gst_rtp::RTPHeaderExtension>>,
}

/// Wrapper around `gst::Caps` that implements `Ord` around the structure name.
struct CapsOrd(gst::Caps);

impl CapsOrd {
    fn new(caps: gst::Caps) -> Self {
        assert!(caps.is_fixed());
        CapsOrd(caps)
    }
}

impl PartialEq for CapsOrd {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for CapsOrd {}

impl PartialOrd for CapsOrd {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapsOrd {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .structure(0)
            .unwrap()
            .name()
            .cmp(other.0.structure(0).unwrap().name())
    }
}

/// Wrappers for public methods and associated helper functions.
#[allow(dead_code)]
impl RtpBaseDepay2 {
    pub(super) fn set_src_caps(&self, src_caps: &gst::Caps) {
        gst::debug!(CAT, imp = self, "Setting caps {src_caps:?}");

        let mut state = self.state.borrow_mut();
        if Some(src_caps) == state.src_caps.as_ref() {
            gst::debug!(CAT, imp = self, "Setting same caps {src_caps:?} again");
            return;
        }

        let seqnum = if let Some((seqnum, _)) = state.segment {
            seqnum
        } else {
            gst::Seqnum::next()
        };
        state.src_caps = Some(src_caps.clone());
        let segment_event = self.retrieve_pending_segment_event(&mut state);
        drop(state);

        // Rely on the actual flow return later when pushing a buffer downstream. The bool
        // return from pushing the event does not allow to distinguish between different kinds of
        // errors.
        let _ = self
            .src_pad
            .push_event(gst::event::Caps::builder(src_caps).seqnum(seqnum).build());

        if let Some(segment_event) = segment_event {
            let _ = self.src_pad.push_event(segment_event);
        }
    }

    pub(super) fn drop_packets(&self, ext_seqnum: impl RangeBounds<u64>) {
        gst::trace!(
            CAT,
            imp = self,
            "Dropping packets up to ext seqnum {:?}",
            ext_seqnum.end_bound()
        );

        let mut state = self.state.borrow_mut();
        state.discont_pending = true;

        let end = match ext_seqnum.end_bound() {
            Bound::Included(end) => *end,
            Bound::Excluded(end) if *end == 0 => return,
            Bound::Excluded(end) => *end - 1,
            Bound::Unbounded => {
                state.pending_buffers.clear();
                return;
            }
        };

        if let Some(back) = state.pending_packets.back() {
            if back.ext_seqnum <= end {
                state.pending_packets.clear();
                return;
            }
        } else {
            return;
        }

        while state
            .pending_packets
            .front()
            .is_some_and(|p| p.ext_seqnum <= end)
        {
            let _ = state.pending_packets.pop_front();
        }
    }

    fn retrieve_pts_dts(
        &self,
        state: &State,
        ext_seqnum: u64,
    ) -> (Option<gst::ClockTime>, Option<gst::ClockTime>) {
        let mut pts = None;
        let mut dts = None;

        for front in state
            .pending_packets
            .iter()
            .take_while(|p| p.ext_seqnum <= ext_seqnum)
        {
            // Remember first PTS/DTS
            if pts.is_none() || dts.is_none() {
                pts = front.buffer.pts();
                dts = front.buffer.dts();
            }

            if pts.is_some() || dts.is_some() {
                break;
            }
        }

        (pts, dts)
    }

    fn copy_metas(
        &self,
        settings: &Settings,
        state: &mut State,
        ext_seqnum: u64,
        buffer: &mut gst::BufferRef,
    ) -> bool {
        let mut extension_wants_caps_update = false;
        let extensions = self.extensions.lock().unwrap();

        let mut ssrc = None;
        let mut csrc = [0u32; rtp_types::RtpPacket::MAX_N_CSRCS];
        let mut n_csrc = 0;

        // Copy over metas and other metadata from the packets that made up this buffer
        let obj = self.obj();
        let mut reference_timestamp_metas = BTreeMap::new();
        for front in state
            .pending_packets
            .iter()
            .take_while(|p| p.ext_seqnum <= ext_seqnum)
        {
            // Filter out reference timestamp metas that have the same timestamps

            front.buffer.foreach_meta(|meta| {
                use std::ops::ControlFlow::*;

                // Do not copy metas that are memory specific
                if meta.has_tag::<gst::meta::tags::Memory>()
                    || meta.has_tag::<gst::meta::tags::MemoryReference>()
                {
                    return Continue(());
                }

                // Actual filtering of reference timestamp metas with same timestamp
                if let Some(meta) = meta.downcast_ref::<gst::ReferenceTimestampMeta>() {
                    let mut same = false;
                    reference_timestamp_metas
                        .entry(CapsOrd::new(meta.reference_owned()))
                        .and_modify(|timestamp| {
                            same = *timestamp == meta.timestamp();
                            *timestamp = meta.timestamp();
                        })
                        .or_insert(meta.timestamp());

                    if same {
                        return Continue(());
                    }
                }

                (obj.class().as_ref().transform_meta)(&obj, &front.buffer, &meta, buffer);

                Continue(())
            });

            if !extensions.is_empty() && front.have_header_extensions {
                let map = front.buffer.map_readable().unwrap();
                let packet = rtp_types::RtpPacket::parse(&map).unwrap();
                let (extension_pattern, mut extensions_data) = packet.extension().unwrap();

                let extension_flags = match extension_pattern {
                    0xBEDE => gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE,
                    x if x >> 4 == 0x100 => gst_rtp::RTPHeaderExtensionFlags::TWO_BYTE,
                    _ => {
                        gst::trace!(
                            CAT,
                            imp = self,
                            "Unknown extension pattern {extension_pattern:04X}"
                        );
                        continue;
                    }
                };

                while !extensions_data.is_empty() {
                    let (id, len) = if extension_flags == gst_rtp::RTPHeaderExtensionFlags::ONE_BYTE
                    {
                        let b = extensions_data[0];
                        extensions_data = &extensions_data[1..];

                        let id = b >> 4;
                        let len = (b & 0x0f) + 1;

                        // Padding
                        if id == 0 {
                            continue;
                        }
                        if id == 15 {
                            // Special ID
                            break;
                        }

                        (id, len as usize)
                    } else {
                        let id = extensions_data[0];
                        extensions_data = &extensions_data[1..];

                        // Padding
                        if id == 0 {
                            continue;
                        }

                        if extensions_data.is_empty() {
                            break;
                        }

                        let len = extensions_data[1];
                        extensions_data = &extensions_data[1..];

                        (id, len as usize)
                    };

                    if extensions_data.len() < len {
                        break;
                    }

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Handling RTP header extension with id {id} and length {len}"
                    );

                    let (extension_data, remainder) = extensions_data.split_at(len);
                    extensions_data = remainder;

                    let Some(extension) = extensions.get(&id) else {
                        continue;
                    };

                    if !extension.read(extension_flags, extension_data, buffer) {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Failed reading RTP header extension with id {id} and length {len}"
                        );
                        continue;
                    }

                    extension_wants_caps_update |= extension.wants_update_non_rtp_src_caps();
                }
            }

            if settings.source_info {
                if ssrc.is_none() {
                    ssrc = Some(front.ssrc);
                }

                for c in &front.csrc[..front.n_csrc as usize] {
                    if !csrc[..n_csrc].contains(c) && n_csrc < rtp_types::RtpPacket::MAX_N_CSRCS {
                        csrc[n_csrc] = *c;
                        n_csrc += 1;
                    }
                }
            }
        }

        if settings.source_info && ssrc.is_some() {
            gst_rtp::RTPSourceMeta::add(buffer, ssrc, &csrc[..n_csrc]);
        }

        extension_wants_caps_update
    }

    pub(super) fn queue_buffer(
        &self,
        packet_to_buffer_relation: PacketToBufferRelation,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();

        {
            // Reset some buffer metadata, it shouldn't have been set
            let buffer_ref = buffer.make_mut();
            buffer_ref.set_pts(None);
            buffer_ref.set_dts(None);
        }

        gst::trace!(
            CAT,
            imp = self,
            "Queueing buffer {buffer:?} for seqnum range {packet_to_buffer_relation:?}"
        );

        let mut state = self.state.borrow_mut();
        if state.src_caps.is_none() {
            gst::error!(CAT, imp = self, "No source pad caps negotiated yet");
            return Err(gst::FlowError::NotNegotiated);
        }

        if matches!(packet_to_buffer_relation, PacketToBufferRelation::OutOfBand) {
            gst::trace!(
                CAT,
                imp = self,
                "Keeping buffer without associated seqnums until next buffer or EOS"
            );

            state.pending_buffers.push_back(PendingBuffer {
                metadata_set: false,
                buffer,
            });
            return Ok(gst::FlowSuccess::Ok);
        };

        let (seqnums, timestamp_offset) = match packet_to_buffer_relation {
            PacketToBufferRelation::Seqnums(seqnums) => (seqnums.clone(), None),
            PacketToBufferRelation::SeqnumsWithOffset {
                seqnums: ids,
                timestamp_offset,
            } => (ids.clone(), Some(timestamp_offset)),
            PacketToBufferRelation::OutOfBand => unreachable!(),
        };

        if seqnums.is_empty() {
            gst::error!(CAT, imp = self, "Empty packet ext seqnum range provided");
            return Err(gst::FlowError::Error);
        }

        let seqnum_start = *seqnums.start();
        let seqnum_end = *seqnums.end();

        // Drop all older seqnums
        while state
            .pending_packets
            .front()
            .is_some_and(|p| p.ext_seqnum < seqnum_start)
        {
            let p = state.pending_packets.pop_front().unwrap();
            gst::trace!(
                CAT,
                imp = self,
                "Dropping packet with extended seqnum {}",
                p.ext_seqnum
            );
        }

        if state.pending_packets.is_empty() {
            gst::error!(
                CAT,
                imp = self,
                "Queueing buffers for future ext seqnums not allowed"
            );
            return Err(gst::FlowError::Error);
        };

        if seqnum_end > state.pending_packets.back().unwrap().ext_seqnum {
            gst::error!(
                CAT,
                imp = self,
                "Queueing buffers for future ext seqnums not allowed"
            );
            return Err(gst::FlowError::Error);
        }

        let mut discont_pending = state.discont_pending;
        state.discont_pending = false;

        let mut pts = state
            .pending_packets
            .iter()
            .take_while(|p| p.ext_seqnum <= seqnum_end)
            .find_map(|p| p.buffer.pts());
        let mut dts = None;

        if let Some(timestamp_offset) = timestamp_offset {
            match timestamp_offset {
                TimestampOffset::Pts(pts_offset) => {
                    if let Some(ts) = pts {
                        let pts_signed = ts.into_positive() + pts_offset;
                        if let Some(ts) = pts_signed.positive() {
                            pts = Some(ts);
                        } else {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Negative PTS {} calculated, not supported",
                                pts_signed
                            );
                        }
                    }
                }
                TimestampOffset::PtsAndDts(pts_offset, dts_offset) => {
                    if let Some(ts) = pts {
                        let pts_signed = ts.into_positive() + pts_offset;
                        if let Some(ts) = pts_signed.positive() {
                            pts = Some(ts);
                        } else {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Negative PTS {} calculated, not supported",
                                pts_signed
                            );
                        }
                    }

                    if let Some(pts) = pts {
                        let dts_signed = pts.into_positive() + dts_offset;
                        if let Some(ts) = dts_signed.positive() {
                            dts = Some(ts);
                        } else {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Negative DTS {} calculated, not supported",
                                dts_signed
                            );
                        }
                    }
                }
            }
        } else if state
            .last_used_ext_seqnum
            .is_some_and(|last_used_ext_seqnum| seqnum_end <= last_used_ext_seqnum)
        {
            // If we have no timestamp offset and this is not the first time a buffer for this
            // packet is queued then unset the PTS. It's not going to be correct.
            pts = None;
        }

        // Decorate all previously queued buffers metadata
        for buffer in state
            .pending_buffers
            .iter_mut()
            .skip_while(|b| b.metadata_set)
        {
            assert!(!buffer.metadata_set);
            buffer.metadata_set = true;
            let buffer = &mut buffer.buffer;
            let buffer_ref = buffer.make_mut();
            buffer_ref.set_pts(pts);
            buffer_ref.set_dts(dts);
            if discont_pending {
                buffer_ref.set_flags(gst::BufferFlags::DISCONT);
                discont_pending = false;
            }
        }

        // Decorate the new buffer
        {
            let buffer_ref = buffer.make_mut();
            buffer_ref.set_pts(pts);
            buffer_ref.set_dts(dts);
            let needs_caps_update = self.copy_metas(&settings, &mut state, seqnum_end, buffer_ref);

            if discont_pending {
                buffer_ref.set_flags(gst::BufferFlags::DISCONT);
            }

            // If a caps update is needed then let's do that *before* outputting this buffer.
            if needs_caps_update {
                let old_src_caps = state.src_caps.as_ref().unwrap();
                let mut src_caps = old_src_caps.copy();

                {
                    let src_caps = src_caps.get_mut().unwrap();
                    let extensions = self.extensions.lock().unwrap();
                    for extension in extensions.values() {
                        if !extension.update_non_rtp_src_caps(src_caps) {
                            gst::error!(
                                CAT,
                                imp = self,
                                "RTP header extension {} could not update caps",
                                extension.name()
                            );
                            return Err(gst::FlowError::Error);
                        }
                    }
                }

                if &src_caps != old_src_caps {
                    let seqnum = state.segment.as_ref().unwrap().0;
                    drop(state);
                    self.finish_pending_buffers()?;

                    let _ = self
                        .src_pad
                        .push_event(gst::event::Caps::builder(&src_caps).seqnum(seqnum).build());

                    state = self.state.borrow_mut();
                    state.src_caps = Some(src_caps);
                }
            }
        }

        state.pending_buffers.push_back(PendingBuffer {
            metadata_set: true,
            buffer,
        });

        state.last_used_ext_seqnum = Some(seqnum_end);
        if pts.is_some() {
            state.last_pts = pts;
            state.last_dts = dts;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    pub(super) fn finish_pending_buffers(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if state.segment.is_none() {
            if !state.pending_buffers.is_empty() {
                // The queued buffers must be based on the caps only. They can be forwarded at a later
                // time, if there is one.
                gst::debug!(CAT, imp = self, "Can't finish buffers yet without segment");
            }
            return Ok(gst::FlowSuccess::Ok);
        }

        // As long as there are buffers that can be finished, take all with the same PTS (or no
        // PTS) and put them into a buffer list.
        while state.pending_buffers.iter().any(|b| b.metadata_set) {
            let pts = state.pending_buffers.front().unwrap().buffer.pts();

            // First count number of pending packets so we can allocate a buffer list with the correct
            // size instead of re-allocating for every added buffer, or simply push a single buffer
            // downstream instead of allocating a buffer list just for one buffer.
            let num_buffers = state
                .pending_buffers
                .iter()
                .take_while(|b| {
                    b.metadata_set && (b.buffer.pts() == pts || b.buffer.pts().is_none())
                })
                .count();

            assert_ne!(num_buffers, 0);

            gst::trace!(CAT, imp = self, "Flushing {num_buffers} buffers");

            let segment_event = self.retrieve_pending_segment_event(&mut state);

            enum BufferOrList {
                Buffer(gst::Buffer),
                List(gst::BufferList),
            }

            let buffers = if num_buffers == 1 {
                let buffer = state.pending_buffers.pop_front().unwrap().buffer;
                gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}");
                BufferOrList::Buffer(buffer)
            } else {
                let mut list = gst::BufferList::new_sized(num_buffers);
                {
                    let list = list.get_mut().unwrap();
                    while state.pending_buffers.front().is_some_and(|b| {
                        b.metadata_set && (b.buffer.pts() == pts || b.buffer.pts().is_none())
                    }) {
                        let buffer = state.pending_buffers.pop_front().unwrap().buffer;
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

            let res = match buffers {
                BufferOrList::Buffer(buffer) => self.src_pad.push(buffer),
                BufferOrList::List(list) => self.src_pad.push_list(list),
            };

            if let Err(err) = res {
                if ![gst::FlowError::Flushing, gst::FlowError::Eos].contains(&err) {
                    gst::warning!(CAT, imp = self, "Failed pushing buffers: {err:?}");
                } else {
                    gst::debug!(CAT, imp = self, "Failed pushing buffers: {err:?}");
                }
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
impl RtpBaseDepay2 {
    fn start_default(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn stop_default(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn set_sink_caps_default(&self, _caps: &gst::Caps) -> bool {
        true
    }

    fn handle_packet_default(
        &self,
        _packet: &super::Packet,
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
                            // Just continue here. The depayloader is now flushed and can proceed below,
                            // and if there was a serious error downstream it will happen on the next
                            // buffer pushed downstream
                            gst::debug!(CAT, imp = self, "Draining failed: {err:?}");
                        }
                    }

                    let mut state = self.state.borrow_mut();
                    if let Some(segment) = segment.downcast_ref::<gst::ClockTime>() {
                        state.segment = Some((event.seqnum(), segment.clone()));
                        state.pending_segment = true;
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

                    return Ok(gst::FlowSuccess::Ok);
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
                state.current_stream = None;
                state.last_pts = None;
                state.last_dts = None;
                drop(state);

                *self.stats.lock().unwrap() = None;
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

    fn src_event_default(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
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
        // FIXME: Needs caps update!
        drop(extensions);

        self.obj().notify("extensions");
    }

    fn clear_extensions(&self) {
        let mut extensions = self.extensions.lock().unwrap();
        extensions.clear();
        // FIXME: Needs caps update!
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
impl RtpBaseDepay2 {
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

        state.src_caps.as_ref()?;

        let (seqnum, segment) = state.segment.as_ref().unwrap();
        let mut segment = segment.clone();

        // If npt-start is set, all the other values are also valid.
        if let Some(npt_start) = state.npt_start {
            let (mut npt_start_pts, npt_start_ext_rtptime) = state.npt_start_times?;
            let clock_rate = state.clock_rate.unwrap();

            let mut start = segment.start().unwrap();

            if let Some((clock_base, pts)) = Option::zip(state.clock_base, npt_start_pts) {
                let ext_clock_base = clock_base as u64 + (1 << 32);

                let gap = npt_start_ext_rtptime
                    .saturating_sub(ext_clock_base)
                    .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                    .map(gst::ClockTime::from_nseconds);

                // Account for lost packets
                if let Some(gap) = gap {
                    if pts > gap {
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Found gap of {}, adjusting start: {} = {} - {}",
                            gap.display(),
                            (pts - gap).display(),
                            pts.display(),
                            gap.display(),
                        );
                        start = pts - gap;
                    }
                }
            }

            let mut stop = segment.stop();
            if let Some(npt_stop) = state.npt_stop {
                stop = Some(start + npt_stop.saturating_sub(npt_start));
            }

            if npt_start_pts.is_none() {
                npt_start_pts = Some(start);
            }

            let running_time = segment.to_running_time(start);

            segment.reset();
            segment.set_rate(state.play_speed);
            segment.set_applied_rate(state.play_scale);
            segment.set_start(start);
            segment.set_stop(stop);
            segment.set_time(npt_start);
            segment.set_position(npt_start_pts);
            segment.set_base(running_time);
        }

        let segment_event = gst::event::Segment::builder(&segment)
            .seqnum(*seqnum)
            .build();
        state.pending_segment = false;
        gst::debug!(
            CAT,
            imp = self,
            "Created segment event {segment:?} with seqnum {seqnum:?}",
        );

        Some(segment_event)
    }

    fn handle_buffer(
        &self,
        settings: &Settings,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if state.sink_caps.is_none() {
            gst::error!(CAT, imp = self, "No sink pad caps");
            gst::element_imp_error!(
                self,
                gst::CoreError::Negotiation,
                ("No RTP caps were negotiated"),
                [
                    "Input buffers need to have RTP caps set on them. This is usually \
                     achieved by setting the 'caps' property of the upstream source \
                     element (often udpsrc or appsrc), or by putting a capsfilter \
                     element before the depayloader and setting the 'caps' property \
                     on that. Also see https://gitlab.freedesktop.org/gstreamer/gstreamer/-/ \
                     blob/main/subprojects/gst-plugins-good/gst/rtp/README",
                ]
            );
            return Err(gst::FlowError::NotNegotiated);
        }

        if state.segment.is_none() {
            gst::error!(CAT, imp = self, "Received buffers without segment");
            return Err(gst::FlowError::Error);
        }

        // Always set if the caps are set.
        let clock_rate = state.clock_rate.unwrap();

        // Check for too many pending packets
        if let Some((front, back)) =
            Option::zip(state.pending_packets.front(), state.pending_packets.back())
        {
            let rtp_diff = back
                .ext_timestamp
                .saturating_sub(front.ext_timestamp)
                .mul_div_floor(*gst::ClockTime::SECOND, clock_rate as u64)
                .map(gst::ClockTime::from_nseconds);
            let pts_diff = back.buffer.pts().opt_saturating_sub(front.buffer.pts());

            if let Some(rtp_diff) = rtp_diff {
                if rtp_diff > gst::ClockTime::SECOND {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "More than {rtp_diff} of RTP time queued, probably a bug in the subclass"
                    );
                }
            }

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

        let buffer = match buffer.into_mapped_buffer_readable() {
            Ok(buffer) => buffer,
            Err(_) => {
                gst::error!(CAT, imp = self, "Failed to map buffer");
                gst::element_imp_error!(self, gst::StreamError::Failed, ["Failed to map buffer"]);
                return Err(gst::FlowError::Error);
            }
        };

        // Parse RTP packet and extract the relevant fields we will need later.
        let packet = match rtp_types::RtpPacket::parse(&buffer) {
            Ok(packet) => packet,
            Err(err) => {
                gst::warning!(CAT, imp = self, "Failed to parse RTP packet: {err:?}");
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let seqnum = packet.sequence_number();
        let ssrc = packet.ssrc();
        let pt = packet.payload_type();
        let rtptime = packet.timestamp();
        let marker = packet.marker_bit();
        let have_header_extensions = packet.extension_len() != 0;
        let csrc = {
            let mut csrc = packet.csrc();
            std::array::from_fn::<u32, { rtp_types::RtpPacket::MAX_N_CSRCS }, _>(|_idx| {
                csrc.next().unwrap_or_default()
            })
        };
        let n_csrc = packet.n_csrcs();

        let payload_range = {
            let payload = packet.payload();
            if payload.is_empty() {
                0..0
            } else {
                let offset = packet.payload_offset();
                offset..(offset + payload.len())
            }
        };

        // FIXME: Allow subclasses to decide not to flush on DISCONTs and handle the situation
        // themselves, e.g. to be able to continue reconstructing frames despite missing data.
        if buffer.buffer().flags().contains(gst::BufferFlags::DISCONT) {
            gst::info!(CAT, imp = self, "Discont received");
            state.current_stream = None;
        }

        // Check if there was a stream discontinuity, e.g. based on the SSRC changing or the seqnum
        // having a gap compared to the previous packet.
        if let Some(ref mut current_stream) = state.current_stream {
            if current_stream.ssrc != ssrc {
                gst::info!(
                    CAT,
                    imp = self,
                    "Stream SSRC changed from {:08x} to {:08x}",
                    current_stream.ssrc,
                    ssrc
                );
                state.current_stream = None;
            } else if current_stream.pt != pt {
                gst::info!(
                    CAT,
                    imp = self,
                    "Stream payload type changed from {} to {}",
                    current_stream.pt,
                    pt
                );
                state.current_stream = None;
            } else {
                let expected_seqnum = (current_stream.ext_seqnum + 1) & 0xffff;
                if expected_seqnum != seqnum as u64 {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Got seqnum {seqnum} but expected {expected_seqnum}"
                    );

                    let mut ext_seqnum = seqnum as u64 + (current_stream.ext_seqnum & !0xffff);

                    if ext_seqnum < current_stream.ext_seqnum {
                        let diff = current_stream.ext_seqnum - ext_seqnum;
                        if diff > 0x7fff {
                            ext_seqnum += 1 << 16;
                        }
                    } else {
                        let diff = ext_seqnum - current_stream.ext_seqnum;
                        if diff > 0x7fff {
                            ext_seqnum -= 1 << 16;
                        }
                    }
                    let diff = if ext_seqnum > current_stream.ext_seqnum {
                        (ext_seqnum - current_stream.ext_seqnum) as i32
                    } else {
                        -((current_stream.ext_seqnum - ext_seqnum) as i32)
                    };

                    if diff > 0 {
                        gst::info!(CAT, imp = self, "{diff} missing packets or sender restart");
                        state.current_stream = None;
                    } else if diff >= -(settings.max_reorder as i32) {
                        gst::info!(CAT, imp = self, "Got old packet, dropping");
                        return Ok(gst::FlowSuccess::Ok);
                    } else {
                        gst::info!(CAT, imp = self, "Sender restart");
                        state.current_stream = None;
                    }
                } else {
                    // Calculate extended RTP time
                    let ext_rtptime = {
                        let mut ext_rtptime =
                            rtptime as u64 + (current_stream.ext_rtptime & !0xffff_ffff);

                        // Check for wraparound
                        if ext_rtptime < current_stream.ext_rtptime {
                            let diff = current_stream.ext_rtptime - ext_rtptime;
                            if diff > 0x7fff_ffff {
                                ext_rtptime += 1 << 32;
                            }
                        } else {
                            let diff = ext_rtptime - current_stream.ext_rtptime;
                            if diff > 0x7fff_ffff {
                                ext_rtptime -= 1 << 32;
                            }
                        }

                        ext_rtptime
                    };

                    current_stream.ext_seqnum += 1;
                    current_stream.ext_rtptime = ext_rtptime;
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Handling packet with extended seqnum {} and extended RTP time {}",
                        current_stream.ext_seqnum,
                        current_stream.ext_rtptime,
                    );
                }
            }
        }

        // Drain if there was any discontinuity.
        let discont = state.current_stream.is_none();
        if discont && !state.pending_packets.is_empty() {
            drop(state);
            gst::info!(CAT, imp = self, "Got discontinuity, draining");

            if let Err(err) = self.drain() {
                if ![gst::FlowError::Flushing, gst::FlowError::Eos].contains(&err) {
                    gst::warning!(CAT, imp = self, "Error while draining: {err:?}");
                } else {
                    gst::debug!(CAT, imp = self, "Error while draining: {err:?}");
                }
                return Err(err);
            }
            state = self.state.borrow_mut();
        }

        // Update current stream state at this point.
        let ext_seqnum;
        let ext_rtptime;
        if let Some(ref current_stream) = state.current_stream {
            ext_seqnum = current_stream.ext_seqnum;
            ext_rtptime = current_stream.ext_rtptime;
        } else {
            ext_seqnum = seqnum as u64 + (1 << 16);
            ext_rtptime = rtptime as u64 + (1 << 32);

            gst::info!(
                CAT,
                imp = self,
                "Starting stream with SSRC {ssrc:08x} at extended seqnum {ext_seqnum} with extended RTP time {ext_rtptime}",
            );

            state.current_stream = Some(CurrentStream {
                ssrc,
                pt,
                ext_seqnum,
                ext_rtptime,
            });
            state.last_used_ext_seqnum = None;
        }

        // Remember the initial PTS/rtp_time mapping if old-style RTSP caps are used.
        if state.npt_start.is_some() && state.npt_start_times.is_none() {
            state.npt_start_times = Some((buffer.buffer().pts(), ext_rtptime));
        }

        // If a segment event was pending, take it now and push it out later.
        let segment_event = self.retrieve_pending_segment_event(&mut state);

        // Remember current packet.
        state.pending_packets.push_back(PendingPacket {
            ext_seqnum,
            ext_timestamp: ext_rtptime,
            ssrc,
            csrc,
            n_csrc,
            have_header_extensions,
            buffer: buffer.buffer_owned(),
        });

        // Remember if the next buffer that has to be pushed out should have the DISCONT flag set.
        state.discont_pending |= discont;

        // Update stats with the current packet.
        *self.stats.lock().unwrap() = {
            let (_seqnum, segment) = state.segment.as_ref().unwrap();

            let running_time_pts = segment.to_running_time(buffer.buffer().pts());
            let running_time_dts = segment.to_running_time(buffer.buffer().dts());
            Some(Stats {
                ssrc,
                clock_rate,
                running_time_dts,
                running_time_pts,
                seqnum,
                timestamp: rtptime,
                npt_start: state.npt_start,
                npt_stop: state.npt_stop,
                play_speed: state.play_speed,
                play_scale: state.play_scale,
            })
        };
        drop(state);

        if let Some(segment_event) = segment_event {
            let _ = self.src_pad.push_event(segment_event);
        }

        // Now finally handle the packet.
        let obj = self.obj();
        let mut res = (obj.class().as_ref().handle_packet)(
            &obj,
            &super::Packet {
                buffer,
                discont,
                ext_seqnum,
                ext_timestamp: ext_rtptime,
                marker,
                payload_range,
            },
        );
        if let Err(err) = res {
            gst::error!(CAT, imp = self, "Failed handling packet: {err:?}");
        } else {
            res = self.finish_pending_buffers();

            if let Err(err) = res {
                gst::debug!(CAT, imp = self, "Failed finishing pending buffers: {err:?}");
            }
        }

        // Now drop all pending packets that were used for producing buffers
        // except for the very last one as that might still be used.
        let mut state = self.state.borrow_mut();
        if let Some(last_used_ext_seqnum) = state.last_used_ext_seqnum {
            while state
                .pending_packets
                .front()
                .is_some_and(|b| b.ext_seqnum < last_used_ext_seqnum)
            {
                let _ = state.pending_packets.pop_front();
            }
        }

        res
    }
}

/// Other methods.
impl RtpBaseDepay2 {
    fn update_extensions_from_sink_caps(
        &self,
        sink_caps: &gst::Caps,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let s = sink_caps.structure(0).unwrap();

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
                    "Can't parse RTP header extension id from caps {sink_caps:?}"
                );
                return Err(gst::FlowError::NotNegotiated);
            };

            let uri = if let Ok(uri) = v.get::<String>() {
                uri
            } else if let Ok(arr) = v.get::<gst::ArrayRef>() {
                if let Some(uri) = arr.get(1).and_then(|v| v.get::<String>().ok()) {
                    uri
                } else {
                    gst::error!(CAT, imp = self, "Couldn't get URI for RTP header extension id {ext_id} from caps {sink_caps:?}");
                    return Err(gst::FlowError::NotNegotiated);
                }
            } else {
                gst::error!(
                    CAT,
                    imp = self,
                    "Couldn't get URI for RTP header extension id {ext_id} from caps {sink_caps:?}"
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
                    if extension.set_attributes_from_caps(sink_caps) {
                        continue;
                    }

                    // Try to get a new one for this extension ID instead
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to configure extension {ext_id} from caps {sink_caps}"
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

            if !ext.set_attributes_from_caps(sink_caps) {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to configure extension {ext_id} from caps {sink_caps}"
                );
                continue;
            }

            extensions.insert(*ext_id, ext);
            extensions_changed |= true;
        }

        // Remove all extensions that are not in the caps
        extensions.retain(|ext_id, _| {
            if !caps_extensions.contains_key(ext_id) {
                extensions_changed = true;
                false
            } else {
                true
            }
        });
        drop(extensions);

        if extensions_changed {
            self.obj().notify("extensions");
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn set_sink_caps(&self, sink_caps: gst::Caps) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, imp = self, "Received caps {sink_caps:?}");

        let s = sink_caps.structure(0).unwrap();

        if !s.has_name("application/x-rtp") {
            gst::error!(CAT, imp = self, "Non-RTP caps {sink_caps:?} not supported");
            return Err(gst::FlowError::NotNegotiated);
        }

        let clock_rate = match s.get::<i32>("clock-rate") {
            Ok(clock_rate) if clock_rate > 0 => clock_rate,
            _ => {
                gst::error!(
                    CAT,
                    imp = self,
                    "RTP caps {sink_caps:?} without 'clock-rate'"
                );
                return Err(gst::FlowError::NotNegotiated);
            }
        };

        let npt_start = s.get::<gst::ClockTime>("npt-start").ok();
        let npt_stop = s.get::<gst::ClockTime>("npt-stop").ok();
        let play_scale = s.get::<f64>("play-scale").unwrap_or(1.0);
        let play_speed = s.get::<f64>("play-speed").unwrap_or(1.0);
        let clock_base = s.get::<u32>("clock-base").ok();
        let onvif_mode = s.get::<bool>("onvif-mode").unwrap_or_default();

        self.update_extensions_from_sink_caps(&sink_caps)?;

        let state = self.state.borrow_mut();
        let same_caps = state
            .sink_caps
            .as_ref()
            .map(|old_caps| &sink_caps == old_caps)
            .unwrap_or(false);
        drop(state);

        if !same_caps {
            let obj = self.obj();

            if let Err(err) = self.drain() {
                // Just continue here. The depayloader is now flushed and can proceed below,
                // and if there was a serious error downstream it will happen on the next
                // buffer pushed downstream
                gst::debug!(CAT, imp = self, "Draining failed: {err:?}");
            }

            let set_sink_caps_res = if (obj.class().as_ref().set_sink_caps)(&obj, &sink_caps) {
                gst::debug!(CAT, imp = self, "Caps {sink_caps:?} accepted");
                Ok(gst::FlowSuccess::Ok)
            } else {
                gst::warning!(CAT, imp = self, "Caps {sink_caps:?} not accepted");
                Err(gst::FlowError::NotNegotiated)
            };

            let mut state = self.state.borrow_mut();
            if let Err(err) = set_sink_caps_res {
                state.sink_caps = None;
                // FIXME: Forget everything here?
                return Err(err);
            }

            state.sink_caps = Some(sink_caps);
            state.clock_rate = Some(clock_rate as u32);

            // If npt-start is set, all the other values are also valid.
            //
            // In ONVIF mode these are all ignored and instead upstream is supposed to provide the
            // correct segment already.
            if npt_start.is_some() && !onvif_mode {
                state.npt_start = npt_start;
                state.npt_stop = npt_stop;
                state.play_speed = play_speed;
                state.play_scale = play_scale;
                state.clock_base = clock_base;
                state.npt_start_times = None;
                state.pending_segment = state.segment.is_some();
            } else {
                if state.npt_start.is_some() {
                    state.pending_segment = state.segment.is_some();
                }
                state.npt_start = None;
                state.npt_stop = None;
                state.play_speed = 1.0;
                state.play_scale = 1.0;
                state.clock_base = None;
                state.npt_start_times = None;
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
        state.pending_packets.clear();

        // Update all pending buffers without metadata with the last buffer's PTS/DTS
        let last_pts = state.last_pts;
        let last_dts = state.last_dts;
        let mut discont_pending = state.discont_pending;
        state.discont_pending = false;

        for buffer in state
            .pending_buffers
            .iter_mut()
            .skip_while(|b| b.metadata_set)
        {
            assert!(!buffer.metadata_set);
            buffer.metadata_set = true;
            let buffer = &mut buffer.buffer;
            let buffer_ref = buffer.make_mut();
            buffer_ref.set_pts(last_pts);
            buffer_ref.set_dts(last_dts);
            if discont_pending {
                buffer_ref.set_flags(gst::BufferFlags::DISCONT);
                discont_pending = false;
            }
        }
        drop(state);

        // Forward all buffers
        let res = self.finish_pending_buffers();

        self.flush();

        res
    }

    fn flush(&self) {
        let obj = self.obj();
        (obj.class().as_ref().flush)(&obj);

        let mut state = self.state.borrow_mut();
        state.pending_packets.clear();
        state.pending_buffers.clear();
        state.discont_pending = true;
    }

    fn create_stats(&self) -> gst::Structure {
        let stats = self.stats.lock().unwrap().clone();
        if let Some(stats) = stats {
            gst::Structure::builder("application/x-rtp-depayload-stats")
                .field("ssrc", stats.ssrc)
                .field("clock-rate", stats.clock_rate)
                .field("running-time-dts", stats.running_time_dts)
                .field("running-time-pts", stats.running_time_pts)
                .field("seqnum", stats.seqnum as u32)
                .field("timestamp", stats.timestamp)
                .field("npt-start", stats.npt_start)
                .field("npt-stop", stats.npt_stop)
                .field("play-speed", stats.play_speed)
                .field("play-scale", stats.play_scale)
                .build()
        } else {
            gst::Structure::builder("application/x-rtp-depayload-stats").build()
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtpBaseDepay2 {
    const NAME: &'static str = "GstRtpBaseDepay2";
    const ABSTRACT: bool = true;
    type Type = super::RtpBaseDepay2;
    type ParentType = gst::Element;
    type Class = super::Class;

    fn class_init(class: &mut Self::Class) {
        class.start = |obj| obj.imp().start_default();
        class.stop = |obj| obj.imp().stop_default();
        class.set_sink_caps = |obj, caps| obj.imp().set_sink_caps_default(caps);
        class.handle_packet = |obj, packet| obj.imp().handle_packet_default(packet);
        class.drain = |obj| obj.imp().drain_default();
        class.flush = |obj| obj.imp().flush_default();
        class.sink_event = |obj, event| obj.imp().sink_event_default(event);
        class.src_event = |obj, event| obj.imp().src_event_default(event);
        class.sink_query = |obj, query| obj.imp().sink_query_default(query);
        class.src_query = |obj, query| obj.imp().src_query_default(query);
        class.transform_meta =
            |obj, in_buf, meta, out_buf| obj.imp().transform_meta_default(in_buf, meta, out_buf);
        class.allowed_meta_tags = &[];
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

        Self {
            src_pad,
            sink_pad,
            state: AtomicRefCell::default(),
            settings: Mutex::default(),
            stats: Mutex::default(),
            extensions: Mutex::default(),
        }
    }
}

impl ObjectImpl for RtpBaseDepay2 {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Statistics")
                    .blurb("Various statistics")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("max-reorder")
                    .nick("Maximum Reorder")
                    .blurb("Maximum seqnum reorder before assuming sender has restarted")
                    .default_value(Settings::default().max_reorder)
                    .minimum(0)
                    .maximum(0x7fff)
                    .mutable_playing()
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
                        let s = args[0].get::<super::RtpBaseDepay2>().unwrap();
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
                        let s = args[0].get::<super::RtpBaseDepay2>().unwrap();
                        let ext_id = args[1].get::<u32>().unwrap();
                        let uri = args[2].get::<&str>().unwrap();
                        let ext = s.imp().request_extension(ext_id, uri);

                        Some(ext.to_value())
                    })
                    .build(),
                glib::subclass::Signal::builder("clear-extensions")
                    .action()
                    .class_handler(|args| {
                        let s = args[0].get::<super::RtpBaseDepay2>().unwrap();
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
            "max-reorder" => {
                self.settings.lock().unwrap().max_reorder = value.get().unwrap();
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
            "stats" => self.create_stats().to_value(),
            "max-reorder" => self.settings.lock().unwrap().max_reorder.to_value(),
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

impl GstObjectImpl for RtpBaseDepay2 {}

impl ElementImpl for RtpBaseDepay2 {
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
