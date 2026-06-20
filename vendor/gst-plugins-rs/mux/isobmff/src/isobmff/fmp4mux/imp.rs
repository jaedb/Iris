// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use anyhow::{bail, Context};
use num_integer::Integer;
use std::cmp;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::mem;
use std::sync::Mutex;

use crate::av1::obu::read_seq_header_obu_bytes;
use crate::isobmff::boxes::create_dac3;
use crate::isobmff::boxes::create_dec3;
use crate::isobmff::fmp4mux::boxes::create_fmp4_fragment_header;
use crate::isobmff::fmp4mux::boxes::create_fmp4_header;
use crate::isobmff::fmp4mux::boxes::create_mfra;
use crate::isobmff::transform_matrix::TransformMatrix;
use crate::isobmff::ChunkMode;
use crate::isobmff::DeltaFrames;
use crate::isobmff::ElstInfo;
use crate::isobmff::FragmentHeaderConfiguration;
use crate::isobmff::FragmentHeaderStream;
use crate::isobmff::FragmentOffset;
use crate::isobmff::HeaderUpdateMode;
use crate::isobmff::PresentationConfiguration;
use crate::isobmff::SplitNowEvent;
use crate::isobmff::TrackConfiguration;
use crate::isobmff::Variant;
use crate::isobmff::WriteEdtsMode;
use std::sync::LazyLock;

use crate::isobmff::Buffer;

/// Offset for the segment in non-single-stream variants.
const SEGMENT_OFFSET: gst::ClockTime = gst::ClockTime::from_seconds(60 * 60 * 1000);

/// Offset between NTP and UNIX epoch in seconds.
/// NTP = UNIX + NTP_UNIX_OFFSET.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

/// Reference timestamp meta caps for NTP timestamps.
static NTP_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-ntp").build());

/// Reference timestamp meta caps for UNIX timestamps.
static UNIX_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-unix").build());

/// Returns the UTC time of the buffer in the UNIX epoch.
fn get_utc_time_from_buffer(buffer: &gst::BufferRef) -> Option<gst::ClockTime> {
    buffer
        .iter_meta::<gst::ReferenceTimestampMeta>()
        .find_map(|meta| {
            if meta.reference().can_intersect(&UNIX_CAPS) {
                Some(meta.timestamp())
            } else if meta.reference().can_intersect(&NTP_CAPS) {
                meta.timestamp().checked_sub(NTP_UNIX_OFFSET.seconds())
            } else {
                None
            }
        })
}

/// Converts a running time to an UTC time.
fn running_time_to_utc_time(
    running_time: impl Into<gst::Signed<gst::ClockTime>>,
    running_time_utc_time_mapping: (
        impl Into<gst::Signed<gst::ClockTime>>,
        impl Into<gst::Signed<gst::ClockTime>>,
    ),
) -> Option<gst::ClockTime> {
    running_time_utc_time_mapping
        .1
        .into()
        .checked_sub(running_time_utc_time_mapping.0.into())
        .and_then(|res| res.checked_add(running_time.into()))
        .and_then(|res| res.positive())
}

/// Converts an UTC time to a running time.
fn utc_time_to_running_time(
    utc_time: Option<impl Into<gst::Signed<gst::ClockTime>>>,
    running_time_utc_time_mapping: (
        impl Into<gst::Signed<gst::ClockTime>>,
        impl Into<gst::Signed<gst::ClockTime>>,
    ),
) -> Option<gst::Signed<gst::ClockTime>> {
    let utc_time = utc_time?.into();
    running_time_utc_time_mapping
        .0
        .into()
        .checked_sub(running_time_utc_time_mapping.1.into())
        .and_then(|res| res.checked_add(utc_time))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkStrategy {
    None,
    Duration(gst::ClockTime),
    Keyframe,
}

impl ChunkStrategy {
    fn is_keyframe(&self) -> bool {
        *self == ChunkStrategy::Keyframe
    }

    fn is_chunk_mode(&self) -> bool {
        *self != ChunkStrategy::None
    }
}

fn get_chunk_strategy(settings: &Settings) -> ChunkStrategy {
    match settings.chunk_mode {
        ChunkMode::Duration if settings.chunk_duration.is_some() => {
            ChunkStrategy::Duration(settings.chunk_duration.unwrap())
        }
        ChunkMode::Keyframe => ChunkStrategy::Keyframe,
        _ => ChunkStrategy::None,
    }
}

pub static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "fmp4mux",
        gst::DebugColorFlags::empty(),
        Some("FMP4Mux Element"),
    )
});

const DEFAULT_FRAGMENT_DURATION: gst::ClockTime = gst::ClockTime::from_seconds(10);
const DEFAULT_CHUNK_DURATION: Option<gst::ClockTime> = gst::ClockTime::NONE;
const DEFAULT_HEADER_UPDATE_MODE: HeaderUpdateMode = HeaderUpdateMode::None;
const DEFAULT_WRITE_MFRA: bool = false;
const DEFAULT_WRITE_MEHD: bool = false;
const DEFAULT_INTERLEAVE_BYTES: Option<u64> = None;
const DEFAULT_INTERLEAVE_TIME: Option<gst::ClockTime> = Some(gst::ClockTime::from_mseconds(250));
const DEFAULT_WRITE_EDTS_MODE: WriteEdtsMode = WriteEdtsMode::Auto;
const DEFAULT_SEND_FORCE_KEYUNIT: bool = true;
const DEFAULT_MANUAL_SPLIT: bool = false;
const DEFAULT_OFFSET_TO_ZERO: bool = false;
const DEFAULT_DECODE_TIME_OFFSET: gst::ClockTimeDiff = 0;
const DEFAULT_START_FRAGMENT_SEQUENCE_NUMBER: u32 = 1;
const DEFAULT_ENABLE_KEYFRAME_META: bool = false;
const DEFAULT_CHUNK_MODE: ChunkMode = ChunkMode::None;

#[derive(Debug, Clone)]
struct Settings {
    fragment_duration: gst::ClockTime,
    chunk_duration: Option<gst::ClockTime>,
    header_update_mode: HeaderUpdateMode,
    write_mfra: bool,
    write_mehd: bool,
    interleave_bytes: Option<u64>,
    interleave_time: Option<gst::ClockTime>,
    movie_timescale: u32,
    offset_to_zero: bool,
    write_edts_mode: WriteEdtsMode,
    send_force_keyunit: bool,
    manual_split: bool,
    decode_time_offset: gst::ClockTimeDiff,
    start_fragment_sequence_number: u32,
    enable_keyframe_meta: bool,
    chunk_mode: ChunkMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fragment_duration: DEFAULT_FRAGMENT_DURATION,
            chunk_duration: DEFAULT_CHUNK_DURATION,
            header_update_mode: DEFAULT_HEADER_UPDATE_MODE,
            write_mfra: DEFAULT_WRITE_MFRA,
            write_mehd: DEFAULT_WRITE_MEHD,
            interleave_bytes: DEFAULT_INTERLEAVE_BYTES,
            interleave_time: DEFAULT_INTERLEAVE_TIME,
            movie_timescale: 0,
            offset_to_zero: DEFAULT_OFFSET_TO_ZERO,
            write_edts_mode: DEFAULT_WRITE_EDTS_MODE,
            send_force_keyunit: DEFAULT_SEND_FORCE_KEYUNIT,
            manual_split: DEFAULT_MANUAL_SPLIT,
            decode_time_offset: DEFAULT_DECODE_TIME_OFFSET,
            start_fragment_sequence_number: DEFAULT_START_FRAGMENT_SEQUENCE_NUMBER,
            enable_keyframe_meta: DEFAULT_ENABLE_KEYFRAME_META,
            chunk_mode: DEFAULT_CHUNK_MODE,
        }
    }
}

#[derive(Debug, Clone)]
struct PreQueuedBuffer {
    /// Buffer
    ///
    /// Buffer PTS/DTS are updated to the output segment in multi-stream configurations.
    buffer: gst::Buffer,

    /// PTS
    ///
    /// In ONVIF mode this is the UTC time, otherwise it is the PTS running time.
    pts: gst::ClockTime,

    /// End PTS
    ///
    /// In ONVIF mode this is the UTC time, otherwise it is the PTS running time.
    end_pts: gst::ClockTime,

    /// DTS
    ///
    /// In ONVIF mode this is the UTC time, otherwise it is the DTS running time.
    dts: Option<gst::Signed<gst::ClockTime>>,

    /// End DTS
    ///
    /// In ONVIF mode this is the UTC time, otherwise it is the DTS running time.
    end_dts: Option<gst::Signed<gst::ClockTime>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum SplitNowType {
    Chunk,
    Fragment,
}

#[derive(Debug)]
struct GopBuffer {
    buffer: gst::Buffer,
    pts: gst::ClockTime,
    pts_position: gst::ClockTime,
    dts: Option<gst::Signed<gst::ClockTime>>,
    /// If a split-now event was received and this buffer should become the first
    /// buffer of the next fragment or chunk.
    ///
    /// There can be multiple if this stream had no buffers for a fragment or chunk.
    split_now: Vec<SplitNowType>,
}

#[derive(Debug)]
struct Gop {
    /// Start PTS.
    start_pts: gst::ClockTime,
    /// Start DTS.
    start_dts: Option<gst::Signed<gst::ClockTime>>,
    /// Earliest PTS.
    earliest_pts: gst::ClockTime,
    /// Once this is known to be the final earliest PTS/DTS
    final_earliest_pts: bool,
    /// PTS plus duration of last buffer, or start of next GOP
    end_pts: gst::ClockTime,
    /// Once this is known to be the final end PTS/DTS
    final_end_pts: bool,
    /// DTS plus duration of last buffer, or start of next GOP
    end_dts: Option<gst::Signed<gst::ClockTime>>,

    /// Earliest PTS buffer position
    earliest_pts_position: gst::ClockTime,

    /// Buffer, PTS running time, DTS running time
    buffers: Vec<GopBuffer>,
}

struct Stream {
    /// Sink pad for this stream.
    sinkpad: crate::isobmff::FMP4MuxPad,

    /// Pre-queue for ONVIF variant to timestamp all buffers with their UTC time.
    ///
    /// In non-ONVIF mode this just collects the PTS/DTS and the corresponding running
    /// times for later usage.
    pre_queue: VecDeque<PreQueuedBuffer>,

    /// Currently configured caps for this stream.
    caps: gst::Caps,
    /// Set to the new caps on caps change. If set, this stream will
    /// not accept any further buffers until the chunk/fragment is
    /// drained and draining will happen ASAP.
    next_caps: Option<gst::Caps>,
    /// Set if language or rotation tag has changed.
    tag_changed: bool,
    /// Set to true if an incomplete GOP has been drained to the last
    /// fragment and we accept delta-frames without having a
    /// key-frame. The GOP queue is empty at this point. Once it is
    /// filled again this flag will be reset.
    pushed_incomplete_gop: bool,
    /// Whether this stream is intra-only and has frame reordering.
    delta_frames: DeltaFrames,
    /// Whether this stream might have header frames without timestamps that should be ignored.
    discard_header_buffers: bool,

    /// Currently queued GOPs, including incomplete ones.
    queued_gops: VecDeque<Gop>,
    /// Whether the fully queued GOPs are filling a whole fragment.
    fragment_filled: bool,
    /// Whether a whole chunk is queued.
    chunk_filled: bool,
    // First GOP starts after the end of chunk/fragment.
    late_gop: bool,

    /// Current position (DTS, or PTS for intra-only) to prevent
    /// timestamps from going backwards when queueing new buffers
    current_position: gst::Signed<gst::ClockTime>,

    /// Mapping between running time and UTC time in ONVIF mode.
    running_time_utc_time_mapping: Option<(gst::Signed<gst::ClockTime>, gst::ClockTime)>,

    /// More data to be included in the fragmented stream header
    extra_header_data: Option<Vec<u8>>,

    /// Codec-specific boxes to be included in the sample entry
    codec_specific_boxes: Vec<u8>,

    /// Earliest PTS of the whole stream
    earliest_pts: Option<gst::ClockTime>,
    /// Current end PTS of the whole stream
    end_pts: Option<gst::ClockTime>,

    /// Language code from tags
    language_code: Option<[u8; 3]>,
    /// Orientation from tags, stream orientation takes precedence over global orientation
    global_orientation: &'static TransformMatrix,
    stream_orientation: Option<&'static TransformMatrix>,
    /// Bitrate tags
    avg_bitrate: Option<u32>,
    max_bitrate: Option<u32>,

    /// Edit list entries for this stream.
    elst_infos: Vec<ElstInfo>,

    /// Pending split-now event for this stream, if any.
    ///
    /// This will be processed on the next aggregate call once
    /// the conditions are met.
    pending_split_now: Vec<SplitNowEvent>,
}

impl Stream {
    fn get_elst_infos(&self) -> Result<Vec<ElstInfo>, anyhow::Error> {
        let mut elst_infos = self.elst_infos.clone();
        let earliest_pts = self.earliest_pts.unwrap_or(gst::ClockTime::ZERO);
        let end_pts = self
            .end_pts
            .unwrap_or(gst::ClockTime::from_nseconds(u64::MAX - 1));

        let mut iter = elst_infos
            .iter_mut()
            .filter(|e| e.start.is_some())
            .peekable();
        while let Some(&mut ref mut elst_info) = iter.next() {
            if elst_info.duration.is_none_or(|duration| duration.is_zero()) {
                elst_info.duration = if let Some(next) = iter.peek_mut() {
                    Some(
                        (next.start.unwrap() - elst_info.start.unwrap())
                            .positive()
                            .unwrap_or(gst::ClockTime::ZERO),
                    )
                } else {
                    Some(end_pts - earliest_pts)
                }
            }
        }

        Ok(elst_infos)
    }

    fn timescale(&self) -> u32 {
        let trak_timescale = { self.sinkpad.imp().state.lock().unwrap().trak_timescale };

        if trak_timescale > 0 {
            return trak_timescale;
        }

        let s = self.caps.structure(0).unwrap();

        if let Ok(fps) = s.get::<gst::Fraction>("framerate") {
            if fps.numer() == 0 {
                return 10_000;
            }

            if fps.denom() != 1 && fps.denom() != 1001 {
                if let Some(fps) = (fps.denom() as u64)
                    .nseconds()
                    .mul_div_round(1_000_000_000, fps.numer() as u64)
                    .and_then(gst_video::guess_framerate)
                {
                    return (fps.numer() as u32)
                        .mul_div_round(100, fps.denom() as u32)
                        .unwrap_or(10_000);
                }
            }

            if fps.denom() == 1001 {
                fps.numer() as u32
            } else {
                (fps.numer() as u32)
                    .mul_div_round(100, fps.denom() as u32)
                    .unwrap_or(10_000)
            }
        } else if let Ok(rate) = s.get::<i32>("rate") {
            rate as u32
        } else {
            10_000
        }
    }

    fn caps_or_tag_change(&self) -> bool {
        self.next_caps.is_some() || self.tag_changed
    }

    fn flush(&mut self) {
        self.queued_gops.clear();
        self.current_position = gst::ClockTime::MIN_SIGNED;
        self.fragment_filled = false;
        self.pre_queue.clear();
        self.running_time_utc_time_mapping = None;
        self.pending_split_now.clear();
    }

    fn orientation(&self) -> &'static TransformMatrix {
        self.stream_orientation.unwrap_or(self.global_orientation)
    }

    fn parse_language_code(lang: &str) -> Option<[u8; 3]> {
        let lang = gst_tag::language_codes::language_code_iso_639_2t(lang)?;
        if lang.len() == 3 && lang.chars().all(|c| c.is_ascii_lowercase()) {
            let mut language_code: [u8; 3] = [0; 3];
            for (out, c) in Iterator::zip(language_code.iter_mut(), lang.chars()) {
                *out = c as u8;
            }
            Some(language_code)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct State {
    /// Currently configured streams.
    streams: Vec<Stream>,

    /// Stream header with ftyp and moov box.
    ///
    /// Created once we received caps and kept up to date with the caps,
    /// sent as part of the buffer list for the first fragment.
    stream_header: Option<gst::Buffer>,

    /// Set to true if the caps of *any* sinkpad have changed or on
    /// new language code or image orientation.
    need_new_header: bool,

    /// Sequence number of the current fragment.
    sequence_number: u32,

    /// Fragment tracking for mfra box
    current_offset: u64,
    fragment_offsets: Vec<FragmentOffset>,

    /// Earliest PTS of the whole stream
    earliest_pts: Option<gst::ClockTime>,
    /// Current end PTS of the whole stream
    end_pts: Option<gst::ClockTime>,
    /// Start DTS of the whole stream
    start_dts: Option<gst::Signed<gst::ClockTime>>,

    /// Start PTS of the current fragment
    fragment_start_pts: Option<gst::ClockTime>,
    /// End PTS of the current fragment
    fragment_end_pts: Option<gst::ClockTime>,
    /// Start PTS of the current chunk
    ///
    /// This is equal to `fragment_start_pts` if the current chunk is the first of a fragment,
    /// and always equal to `fragment_start_pts` if no `chunk_duration` is set.
    chunk_start_pts: Option<gst::ClockTime>,
    /// Additional timeout delay in case GOPs are bigger than the fragment duration
    timeout_delay: gst::ClockTime,

    /// If headers (ftyp / moov box) were sent.
    sent_headers: bool,

    /// split-at-running-time requests
    pending_split_at_running_time_requests: BTreeSet<gst::ClockTime>,
}

impl State {
    fn flush(&mut self) {
        for stream in &mut self.streams {
            stream.flush();
        }

        self.current_offset = 0;
        self.fragment_offsets.clear();
        self.pending_split_at_running_time_requests.clear();
        self.end_pts = None;
        self.fragment_start_pts = None;
        self.fragment_end_pts = None;
        self.chunk_start_pts = None;
    }
    fn stream_from_pad(&self, pad: &gst_base::AggregatorPad) -> Option<&Stream> {
        self.streams.iter().find(|s| *pad == s.sinkpad)
    }
    fn mut_stream_from_pad(&mut self, pad: &gst_base::AggregatorPad) -> Option<&mut Stream> {
        self.streams.iter_mut().find(|s| *pad == s.sinkpad)
    }
}

#[derive(Default)]
pub(crate) struct FMP4Mux {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl FMP4Mux {
    fn add_elst_info(
        &self,
        buffer: &PreQueuedBuffer,
        stream: &mut Stream,
    ) -> Result<(), anyhow::Error> {
        let cmeta = if let Some(cmeta) = buffer.buffer.meta::<gst_audio::AudioClippingMeta>() {
            cmeta
        } else {
            return Ok(());
        };

        let timescale = stream
            .caps
            .structure(0)
            .unwrap()
            .get::<i32>("rate")
            .unwrap_or_else(|_| stream.timescale() as i32);

        let samples_to_gstclocktime = move |v: u64| {
            let nseconds = v
                .mul_div_round(gst::ClockTime::SECOND.nseconds(), timescale as u64)
                .context("Invalid start in the AudioClipMeta")?;
            Ok::<_, anyhow::Error>(gst::ClockTime::from_nseconds(nseconds))
        };

        let generic_to_gstclocktime = move |t| -> Result<Option<gst::ClockTime>, anyhow::Error> {
            if let gst::GenericFormattedValue::Default(Some(v)) = t {
                let v = u64::from(v);
                let v = samples_to_gstclocktime(v)?;
                Ok(Some(v).filter(|x| !x.is_zero()))
            } else if let gst::GenericFormattedValue::Time(Some(v)) = t {
                Ok(Some(v).filter(|x| !x.is_zero()))
            } else {
                Ok(None)
            }
        };

        let start = generic_to_gstclocktime(cmeta.start())?;
        let end = generic_to_gstclocktime(cmeta.end())?;

        if end.is_none() && start.is_none() {
            bail!("No start or end time in `default` format in the AudioClippingMeta");
        }

        let start = if let Some(start) = generic_to_gstclocktime(cmeta.start())? {
            start + buffer.pts
        } else {
            gst::ClockTime::ZERO
        };
        let duration = end.map(|end| buffer.end_pts - end);

        stream.elst_infos.push(ElstInfo {
            start: Some(start.into()),
            duration,
        });

        Ok(())
    }

    /// Checks if a buffer is valid according to the stream configuration.
    fn check_buffer(buffer: &mut gst::Buffer, stream: &Stream) -> Result<(), gst::FlowError> {
        let Stream {
            sinkpad,
            delta_frames,
            discard_header_buffers,
            ..
        } = stream;
        if *discard_header_buffers && buffer.flags().contains(gst::BufferFlags::HEADER) {
            return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
        }

        if buffer.dts().is_none() && delta_frames.requires_dts() {
            // For gap buffer simply set the missing DTS to PTS.
            if buffer.flags().contains(gst::BufferFlags::GAP)
                && buffer.flags().contains(gst::BufferFlags::DROPPABLE)
                && buffer.size() == 0
            {
                let pts = buffer.pts();
                buffer.make_mut().set_dts(pts);
                return Ok(());
            } else {
                gst::error!(CAT, obj = sinkpad, "Require DTS for video streams");
                return Err(gst::FlowError::Error);
            }
        }

        if buffer.pts().is_none() {
            gst::error!(CAT, obj = sinkpad, "Require timestamped buffers");
            return Err(gst::FlowError::Error);
        }

        if delta_frames.intra_only() && buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            gst::error!(CAT, obj = sinkpad, "Intra-only stream with delta units");
            return Err(gst::FlowError::Error);
        }

        Ok(())
    }

    /// Peek the currently queued buffer on this stream.
    ///
    /// This also determines the PTS/DTS that is finally going to be used, including
    /// timestamp conversion to the UTC times in ONVIF mode.
    fn peek_buffer(
        &self,
        stream: &mut Stream,
        fragment_duration: gst::ClockTime,
    ) -> Result<Option<PreQueuedBuffer>, gst::FlowError> {
        // If not in ONVIF mode or the mapping is already known and there is a pre-queued buffer
        // then we can directly return it from here.
        if self.obj().class().as_ref().variant != Variant::FragmentedONVIF
            || stream.running_time_utc_time_mapping.is_some()
        {
            if let Some(pre_queued_buffer) = stream.pre_queue.front() {
                return Ok(Some(pre_queued_buffer.clone()));
            }
        }

        if stream.caps_or_tag_change() {
            return Ok(None);
        }

        // Pop buffer here, it will be stored in the pre-queue after calculating its timestamps
        let Some(mut buffer) = stream.sinkpad.pop_buffer() else {
            return Ok(None);
        };
        Self::check_buffer(&mut buffer, stream)?;

        let segment = match stream.sinkpad.segment().downcast::<gst::ClockTime>().ok() {
            Some(segment) => segment,
            None => {
                gst::error!(CAT, obj = stream.sinkpad, "Got buffer before segment");
                return Err(gst::FlowError::Error);
            }
        };

        let pts_position = buffer.pts().unwrap();
        let duration = buffer.duration();
        let end_pts_position = duration.opt_add(pts_position).unwrap_or(pts_position);

        let pts = segment
            .to_running_time_full(pts_position)
            .ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = stream.sinkpad,
                    "Couldn't convert PTS to running time"
                );
                gst::FlowError::Error
            })?
            .positive()
            .unwrap_or_else(|| {
                gst::warning!(CAT, obj = stream.sinkpad, "Negative PTSs are not supported");
                gst::ClockTime::ZERO
            });

        let end_pts = segment
            .to_running_time_full(end_pts_position)
            .ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = stream.sinkpad,
                    "Couldn't convert end PTS to running time"
                );
                gst::FlowError::Error
            })?
            .positive()
            .unwrap_or_else(|| {
                gst::warning!(CAT, obj = stream.sinkpad, "Negative PTSs are not supported");
                gst::ClockTime::ZERO
            });

        if stream
            .earliest_pts
            .is_none_or(|earliest_pts| earliest_pts > pts)
        {
            stream.end_pts = Some(pts);
        }

        if stream.end_pts.opt_lt(end_pts).unwrap_or(true) {
            stream.end_pts = Some(end_pts);
        }

        let (dts, end_dts) = if !stream.delta_frames.requires_dts() {
            (None, None)
        } else {
            // Negative DTS are handled via the dts_offset and by having negative composition time
            // offsets in the `trun` box. The smallest DTS here is shifted to zero.
            let dts_position = buffer.dts().expect("not DTS");
            let end_dts_position = duration.opt_add(dts_position).unwrap_or(dts_position);

            let dts = segment.to_running_time_full(dts_position).ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = stream.sinkpad,
                    "Couldn't convert DTS to running time"
                );
                gst::FlowError::Error
            })?;

            let end_dts = segment
                .to_running_time_full(end_dts_position)
                .ok_or_else(|| {
                    gst::error!(
                        CAT,
                        obj = stream.sinkpad,
                        "Couldn't convert end DTS to running time"
                    );
                    gst::FlowError::Error
                })?;

            let end_dts = std::cmp::max(end_dts, dts);

            (Some(dts), Some(end_dts))
        };

        // If this is a multi-stream element then we need to update the PTS/DTS positions according
        // to the output segment, specifically to re-timestamp them with the running time and
        // adjust for the segment shift to compensate for negative DTS.
        if !self.obj().class().as_ref().variant.is_single_stream() {
            let pts_position = pts + SEGMENT_OFFSET;
            let dts_position = dts.map(|dts| {
                dts.checked_add_unsigned(SEGMENT_OFFSET)
                    .and_then(|dts| dts.positive())
                    .unwrap_or(gst::ClockTime::ZERO)
            });

            let buffer = buffer.make_mut();
            buffer.set_pts(pts_position);
            buffer.set_dts(dts_position);
        }

        if self.obj().class().as_ref().variant != Variant::FragmentedONVIF {
            // Store in the queue so we don't have to recalculate this all the time
            stream.pre_queue.push_back(PreQueuedBuffer {
                buffer,
                pts,
                end_pts,
                dts,
                end_dts,
            });
        } else if let Some(running_time_utc_time_mapping) = stream.running_time_utc_time_mapping {
            // For ONVIF we need to re-timestamp the buffer with its UTC time.
            //
            // After re-timestamping, put the buffer into the pre-queue so re-timestamping only has to
            // happen once.
            let utc_time = match get_utc_time_from_buffer(&buffer) {
                None => {
                    // Calculate from the mapping
                    running_time_to_utc_time(pts, running_time_utc_time_mapping).ok_or_else(
                        || {
                            gst::error!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream has negative PTS UTC time"
                            );
                            gst::FlowError::Error
                        },
                    )?
                }
                Some(utc_time) => utc_time,
            };
            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "Mapped PTS running time {pts} to UTC time {utc_time}"
            );

            let end_pts_utc_time =
                running_time_to_utc_time(end_pts, (pts, utc_time)).ok_or_else(|| {
                    gst::error!(
                        CAT,
                        obj = stream.sinkpad,
                        "Stream has negative end PTS UTC time"
                    );
                    gst::FlowError::Error
                })?;

            let (dts_utc_time, end_dts_utc_time) = if let Some(dts) = dts {
                let dts_utc_time =
                    running_time_to_utc_time(dts, (pts, utc_time)).ok_or_else(|| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream has negative DTS UTC time"
                        );
                        gst::FlowError::Error
                    })?;
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Mapped DTS running time {dts} to UTC time {dts_utc_time}"
                );

                let end_dts_utc_time = running_time_to_utc_time(end_dts.unwrap(), (pts, utc_time))
                    .ok_or_else(|| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream has negative end DTS UTC time"
                        );
                        gst::FlowError::Error
                    })?;

                (
                    Some(gst::Signed::Positive(dts_utc_time)),
                    Some(gst::Signed::Positive(end_dts_utc_time)),
                )
            } else {
                (None, None)
            };

            stream.pre_queue.push_back(PreQueuedBuffer {
                buffer,
                pts: utc_time,
                end_pts: end_pts_utc_time,
                dts: dts_utc_time,
                end_dts: end_dts_utc_time,
            });
        } else {
            // In ONVIF mode we need to get UTC times for each buffer and synchronize based on that.
            // Queue up to min(6s, fragment_duration) of data in the very beginning to get the first UTC time and then backdate.
            if let Some((last, first)) =
                Option::zip(stream.pre_queue.back(), stream.pre_queue.front())
            {
                // Existence of PTS/DTS checked below
                let (last, first) = if stream.delta_frames.requires_dts() {
                    (last.end_dts.unwrap(), first.end_dts.unwrap())
                } else {
                    (
                        gst::Signed::Positive(last.end_pts),
                        gst::Signed::Positive(first.end_pts),
                    )
                };

                let limit = std::cmp::min(gst::ClockTime::from_seconds(6), fragment_duration);
                if last.saturating_sub(first) > gst::Signed::Positive(limit) {
                    gst::error!(
                        CAT,
                        obj = stream.sinkpad,
                        "Got no UTC time in the first {limit} of the stream"
                    );
                    return Err(gst::FlowError::Error);
                }
            }

            let utc_time = match get_utc_time_from_buffer(&buffer) {
                Some(utc_time) => utc_time,
                None => {
                    stream.pre_queue.push_back(PreQueuedBuffer {
                        buffer,
                        pts,
                        end_pts,
                        dts,
                        end_dts,
                    });
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
            };

            let mapping = (gst::Signed::Positive(pts), utc_time);
            stream.running_time_utc_time_mapping = Some(mapping);

            // Push the buffer onto the pre-queue and re-timestamp it and all other buffers
            // based on the mapping above once we have an UTC time.
            stream.pre_queue.push_back(PreQueuedBuffer {
                buffer,
                pts,
                end_pts,
                dts,
                end_dts,
            });

            for pre_queued_buffer in stream.pre_queue.iter_mut() {
                let pts_utc_time = running_time_to_utc_time(pre_queued_buffer.pts, mapping)
                    .ok_or_else(|| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream has negative PTS UTC time"
                        );
                        gst::FlowError::Error
                    })?;
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Mapped PTS running time {} to UTC time {pts_utc_time}",
                    pre_queued_buffer.pts,
                );
                pre_queued_buffer.pts = pts_utc_time;

                let end_pts_utc_time = running_time_to_utc_time(pre_queued_buffer.end_pts, mapping)
                    .ok_or_else(|| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream has negative end PTS UTC time"
                        );
                        gst::FlowError::Error
                    })?;
                pre_queued_buffer.end_pts = end_pts_utc_time;

                if let Some(dts) = pre_queued_buffer.dts {
                    let dts_utc_time = running_time_to_utc_time(dts, mapping).ok_or_else(|| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream has negative DTS UTC time"
                        );
                        gst::FlowError::Error
                    })?;
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Mapped DTS running time {dts} to UTC time {dts_utc_time}"
                    );
                    pre_queued_buffer.dts = Some(gst::Signed::Positive(dts_utc_time));

                    let end_dts_utc_time =
                        running_time_to_utc_time(pre_queued_buffer.end_dts.unwrap(), mapping)
                            .ok_or_else(|| {
                                gst::error!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "Stream has negative DTS UTC time"
                                );
                                gst::FlowError::Error
                            })?;
                    pre_queued_buffer.end_dts = Some(gst::Signed::Positive(end_dts_utc_time));
                }
            }

            // Fall through and return the front of the queue
        }

        Ok(Some(stream.pre_queue.front().unwrap().clone()))
    }

    /// Pop the currently queued buffer from this stream.
    fn pop_buffer(&self, stream: &mut Stream) -> PreQueuedBuffer {
        // Only allowed to be called after peek was successful so there must be a buffer now
        // or in ONVIF mode we must also know the mapping now.

        assert!(!stream.pre_queue.is_empty());
        if self.obj().class().as_ref().variant == Variant::FragmentedONVIF {
            assert!(stream.running_time_utc_time_mapping.is_some());
        }

        let buffer = stream.pre_queue.pop_front().unwrap();

        if let Err(err) = self.add_elst_info(&buffer, stream) {
            gst::error!(CAT, "Failed to add elst info: {err:#}");
        }

        buffer
    }

    // Caps/tag changes are allowed only in case that the
    // header-update-mode is Caps.
    //
    // CAUTION: This function logs an error if operation is not
    // allowed so it should be evaluated only in case the caps/tags
    // would change otherwise (e. g. right-most operand in boolean
    // expressions).
    fn header_update_allowed(&self, reason: &str) -> bool {
        if self.settings.lock().unwrap().header_update_mode == HeaderUpdateMode::Caps {
            gst::debug!(
                CAT,
                imp = self,
                "Header update because incompatible change of {}",
                reason
            );
            true
        } else {
            gst::error!(
                CAT,
                imp = self,
                "Incompatible {} change not allowed if header-update-mode is enabled",
                reason
            );
            false
        }
    }

    /// Update stream caps only if they have relevant changes for the header.
    fn caps_compatible(&self, stream: &Stream, caps: &gst::CapsRef) -> bool {
        let fields: &[&str] = match caps.structure(0).unwrap().name().as_str() {
            "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9" | "video/x-av1"
            | "image/jpeg" => [
                "width",
                "height",
                "profile",
                "level",
                "tier",
                "colorimetry",
                "stream-format",
                "chroma-format",
                "bit-depth-luma",
                "codec_data",
            ]
            .as_slice(),
            "audio/mpeg" | "audio/x-opus" | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw"
            | "audio/x-ac3" | "audio/x-eac3" | "audio/x-adpcm" => {
                ["channels", "rate", "layout", "bitrate", "codec_data"].as_slice()
            }
            "application/x-onvif-metadata" => [].as_slice(),
            _ => unreachable!(),
        };

        // Now check if all relevant fields for the existing caps match the new caps
        !fields.iter().any(|f| {
            let c = stream.caps.structure(0).unwrap().value(*f);
            let n = caps.structure(0).unwrap().value(*f);
            match (c, n) {
                (Ok(c), Ok(n)) => c.compare(n) != Some(core::cmp::Ordering::Equal),
                (Err(_), Err(_)) => false,
                _ => true,
            }
        })
    }

    /// Finds the stream that has the earliest buffer queued.
    fn find_earliest_stream<'a>(
        &self,
        streams: &'a mut [Stream],
        timeout: bool,
        fragment_duration: gst::ClockTime,
    ) -> Result<Option<&'a mut Stream>, gst::FlowError> {
        if streams.iter().all(|s| s.fragment_filled || s.chunk_filled) {
            gst::trace!(
                CAT,
                imp = self,
                "All streams are currently filled and have to be drained"
            );
            return Ok(None);
        }

        let mut earliest_stream = None;
        let mut all_have_data_or_eos = true;

        for stream in streams.iter_mut() {
            let pre_queued_buffer = match Self::peek_buffer(self, stream, fragment_duration) {
                Ok(Some(buffer)) => buffer,
                Ok(None) | Err(gst_base::AGGREGATOR_FLOW_NEED_DATA) => {
                    if stream.sinkpad.is_eos() {
                        gst::trace!(CAT, obj = stream.sinkpad, "Stream is EOS");
                    } else {
                        all_have_data_or_eos = false;
                        gst::trace!(CAT, obj = stream.sinkpad, "Stream has no buffer");
                    }
                    continue;
                }
                Err(err) => return Err(err),
            };

            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "Stream has running time PTS {} / DTS {} queued",
                pre_queued_buffer.pts,
                pre_queued_buffer.dts.display(),
            );

            let running_time = if stream.delta_frames.requires_dts() {
                pre_queued_buffer.dts.unwrap()
            } else {
                gst::Signed::Positive(pre_queued_buffer.pts)
            };

            if earliest_stream
                .as_ref()
                .is_none_or(|(_stream, earliest_running_time)| {
                    *earliest_running_time > running_time
                })
            {
                earliest_stream = Some((stream, running_time));
            }
        }

        if !timeout && !all_have_data_or_eos {
            gst::trace!(
                CAT,
                imp = self,
                "No timeout and not all streams have a buffer or are EOS"
            );
            Ok(None)
        } else if let Some((stream, earliest_running_time)) = earliest_stream {
            gst::trace!(
                CAT,
                imp = self,
                "Stream {} is earliest stream with running time {}",
                stream.sinkpad.name(),
                earliest_running_time
            );
            Ok(Some(stream))
        } else {
            gst::trace!(CAT, imp = self, "No streams have data queued currently");
            Ok(None)
        }
    }

    /// Queue incoming buffer as individual GOPs.
    fn queue_gops(
        &self,
        stream: &mut Stream,
        mut pre_queued_buffer: PreQueuedBuffer,
    ) -> Result<(), gst::FlowError> {
        gst::trace!(
            CAT,
            obj = stream.sinkpad,
            "Handling buffer {:?}",
            pre_queued_buffer
        );

        let delta_frames = stream.delta_frames;

        // Enforce monotonically increasing PTS for intra-only streams, and DTS otherwise
        if !delta_frames.requires_dts() {
            if pre_queued_buffer.pts < stream.current_position {
                gst::warning!(
                    CAT,
                    obj = stream.sinkpad,
                    "Decreasing PTS {} < {}",
                    pre_queued_buffer.pts,
                    stream.current_position,
                );
                pre_queued_buffer.pts = stream.current_position.positive().unwrap();
            } else {
                stream.current_position = gst::Signed::Positive(pre_queued_buffer.pts);
            }
            pre_queued_buffer.end_pts =
                std::cmp::max(pre_queued_buffer.end_pts, pre_queued_buffer.pts);
        } else {
            let dts = pre_queued_buffer.dts.unwrap();
            let end_dts = pre_queued_buffer.end_dts.unwrap();

            // Enforce monotonically increasing DTS
            // NOTE: PTS stays the same so this will cause a bigger PTS/DTS difference
            if dts < stream.current_position {
                gst::warning!(
                    CAT,
                    obj = stream.sinkpad,
                    "Decreasing DTS {} < {}",
                    dts,
                    stream.current_position,
                );
                pre_queued_buffer.dts = Some(stream.current_position);
            } else {
                pre_queued_buffer.dts = Some(dts);
                stream.current_position = dts;
            }
            pre_queued_buffer.end_dts = Some(std::cmp::max(end_dts, dts));
        }

        let PreQueuedBuffer {
            buffer,
            pts,
            end_pts,
            dts,
            end_dts,
        } = pre_queued_buffer;

        let pts_position = buffer.pts().unwrap();

        // Accept new buffers if this is either an empty queue and we
        // have drain an incomplete GOP before or if this frame is a
        // key frame.
        if (stream.queued_gops.is_empty() && stream.pushed_incomplete_gop)
            || !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
        {
            gst::debug!(
                CAT,
                obj = stream.sinkpad,
                "Starting new GOP at PTS {} DTS {}",
                pts,
                dts.display(),
            );

            // TODO: Move this to the stream creation

            // If the stream is AV1, we need  to parse the SequenceHeader OBU to include in the
            // extra data of the 'av1C' box. It makes the stream playable in some browsers.
            let s = stream.caps.structure(0).unwrap();
            if !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
                && s.name().as_str() == "video/x-av1"
            {
                let buf_map = buffer.map_readable().map_err(|_| {
                    gst::error!(CAT, obj = stream.sinkpad, "Failed to map buffer");
                    gst::FlowError::Error
                })?;
                stream.extra_header_data =
                    read_seq_header_obu_bytes(buf_map.as_slice()).map_err(|_| {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "Failed to parse AV1 SequenceHeader OBU"
                        );
                        gst::FlowError::Error
                    })?;
            }

            // If there's a pending split-now event then we can take it here in any case because
            // we got a new keyframe.
            let split_now = stream
                .pending_split_now
                .drain(..)
                .map(|s| {
                    if s.chunk {
                        gst::info!(
                            CAT,
                            obj = stream.sinkpad,
                            "split-now event for a chunk received and received a keyframe",
                        );
                        SplitNowType::Chunk
                    } else {
                        gst::debug!(
                            CAT,
                            obj = stream.sinkpad,
                            "split-now event for a fragment received",
                        );
                        SplitNowType::Fragment
                    }
                })
                .collect();

            let gop = Gop {
                start_pts: pts,
                start_dts: dts,
                earliest_pts: pts,
                earliest_pts_position: pts_position,
                final_earliest_pts: !delta_frames.requires_dts(),
                end_pts,
                end_dts,
                final_end_pts: false,
                buffers: vec![GopBuffer {
                    buffer,
                    pts,
                    pts_position,
                    dts,
                    split_now,
                }],
            };
            stream.queued_gops.push_front(gop);

            if let Some(prev_gop) = stream.queued_gops.get_mut(1) {
                gst::debug!(
                    CAT,
                    obj = stream.sinkpad,
                    "Updating previous GOP starting at PTS {} to end PTS {} DTS {}",
                    prev_gop.earliest_pts,
                    pts,
                    dts.display(),
                );

                prev_gop.end_pts = std::cmp::max(prev_gop.end_pts, pts);
                prev_gop.end_dts = std::cmp::max(prev_gop.end_dts, dts);

                if !delta_frames.requires_dts() {
                    prev_gop.final_end_pts = true;
                }

                if !prev_gop.final_earliest_pts {
                    // Don't bother logging this for intra-only streams as it would be for every
                    // single buffer.
                    if delta_frames.requires_dts() {
                        gst::debug!(
                            CAT,
                            obj = stream.sinkpad,
                            "Previous GOP has final earliest PTS at {}",
                            prev_gop.earliest_pts
                        );
                    }

                    prev_gop.final_earliest_pts = true;
                    if let Some(prev_prev_gop) = stream.queued_gops.get_mut(2) {
                        prev_prev_gop.final_end_pts = true;
                    }
                }
            }
        } else if let Some(gop) = stream.queued_gops.front_mut() {
            assert!(!delta_frames.intra_only());

            let split_now = stream
                .pending_split_now
                .drain(..)
                .map(|s| {
                    if s.chunk {
                        gst::debug!(
                            CAT,
                            obj = stream.sinkpad,
                            "split-now event for a chunk received",
                        );
                        SplitNowType::Chunk
                    } else {
                        gst::warning!(
                            CAT,
                            obj = stream.sinkpad,
                            "split-now event for a fragment received but didn't receive a keyframe",
                        );
                        SplitNowType::Fragment
                    }
                })
                .collect();

            gop.end_pts = std::cmp::max(gop.end_pts, end_pts);
            gop.end_dts = gop.end_dts.opt_max(end_dts);
            gop.buffers.push(GopBuffer {
                buffer,
                pts,
                pts_position,
                dts,
                split_now,
            });

            if delta_frames.requires_dts() {
                let dts = dts.unwrap();

                if gop.earliest_pts > pts && !gop.final_earliest_pts {
                    gst::debug!(
                        CAT,
                        obj = stream.sinkpad,
                        "Updating current GOP earliest PTS from {} to {}",
                        gop.earliest_pts,
                        pts
                    );
                    gop.earliest_pts = pts;
                    gop.earliest_pts_position = pts_position;

                    if let Some(prev_gop) = stream.queued_gops.get_mut(1) {
                        if prev_gop.end_pts < pts {
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Updating previous GOP starting PTS {} end time from {} to {}",
                                pts,
                                prev_gop.end_pts,
                                pts
                            );
                            prev_gop.end_pts = pts;
                        }
                    }
                }

                let gop = stream.queued_gops.front_mut().unwrap();

                // The earliest PTS is known when the current DTS is bigger or equal to the first
                // PTS that was observed in this GOP. If there was another frame later that had a
                // lower PTS then it wouldn't be possible to display it in time anymore, i.e. the
                // stream would be invalid.
                if gop.start_pts <= dts && !gop.final_earliest_pts {
                    gst::debug!(
                        CAT,
                        obj = stream.sinkpad,
                        "GOP has final earliest PTS at {}",
                        gop.earliest_pts
                    );
                    gop.final_earliest_pts = true;

                    if let Some(prev_gop) = stream.queued_gops.get_mut(1) {
                        prev_gop.final_end_pts = true;
                    }
                }
            }
        } else {
            gst::warning!(
                CAT,
                obj = stream.sinkpad,
                "Waiting for keyframe at the beginning of the stream"
            );
        }

        // A buffer has been accepted so this flag can be reset.
        stream.pushed_incomplete_gop = false;

        if let Some((prev_gop, first_gop)) = Option::zip(
            stream.queued_gops.iter().find(|gop| gop.final_end_pts),
            stream.queued_gops.back(),
        ) {
            gst::debug!(
                CAT,
                obj = stream.sinkpad,
                "Queued full GOPs duration updated to {}",
                prev_gop.end_pts.saturating_sub(first_gop.earliest_pts),
            );
        }

        gst::debug!(
            CAT,
            obj = stream.sinkpad,
            "Queued duration updated to {}",
            Option::zip(stream.queued_gops.front(), stream.queued_gops.back())
                .map(|(end, start)| end.end_pts.saturating_sub(start.start_pts))
                .unwrap_or(gst::ClockTime::ZERO)
        );

        Ok(())
    }

    /// Queue buffers from all streams that are not filled for the current fragment yet
    fn queue_available_buffers(
        &self,
        state: &mut State,
        settings: &Settings,
        timeout: bool,
    ) -> Result<(), gst::FlowError> {
        let fragment_start_pts = state.fragment_start_pts;
        let fragment_end_pts = state.fragment_end_pts;
        let chunk_start_pts = state.chunk_start_pts;

        // Always take a buffer from the stream with the earliest queued buffer to keep the
        // fill-level at all sinkpads in sync.
        while let Some(stream) =
            self.find_earliest_stream(&mut state.streams, timeout, settings.fragment_duration)?
        {
            let pre_queued_buffer = Self::pop_buffer(self, stream);

            // Queue up the buffer and update GOP tracking state
            self.queue_gops(stream, pre_queued_buffer)?;

            // Check if this stream is filled enough now.
            self.check_stream_filled(
                settings,
                stream,
                fragment_start_pts,
                fragment_end_pts,
                chunk_start_pts,
            );
        }

        Ok(())
    }

    /// Check if the stream is filled enough for the current chunk / fragment.
    fn check_stream_filled(
        &self,
        settings: &Settings,
        stream: &mut Stream,
        fragment_start_pts: Option<gst::ClockTime>,
        fragment_end_pts: Option<gst::ClockTime>,
        chunk_start_pts: Option<gst::ClockTime>,
    ) {
        // Either both are none or neither is
        let Some((chunk_start_pts, fragment_start_pts)) =
            Option::zip(chunk_start_pts, fragment_start_pts)
        else {
            return;
        };

        // Simple fast-path in manual-split mode first
        if settings.manual_split {
            // GOPs are stored in reverse order, buffers in normal order
            for (gop_idx, gop) in stream.queued_gops.iter().enumerate().rev() {
                for (buffer_idx, buffer) in gop.buffers.iter().enumerate() {
                    if let Some(split_now) = buffer.split_now.first() {
                        match split_now {
                            SplitNowType::Chunk => {
                                gst::trace!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "split-now for chunk start {} at buffer {}",
                                    chunk_start_pts,
                                    buffer.pts,
                                );

                                // Need to know the earliest PTS of this GOP to be able to split off a
                                // chunk
                                if gop.final_earliest_pts || stream.sinkpad.is_eos() {
                                    // Chunk should be split off at exactly this point
                                    gst::debug!(
                                        CAT,
                                        obj = stream.sinkpad,
                                        "Stream queued enough data for this chunk"
                                    );
                                    stream.chunk_filled = true;
                                } else {
                                    gst::debug!(
                                        CAT,
                                        obj = stream.sinkpad,
                                        "Waiting for GOP earliest PTS"
                                    );
                                }
                            }
                            SplitNowType::Fragment => {
                                gst::trace!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "split-now for fragment start {} at buffer {}",
                                    fragment_start_pts,
                                    buffer.pts,
                                );

                                if buffer_idx != 0 {
                                    gst::warning!(
                                        CAT,
                                        obj = stream.sinkpad,
                                        "split-now for fragment not on the first buffer of a GOP",
                                    );
                                }

                                // Previous GOP is the one with the next higher index
                                let previous_gop = stream.queued_gops.get(gop_idx + 1);

                                // Need to have a final end PTS for the previous GOP or the stream must
                                // be EOS, otherwise we might not have the correct end PTS for this
                                // GOP.
                                //
                                // Or the fragment does not start actually at a keyframe in which case
                                // we just don't care.
                                if previous_gop.is_some_and(|gop| gop.final_end_pts)
                                    || stream.sinkpad.is_eos()
                                    || buffer_idx != 0
                                {
                                    gst::debug!(
                                        CAT,
                                        obj = stream.sinkpad,
                                        "Stream queued enough data for this fragment"
                                    );
                                    // Fragment should be split off at exactly this point
                                    stream.fragment_filled = true;
                                } else {
                                    gst::debug!(
                                        CAT,
                                        obj = stream.sinkpad,
                                        "Waiting for GOP final end PTS"
                                    );
                                }
                            }
                        }

                        // Otherwise need to wait for more data
                        return;
                    }
                }
            }

            // If currently nothing is queued, check the pending split-now events if any.
            // The stream might still be filled.
            if stream.queued_gops.is_empty() {
                if let Some(split_now) = stream.pending_split_now.first() {
                    match split_now {
                        SplitNowEvent { chunk: true } => {
                            // Chunk should be split off at exactly this point
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream queued enough data for this chunk at EOS"
                            );
                            stream.chunk_filled = true;
                        }
                        SplitNowEvent { chunk: false } => {
                            // Fragment should be split off at exactly this point
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream queued enough data for this fragment at EOS"
                            );

                            stream.fragment_filled = true;
                        }
                    }

                    return;
                }
            }
        }

        // Early return, if caps have changed assume this stream is
        // ready for pushing a fragment
        if stream.caps_or_tag_change() {
            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "On caps change stream is considered ready for fragment push",
            );
            stream.fragment_filled = true;
            stream.chunk_filled = true;
            return;
        }

        // All conditions below are not relevant for manual-split mode
        if settings.manual_split {
            return;
        }

        // Always set in non-manual-split mode unless we just started and have to wait
        let Some(fragment_end_pts) = fragment_end_pts else {
            return;
        };

        // Check if this stream is filled enough now.
        match get_chunk_strategy(settings) {
            ChunkStrategy::None => {
                // In fragment-only mode

                // Check if the end of the latest finalized GOP is after the fragment end
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Current fragment start {}, current fragment end {}",
                    fragment_start_pts,
                    fragment_end_pts,
                );

                // If the first GOP already starts after the fragment end PTS then this stream is
                // filled in the sense that it will not have any buffers for this fragment.
                if let Some(gop) = stream.queued_gops.back() {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "GOP {} start PTS {}, GOP end PTS {}",
                        stream.queued_gops.len() - 1,
                        gop.start_pts,
                        gop.end_pts,
                    );
                    if gop.start_pts > fragment_end_pts {
                        gst::debug!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream's first GOP starting after this fragment"
                        );
                        stream.fragment_filled = true;
                        stream.late_gop = true;
                        return;
                    }
                }

                let (gop_idx, gop) = match stream
                    .queued_gops
                    .iter()
                    .enumerate()
                    .find(|(_gop_idx, gop)| gop.final_end_pts || stream.sinkpad.is_eos())
                {
                    Some(gop) => gop,
                    None => {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Fragment mode but no GOP with final end PTS known yet"
                        );
                        return;
                    }
                };

                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "GOP {gop_idx} start PTS {}, GOP end PTS {}",
                    gop.start_pts,
                    gop.end_pts,
                );

                if gop.end_pts >= fragment_end_pts {
                    gst::debug!(
                        CAT,
                        obj = stream.sinkpad,
                        "Stream queued enough data for this fragment"
                    );
                    stream.fragment_filled = true;
                }
            }
            ChunkStrategy::Duration(chunk_duration) => {
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Current chunk start {}, current fragment start {}",
                    chunk_start_pts,
                    fragment_start_pts,
                );

                let chunk_end_pts = chunk_start_pts + chunk_duration;

                if fragment_end_pts < chunk_end_pts {
                    gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Current chunk end {}, current fragment end {}. Fragment end before chunk end, extending fragment",
                    chunk_end_pts,
                    fragment_end_pts,
                );
                } else {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Current chunk end {}, current fragment end {}",
                        chunk_end_pts,
                        fragment_end_pts,
                    );
                }

                // First check if the next split should be the end of a fragment or the end of a chunk.
                // If both are the same then a fragment split has preference.
                if fragment_end_pts <= chunk_end_pts {
                    // If the first GOP already starts after the fragment end PTS then this stream is
                    // filled in the sense that it will not have any buffers for this chunk.
                    if let Some(gop) = stream.queued_gops.back() {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "GOP {} start PTS {}, GOP end PTS {}",
                            stream.queued_gops.len() - 1,
                            gop.start_pts,
                            gop.end_pts,
                        );
                        // If this GOP starts after the end of the current fragment, i.e. is not
                        // included at all, then consider this stream filled as it won't contribute to
                        // this fragment.
                        //
                        // However if the first buffer of the GOP is not actually a keyframe then we
                        // previously drained a partial GOP because the GOP is ending too far after the
                        // planned fragment end.
                        if gop.start_pts > fragment_end_pts
                            && !gop.buffers.first().is_some_and(|b| {
                                b.buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
                            })
                        {
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream's first GOP starting after this fragment"
                            );
                            stream.fragment_filled = true;
                            stream.late_gop = true;
                            return;
                        }
                    }

                    // We can only finish a fragment if a full GOP with final end PTS is queued and it
                    // ends at or after the fragment end PTS.
                    if let Some((gop_idx, gop)) = stream
                        .queued_gops
                        .iter()
                        .enumerate()
                        .find(|(_idx, gop)| gop.final_end_pts || stream.sinkpad.is_eos())
                    {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "GOP {gop_idx} start PTS {}, GOP end PTS {}",
                            gop.start_pts,
                            gop.end_pts,
                        );
                        if gop.end_pts >= fragment_end_pts {
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream queued enough data for finishing this fragment"
                            );
                            stream.fragment_filled = true;
                            return;
                        }
                    }
                }

                if !stream.fragment_filled {
                    // If the first GOP already starts after the chunk end PTS then this stream is
                    // filled in the sense that it will not have any buffers for this chunk.
                    if let Some(gop) = stream.queued_gops.back() {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "GOP {} start PTS {}, GOP end PTS {}",
                            stream.queued_gops.len() - 1,
                            gop.start_pts,
                            gop.end_pts,
                        );
                        if gop.start_pts > chunk_end_pts {
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream's first GOP starting after this chunk"
                            );
                            stream.chunk_filled = true;
                            stream.late_gop = true;
                            return;
                        }
                    }

                    // We can only finish a chunk if a full GOP with final earliest PTS is queued.
                    let (gop_idx, gop) = match stream
                        .queued_gops
                        .iter()
                        .enumerate()
                        .find(|(_idx, gop)| gop.final_earliest_pts || stream.sinkpad.is_eos())
                    {
                        Some(res) => res,
                        None => {
                            gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Chunked mode and want to finish chunk but no GOP with final earliest PTS known yet",
                        );
                            return;
                        }
                    };

                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "GOP {gop_idx} start PTS {}, GOP end PTS {} (final {})",
                        gop.start_pts,
                        gop.end_pts,
                        gop.final_end_pts || stream.sinkpad.is_eos(),
                    );
                    let last_pts = gop.buffers.last().map(|b| b.pts);

                    if gop.end_pts >= chunk_end_pts
                    // only if there's another GOP or at least one further buffer
                    && (gop_idx > 0
                        || last_pts.is_some_and(|last_pts| last_pts.saturating_sub(chunk_start_pts) > chunk_duration))
                    {
                        gst::debug!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream queued enough data for this chunk"
                        );
                        stream.chunk_filled = true;
                    }
                }
            }
            ChunkStrategy::Keyframe => {
                // If the first GOP already starts after the fragment end PTS then this stream is
                // filled in the sense that it will not have any buffers for this chunk.
                if let Some(gop) = stream.queued_gops.back() {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "GOP {} start PTS {}, GOP end PTS {}, fragment end PTS {}",
                        stream.queued_gops.len() - 1,
                        gop.start_pts,
                        gop.end_pts,
                        fragment_end_pts
                    );

                    // If this GOP starts after the end of the current fragment, i.e. is not
                    // included at all, then consider this stream filled as it won't contribute to
                    // this fragment.
                    //
                    // However if the first buffer of the GOP is not actually a keyframe then we
                    // previously drained a partial GOP because the GOP is ending too far after the
                    // planned fragment end.
                    if gop.start_pts > fragment_end_pts
                        && !gop.buffers.first().is_some_and(|b| {
                            b.buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
                        })
                    {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream's first GOP starting after this fragment"
                        );
                        stream.fragment_filled = true;
                        stream.late_gop = true;
                        return;
                    }
                }

                // We can only finish a fragment if a full GOP with final end PTS is queued and it
                // ends at or after the fragment end PTS.
                if let Some((gop_idx, gop)) = stream
                    .queued_gops
                    .iter()
                    .enumerate()
                    .find(|(_idx, gop)| gop.final_end_pts || stream.sinkpad.is_eos())
                {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "GOP {gop_idx} start PTS {}, GOP end PTS {}",
                        gop.start_pts,
                        gop.end_pts,
                    );

                    if gop.end_pts >= fragment_end_pts {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Stream queued enough data for finishing this fragment"
                        );
                        stream.fragment_filled = true;
                        return;
                    }
                }

                if Option::zip(
                    stream.queued_gops.iter().find(|gop| gop.final_end_pts),
                    stream.queued_gops.back(),
                )
                .is_some()
                {
                    stream.chunk_filled = true;
                }
            }
        }
    }

    /// Calculate / update the fragment end PTS.
    ///
    /// This takes into account the configured fragment-duration and the pending
    /// split-at-running-time requests and updates or sets the fragment end PTS accordingly.
    ///
    /// Also cleans up all past split-at-running-time requests.
    fn calculate_fragment_end_pts(&self, settings: &Settings, state: &mut State) {
        use std::ops::Bound;

        // In manual-split mode, fragment end PTS is never set
        if settings.manual_split {
            state.fragment_end_pts = None;
            return;
        }

        let Some(fragment_start_pts) = state.fragment_start_pts else {
            return;
        };

        while let Some(boundary) = state.pending_split_at_running_time_requests.first() {
            if *boundary > fragment_start_pts {
                break;
            }
            let _ = state.pending_split_at_running_time_requests.pop_first();
        }

        let scheduled_fragment_end_pts = fragment_start_pts + settings.fragment_duration;
        let earliest_requested_fragment_end_pts = state
            .pending_split_at_running_time_requests
            .range((Bound::Excluded(fragment_start_pts), Bound::Unbounded))
            .next()
            .copied();

        let new_fragment_end_pts = if let Some(earliest_requested_fragment_end_pts) =
            earliest_requested_fragment_end_pts
        {
            cmp::min(
                scheduled_fragment_end_pts,
                earliest_requested_fragment_end_pts,
            )
        } else {
            scheduled_fragment_end_pts
        };

        if state.fragment_end_pts != Some(new_fragment_end_pts) {
            gst::debug!(
                CAT,
                imp = self,
                "Updating fragment end PTS from {} to {}",
                state.fragment_end_pts.display(),
                new_fragment_end_pts,
            );
            state.fragment_end_pts = Some(new_fragment_end_pts);
        }
    }

    /// Calculate earliest PTS, i.e. PTS of the very first fragment.
    ///
    /// This also sends a force-keyunit event for the start of the second fragment.
    fn calculate_earliest_pts(
        &self,
        settings: &Settings,
        state: &mut State,
        upstream_events: &mut Vec<(crate::isobmff::FMP4MuxPad, gst::Event)>,
        all_eos: bool,
        timeout: bool,
    ) {
        if state.earliest_pts.is_some() {
            return;
        }

        // Calculate the earliest PTS after queueing input if we can now.
        let mut earliest_pts = None;
        let mut start_dts = None;
        for stream in &state.streams {
            let (stream_earliest_pts, stream_start_dts) = match stream.queued_gops.back() {
                None => {
                    if !all_eos && !timeout && !state.need_new_header {
                        earliest_pts = None;
                        start_dts = None;
                        break;
                    }
                    continue;
                }
                Some(oldest_gop) => {
                    if !all_eos
                        && !timeout
                        && !state.need_new_header
                        && !oldest_gop.final_earliest_pts
                    {
                        earliest_pts = None;
                        start_dts = None;
                        break;
                    }

                    (oldest_gop.earliest_pts, oldest_gop.start_dts)
                }
            };

            if earliest_pts.opt_gt(stream_earliest_pts).unwrap_or(true) {
                earliest_pts = Some(stream_earliest_pts);
            }

            if let Some(stream_start_dts) = stream_start_dts {
                if start_dts.opt_gt(stream_start_dts).unwrap_or(true) {
                    start_dts = Some(stream_start_dts);
                }
            }
        }

        let earliest_pts = match earliest_pts {
            Some(earliest_pts) => earliest_pts,
            None => return,
        };

        // The earliest PTS is known and as such the start of the first and second fragment.
        gst::info!(
            CAT,
            imp = self,
            "Got earliest PTS {}, start DTS {} (timeout: {timeout}, all eos: {all_eos}, caps changed: {})",
            earliest_pts,
            start_dts.display(),
            state.need_new_header,
        );

        let fragment_start_pts = earliest_pts;
        let chunk_start_pts = earliest_pts;

        state.earliest_pts = Some(earliest_pts);
        state.start_dts = start_dts;
        state.fragment_start_pts = Some(fragment_start_pts);
        state.chunk_start_pts = Some(chunk_start_pts);

        self.calculate_fragment_end_pts(settings, state);

        // Check if any of the streams are already filled enough for the first chunk/fragment.
        for stream in &mut state.streams {
            // Now send force-keyunit events for the second fragment start.
            self.request_force_keyunit_event(
                settings,
                stream,
                state.fragment_end_pts,
                upstream_events,
            );
            self.check_stream_filled(
                settings,
                stream,
                state.fragment_start_pts,
                state.fragment_end_pts,
                state.chunk_start_pts,
            );
        }
    }

    /// Drain buffers from a single stream.
    #[allow(clippy::too_many_arguments)]
    fn drain_buffers_one_stream(
        &self,
        settings: &Settings,
        stream: &mut Stream,
        need_new_header: bool,
        timeout: bool,
        all_eos: bool,
        fragment_end_pts: Option<gst::ClockTime>,
        chunk_start_pts: gst::ClockTime,
        chunk_end_pts: Option<gst::ClockTime>,
        check_fragment_start: bool,
        fragment_filled: bool,
    ) -> Result<Vec<Gop>, gst::FlowError> {
        assert!(
            timeout
                || need_new_header
                || stream.sinkpad.is_eos()
                || stream.late_gop
                || stream
                    .queued_gops
                    .get(1)
                    .is_some_and(|gop| gop.final_earliest_pts)
                || settings.chunk_duration.is_some()
                || settings.manual_split
        );

        let mut gops = Vec::with_capacity(stream.queued_gops.len());
        if stream.queued_gops.is_empty() || stream.late_gop {
            stream.late_gop = false;
            return Ok(gops);
        }

        // In manual-split mode, drain everything until the split-now event as requested.
        if settings.manual_split {
            if timeout && !stream.fragment_filled && !stream.chunk_filled && !all_eos {
                gst::warning!(
                    CAT,
                    obj = stream.sinkpad,
                    "Timeout in manual-split mode ignored",
                );
                return Ok(gops);
            }

            assert!(stream.fragment_filled || stream.chunk_filled || all_eos);

            if all_eos {
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Draining from {} until manual split or EOS",
                    chunk_start_pts,
                );
            } else {
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Draining from {} until manual split",
                    chunk_start_pts,
                );
            }

            while let Some(gop) = stream.queued_gops.back() {
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Current GOP start {} end {} (final {})",
                    gop.start_pts,
                    gop.end_pts,
                    gop.final_end_pts || stream.sinkpad.is_eos() || stream.caps_or_tag_change()
                );

                let split_index = gop
                    .buffers
                    .iter()
                    .enumerate()
                    .find(|(_, buffer)| !buffer.split_now.is_empty())
                    .map(|(split_idx, _)| split_idx);

                // If no split-now in this GOP then the whole GOP has to be included,
                // otherwise we split the GOP before the index.

                if let Some(split_index) = split_index {
                    let gop = stream.queued_gops.back_mut().unwrap();

                    // On the first buffer of the GOP, so we just stop at this point.
                    if split_index == 0 {
                        gst::trace!(CAT, obj = stream.sinkpad, "Keeping whole GOP");

                        // Remove the first split-now event
                        gop.buffers[split_index].split_now.remove(0);
                    } else {
                        let start_pts = gop.start_pts;
                        let start_dts = gop.start_dts;
                        let earliest_pts = gop.earliest_pts;
                        let earliest_pts_position = gop.earliest_pts_position;

                        let mut buffers = mem::take(&mut gop.buffers);
                        // Contains all buffers from `split_index` to the end
                        gop.buffers = buffers.split_off(split_index);

                        gop.start_pts = gop.buffers[0].pts;
                        gop.start_dts = gop.buffers[0].dts;
                        gop.earliest_pts_position = gop.buffers[0].pts_position;
                        gop.earliest_pts = gop.buffers[0].pts;

                        // Remove the first split-now event
                        gop.buffers[0].split_now.remove(0);

                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Splitting GOP and keeping PTS {}",
                            gop.buffers[0].pts,
                        );

                        let queue_gop = Gop {
                            start_pts,
                            start_dts,
                            earliest_pts,
                            final_earliest_pts: true,
                            end_pts: gop.start_pts,
                            final_end_pts: true,
                            end_dts: gop.start_dts,
                            earliest_pts_position,
                            buffers,
                        };

                        gops.push(queue_gop);
                    }

                    break;
                } else {
                    if !gop.final_end_pts && !stream.sinkpad.is_eos() {
                        gst::warning!(
                            CAT,
                            obj = stream.sinkpad,
                            "Including GOP without final end PTS",
                        );
                    }

                    gst::trace!(CAT, obj = stream.sinkpad, "Pushing complete GOP");
                    gops.push(stream.queued_gops.pop_back().unwrap());
                }
            }

            // If nothing is queued anymore then drop the first of the pending split-now
            // events.
            if stream.queued_gops.is_empty() {
                if !stream.pending_split_now.is_empty() {
                    stream.pending_split_now.remove(0);
                } else {
                    assert!(all_eos);
                }
            }

            return Ok(gops);
        }

        // Otherwise, for the first stream drain as much as necessary and decide the end of this
        // fragment or chunk, for all other streams drain up to that position.
        let fragment_end_pts = fragment_end_pts.unwrap();

        let chunk_strategy = get_chunk_strategy(settings);

        if chunk_strategy.is_chunk_mode() {
            let dequeue_end_pts = if let Some(chunk_end_pts) = chunk_end_pts {
                // Not the first stream
                chunk_end_pts
            } else if chunk_strategy.is_keyframe() {
                stream.queued_gops.back().as_ref().unwrap().end_pts
            } else if fragment_filled {
                // Fragment is filled, so only dequeue everything until the latest GOP
                fragment_end_pts
            } else {
                assert!(settings.chunk_duration.is_some());
                // Fragment is not filled and we either have a full chunk or timeout
                chunk_start_pts + settings.chunk_duration.unwrap()
            };

            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "Draining from {} up to end PTS {} / duration {}",
                chunk_start_pts,
                dequeue_end_pts,
                dequeue_end_pts.saturating_sub(chunk_start_pts),
            );

            while let Some(gop) = stream.queued_gops.back() {
                // If this should be the last chunk of a fragment then only drain every
                // finished GOP until the chunk end PTS. If there is no finished GOP for
                // this stream (it would be not the first stream then), then drain
                // everything up to the chunk end PTS.
                //
                // If this chunk is not the last chunk of a fragment then simply dequeue
                // everything up to the chunk end PTS.
                if fragment_filled {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Fragment filled, current GOP start {} end {} (final {})",
                        gop.start_pts,
                        gop.end_pts,
                        gop.final_end_pts || stream.sinkpad.is_eos() || need_new_header,
                    );

                    // If we have a final GOP, EOS or caps change then include it as long as
                    // it's either
                    //   - ending before the dequeue end PTS
                    //   - no GOPs were dequeued yet and this is the first stream
                    //
                    // The second case would happen if no GOP ends between the last chunk of the
                    // fragment and the fragment duration.
                    if (gop.final_end_pts || stream.sinkpad.is_eos() || need_new_header)
                        && (gop.end_pts <= dequeue_end_pts
                            || (gops.is_empty() && chunk_end_pts.is_none()))
                    {
                        if !gop.final_end_pts && need_new_header {
                            stream.pushed_incomplete_gop = true;
                            gst::trace!(CAT, obj = stream.sinkpad, "Pushing incomplete GOP");
                        } else {
                            gst::trace!(CAT, obj = stream.sinkpad, "Pushing whole GOP");
                        }
                        gops.push(stream.queued_gops.pop_back().unwrap());
                        continue;
                    }
                    if !gops.is_empty() {
                        break;
                    }

                    // Otherwise if this is the first stream, no full GOP is queued and the first
                    // GOP is starting inside this fragment then we need to wait for more data.
                    //
                    // If this is not the first stream then take an incomplete GOP.
                    if gop.start_pts >= dequeue_end_pts
                        || (!gop.final_earliest_pts && !stream.sinkpad.is_eos() && !need_new_header)
                    {
                        gst::trace!(CAT, obj = stream.sinkpad, "GOP starts after fragment end",);
                        break;
                    } else if chunk_end_pts.is_none() {
                        gst::info!(
                            CAT,
                            obj = stream.sinkpad,
                            "Don't have a full GOP at the end of a fragment for the first stream"
                        );
                        return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                    } else {
                        gst::info!(CAT, obj = stream.sinkpad, "Including incomplete GOP");
                    }
                } else {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Chunk filled, current GOP start {} end {} (final {})",
                        gop.start_pts,
                        gop.end_pts,
                        gop.final_end_pts || stream.sinkpad.is_eos() || need_new_header
                    );
                }

                if gop.end_pts <= dequeue_end_pts
                    && (gop.final_end_pts || stream.sinkpad.is_eos() || need_new_header)
                {
                    gst::trace!(CAT, obj = stream.sinkpad, "Pushing whole GOP",);
                    gops.push(stream.queued_gops.pop_back().unwrap());
                } else if gop.start_pts >= dequeue_end_pts
                    || (!gop.final_earliest_pts && !stream.sinkpad.is_eos() && !need_new_header)
                {
                    gst::trace!(CAT, obj = stream.sinkpad, "GOP starts after chunk end",);
                    break;
                } else {
                    let gop = stream.queued_gops.back_mut().unwrap();

                    let start_pts = gop.start_pts;
                    let start_dts = gop.start_dts;
                    let earliest_pts = gop.earliest_pts;
                    let earliest_pts_position = gop.earliest_pts_position;

                    let mut split_index = None;

                    for (idx, buffer) in gop.buffers.iter().enumerate() {
                        if buffer.pts >= dequeue_end_pts {
                            break;
                        }
                        split_index = Some(idx);
                    }
                    let split_index = match split_index {
                        Some(split_index) => split_index,
                        None => {
                            // We have B frames and the first buffer of this GOP is too far
                            // in the future.
                            gst::trace!(
                                CAT,
                                obj = stream.sinkpad,
                                "First buffer of GOP too far in the future",
                            );
                            break;
                        }
                    };

                    // The last buffer of the GOP starts before the chunk end but ends
                    // after the end. We still take it here and remove the whole GOP.
                    if split_index == gop.buffers.len() - 1 {
                        if gop.final_end_pts || stream.sinkpad.is_eos() || need_new_header {
                            gst::trace!(CAT, obj = stream.sinkpad, "Pushing whole GOP",);
                            gops.push(stream.queued_gops.pop_back().unwrap());
                        } else {
                            gst::trace!(
                                CAT,
                                obj = stream.sinkpad,
                                "Can't push whole GOP as it's not final yet",
                            );
                        }
                        break;
                    }

                    let mut buffers = mem::take(&mut gop.buffers);
                    // Contains all buffers from `split_index + 1` to the end
                    gop.buffers = buffers.split_off(split_index + 1);

                    gop.start_pts = gop.buffers[0].pts;
                    gop.start_dts = gop.buffers[0].dts;
                    gop.earliest_pts_position = gop.buffers[0].pts_position;
                    gop.earliest_pts = gop.buffers[0].pts;

                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Splitting GOP and keeping PTS {}",
                        gop.buffers[0].pts,
                    );

                    let queue_gop = Gop {
                        start_pts,
                        start_dts,
                        earliest_pts,
                        final_earliest_pts: true,
                        end_pts: gop.start_pts,
                        final_end_pts: true,
                        end_dts: gop.start_dts,
                        earliest_pts_position,
                        buffers,
                    };

                    gops.push(queue_gop);
                    break;
                }
            }

            if check_fragment_start {
                if let Some(first_buffer) = gops.first().and_then(|gop| gop.buffers.first()) {
                    if first_buffer
                        .buffer
                        .flags()
                        .contains(gst::BufferFlags::DELTA_UNIT)
                    {
                        gst::error!(
                            CAT,
                            obj = stream.sinkpad,
                            "First buffer of a new fragment is not a keyframe"
                        );
                    }
                }
            }
        } else {
            // Non-chunk mode

            let dequeue_end_pts = if let Some(chunk_end_pts) = chunk_end_pts {
                // Not the first stream
                chunk_end_pts
            } else {
                fragment_end_pts
            };

            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "Draining from {} up to end PTS {} / duration {}",
                chunk_start_pts,
                dequeue_end_pts,
                dequeue_end_pts.saturating_sub(chunk_start_pts),
            );

            while let Some(gop) = stream.queued_gops.back() {
                gst::trace!(
                    CAT,
                    obj = stream.sinkpad,
                    "Current GOP start {} end {} (final {})",
                    gop.start_pts,
                    gop.end_pts,
                    gop.final_end_pts || stream.sinkpad.is_eos() || stream.caps_or_tag_change()
                );

                let end_pts = gop.end_pts;
                // If this GOP is not complete then we can't pop it yet unless the pad is EOS
                // or we had a caps change and no buffers from this stream in the fragment yet.
                //
                // If there was no complete GOP at all yet then it might be bigger than the
                // fragment duration. In this case we might not be able to handle the latency
                // requirements in a live pipeline.
                if !gop.final_end_pts && !stream.sinkpad.is_eos() {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Not including GOP without final end PTS",
                    );

                    // Still take the partial GOP if we did not yet
                    // had a full GOP but a caps change on some stream
                    // or if this stream has the caps change.
                    if !stream.pushed_incomplete_gop
                        && ((gops.is_empty() && need_new_header) || stream.caps_or_tag_change())
                    {
                        gst::trace!(
                            CAT,
                            obj = stream.sinkpad,
                            "Pushing partial GOP due to caps change",
                        );
                        stream.pushed_incomplete_gop = true;
                        gops.push(stream.queued_gops.pop_back().unwrap());
                    }

                    break;
                }

                // If this GOP starts after the fragment end then don't dequeue it yet unless this is
                // the first stream and no GOPs were dequeued at all yet. This would mean that the
                // GOP is bigger than the fragment duration.
                if !all_eos
                    && end_pts > dequeue_end_pts
                    && (chunk_end_pts.is_some() || !gops.is_empty())
                {
                    gst::trace!(CAT, obj = stream.sinkpad, "Not including GOP yet",);
                    break;
                }

                gst::trace!(CAT, obj = stream.sinkpad, "Pushing complete GOP");
                gops.push(stream.queued_gops.pop_back().unwrap());
            }
        }

        Ok(gops)
    }

    /// Flatten all GOPs, remove any gaps and calculate durations.
    #[allow(clippy::type_complexity)]
    fn flatten_gops(
        &self,
        idx: usize,
        stream: &Stream,
        gops: Vec<Gop>,
    ) -> Result<
        Option<(
            // All buffers of the GOPs without gaps
            VecDeque<Buffer>,
            // Earliest PTS
            gst::ClockTime,
            // Earliest PTS position
            gst::ClockTime,
            // End PTS
            gst::ClockTime,
            // Start DTS
            Option<gst::Signed<gst::ClockTime>>,
            // Start DTS position
            Option<gst::ClockTime>,
            // End DTS
            Option<gst::Signed<gst::ClockTime>>,
            // Start NTP time (either matches start_dts if required or earliest_pts)
            Option<gst::ClockTime>,
        )>,
        gst::FlowError,
    > {
        let last_gop = gops.last().unwrap();
        let end_pts = last_gop.end_pts;
        let end_dts = last_gop.end_dts;

        let mut gop_buffers = Vec::with_capacity(gops.iter().map(|g| g.buffers.len()).sum());
        gop_buffers.extend(gops.into_iter().flat_map(|gop| gop.buffers.into_iter()));

        // Then calculate durations for all of the buffers and get rid of any GAP buffers in
        // the process.
        // Also calculate the earliest PTS / start DTS here, which needs to consider GAP
        // buffers too.
        let mut buffers = VecDeque::with_capacity(gop_buffers.len());
        let mut earliest_pts = None;
        let mut earliest_pts_position = None;
        let mut start_dts = None;
        let mut start_dts_position = None;
        let mut start_ntp_time = None;

        let mut gop_buffers = gop_buffers.into_iter();
        while let Some(buffer) = gop_buffers.next() {
            // If this is a GAP buffer then skip it. Its duration was already considered
            // below for the non-GAP buffer preceding it, and if there was none then the
            // chunk start would be adjusted accordingly for this stream.
            if buffer.buffer.flags().contains(gst::BufferFlags::GAP)
                && buffer.buffer.flags().contains(gst::BufferFlags::DROPPABLE)
                && buffer.buffer.size() == 0
            {
                gst::trace!(CAT, obj = stream.sinkpad, "Skipping gap buffer {buffer:?}",);
                continue;
            }

            if earliest_pts.is_none_or(|earliest_pts| buffer.pts < earliest_pts) {
                earliest_pts = Some(buffer.pts);
                if !stream.delta_frames.requires_dts() {
                    let utc_time = get_utc_time_from_buffer(&buffer.buffer)
                        .and_then(|t| t.checked_add(NTP_UNIX_OFFSET.seconds()));
                    start_ntp_time = utc_time;
                }
            }
            if earliest_pts_position.is_none_or(|earliest_pts_position| {
                buffer.buffer.pts().unwrap() < earliest_pts_position
            }) {
                earliest_pts_position = Some(buffer.buffer.pts().unwrap());
            }
            if stream.delta_frames.requires_dts() && start_dts.is_none() {
                start_dts = Some(buffer.dts.unwrap());
                let utc_time = get_utc_time_from_buffer(&buffer.buffer)
                    .and_then(|t| t.checked_add(NTP_UNIX_OFFSET.seconds()));
                start_ntp_time = utc_time;
            }
            if stream.delta_frames.requires_dts() && start_dts_position.is_none() {
                start_dts_position = Some(buffer.buffer.dts().unwrap());
            }

            let timestamp = if !stream.delta_frames.requires_dts() {
                gst::Signed::Positive(buffer.pts)
            } else {
                buffer.dts.unwrap()
            };

            // Take as end timestamp the timestamp of the next non-GAP buffer
            let end_timestamp = match gop_buffers.as_slice().iter().find(|buf| {
                !buf.buffer.flags().contains(gst::BufferFlags::GAP)
                    || !buf.buffer.flags().contains(gst::BufferFlags::DROPPABLE)
                    || buf.buffer.size() != 0
            }) {
                Some(buffer) => {
                    if !stream.delta_frames.requires_dts() {
                        gst::Signed::Positive(buffer.pts)
                    } else {
                        buffer.dts.unwrap()
                    }
                }
                None => {
                    if !stream.delta_frames.requires_dts() {
                        gst::Signed::Positive(end_pts)
                    } else {
                        end_dts.unwrap()
                    }
                }
            };

            // Timestamps are enforced to monotonically increase when queueing buffers
            let duration = end_timestamp
                .checked_sub(timestamp)
                .and_then(|duration| duration.positive())
                .expect("Timestamps going backwards");

            let composition_time_offset = if !stream.delta_frames.requires_dts() {
                None
            } else {
                let pts = buffer.pts;
                let dts = buffer.dts.unwrap();

                let offset =
                    i64::try_from((gst::Signed::Positive(pts) - dts).nseconds()).map_err(|_| {
                        gst::error!(CAT, obj = stream.sinkpad, "Too big PTS/DTS difference");
                        gst::FlowError::Error
                    })?;

                Some(offset)
            };

            buffers.push_back(Buffer {
                idx,
                buffer: buffer.buffer,
                timestamp,
                duration,
                composition_time_offset,
            });
        }

        if buffers.is_empty() {
            return Ok(None);
        }

        let earliest_pts = earliest_pts.unwrap();
        let earliest_pts_position = earliest_pts_position.unwrap();
        if stream.delta_frames.requires_dts() {
            assert!(start_dts.is_some());
            assert!(start_dts_position.is_some());
        }
        let start_dts = start_dts;
        let start_dts_position = start_dts_position;

        // Now need to adjust for negative DTS, which does not only include actually
        // negative DTS but also start DTS being before earliest PTS in general (i.e.
        // negative DTS but the whole timeline is shifted above zero).
        //
        // The buffer with the start DTS (i.e. the first buffer) gets assigned to DTS
        // zero (i.e. value of tfdt) in each fragment due to MP4 working based on all
        // samples having increasing DTS by the duration of each sample.
        //
        // We're setting the tfdt to the earliest PTS of the fragment as it is supposed
        // to be the sum of all sample durations of all previous fragments, so we
        // need to shift all composition time offsets by the difference between the start DTS and
        // the earliest PTS.
        if stream.delta_frames.requires_dts() {
            let offset =
                i64::try_from((earliest_pts - start_dts.unwrap()).nseconds()).map_err(|_| {
                    gst::error!(
                        CAT,
                        obj = stream.sinkpad,
                        "Too big earliest PTS / start DTS difference"
                    );
                    gst::FlowError::Error
                })?;

            if offset != 0 {
                for buffer in &mut buffers {
                    buffer.composition_time_offset =
                        Some(buffer.composition_time_offset.unwrap() - offset)
                }
            }
        }

        Ok(Some((
            buffers,
            earliest_pts,
            earliest_pts_position,
            end_pts,
            start_dts,
            start_dts_position,
            end_dts,
            start_ntp_time,
        )))
    }

    /// Drain buffers from all streams for the current chunk.
    ///
    /// Also removes gap buffers, calculates buffer durations and various timestamps relevant for
    /// the current chunk.
    #[allow(clippy::type_complexity)]
    fn drain_buffers(
        &self,
        state: &mut State,
        settings: &Settings,
        timeout: bool,
        all_eos: bool,
    ) -> Result<
        (
            // Drained streams
            Vec<(FragmentHeaderStream, VecDeque<Buffer>)>,
            // Minimum earliest PTS position of all streams
            Option<gst::ClockTime>,
            // Minimum earliest PTS of all streams
            Option<gst::ClockTime>,
            // Minimum start DTS position of all streams (if any stream has DTS)
            Option<gst::ClockTime>,
            // End PTS of this drained fragment or chunk, i.e. start PTS of the next fragment or
            // chunk
            Option<gst::ClockTime>,
            // With these drained buffers the current fragment is filled
            bool,
            // These buffers make the start of a new fragment
            bool,
        ),
        gst::FlowError,
    > {
        let mut drained_streams = Vec::with_capacity(state.streams.len());

        let mut min_earliest_pts_position = None;
        let mut min_earliest_pts = None;
        let mut min_start_dts_position = None;
        let mut chunk_end_pts = None;

        let fragment_start_pts = state.fragment_start_pts.unwrap();
        let fragment_end_pts = state.fragment_end_pts;
        let chunk_start_pts = state.chunk_start_pts.unwrap();
        let fragment_start = fragment_start_pts == chunk_start_pts;
        let chunk_mode = get_chunk_strategy(settings).is_chunk_mode();

        let fragment_filled = if settings.manual_split {
            // In manual-split mode we simply have to look at the first filled stream to
            // decide whether we output a full fragment or just a chunk now.

            state.streams.iter().any(|s| s.fragment_filled)
        } else {
            let fragment_end_pts = fragment_end_pts.unwrap();

            // In fragment mode, each chunk is a full fragment. Otherwise, in chunk mode,
            // this fragment is filled if it is filled for the first non-EOS stream
            // that has a GOP inside this chunk
            !chunk_mode
                || state
                    .streams
                    .iter()
                    .find(|s| {
                        !s.sinkpad.is_eos()
                            && s.queued_gops.back().is_some_and(|gop| {
                                gop.start_pts <= fragment_end_pts
                                // In chunk mode we might've drained a partial GOP as a chunk after
                                // the fragment end if the keyframe came too late. The GOP now
                                // starts with a non-keyframe after the fragment end but is part of
                                // the fragment: the fragment is extended after the end. Allow this
                                // situation here.
                                || gop.buffers.first().is_some_and(|b| {
                                    b.buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
                                })
                            })
                    })
                    .is_some_and(|s| s.fragment_filled)
        };

        // In manual-split mode, for each stream exactly as much as requested is dequeued.
        // The end PTS of the fragment / chunk is decided based on the maximum of all streams.
        //
        // Otherwise the first stream decides how much can be dequeued, if anything at all.
        //
        // In chunk mode:
        //   If more than the fragment duration has passed until the latest GOPs earliest PTS then
        //   the fragment is considered filled and all GOPs until that GOP are drained. The next
        //   chunk would start a new fragment, and would start with the keyframe at the beginning
        //   of that latest GOP.
        //
        //   Otherwise if more than a chunk duration is currently queued in GOPs of which the
        //   earliest PTS is known then drain everything up to that position. If nothing can be
        //   drained at all then advance the timeout by 1s until something can be dequeued.
        //
        // Otherwise:
        //   All complete GOPs (or at EOS everything) up to the fragment duration will be dequeued
        //   but on timeout in live pipelines it might happen that the first stream does not have a
        //   complete GOP queued. In that case nothing is dequeued for any of the streams and the
        //   timeout is advanced by 1s until at least one complete GOP can be dequeued.
        //
        // If the first stream is already EOS then the next stream that is not EOS yet will be
        // taken in its place.
        gst::info!(
            CAT,
            imp = self,
            "Starting to drain at {} (fragment start {}, fragment end {}, filled {}, chunk start {}, chunk end {})",
            chunk_start_pts,
            fragment_start_pts,
            fragment_end_pts.display(),
            fragment_filled,
            chunk_start_pts.display(),
            settings.chunk_duration.map(|duration| chunk_start_pts + duration).display(),
        );

        for (idx, stream) in state.streams.iter_mut().enumerate() {
            let gops = self.drain_buffers_one_stream(
                settings,
                stream,
                state.need_new_header,
                timeout,
                all_eos,
                fragment_end_pts,
                chunk_start_pts,
                chunk_end_pts,
                // Fragment start checks should only happen if there was no caps/tag change with
                // incomplete GOP pushed before. This will become active only in case there was an
                // incomplete GOP pushed before because otherwise the new fragment will not accept
                // inter-frames before an intra-frame anyway.
                state.sent_headers && fragment_start,
                fragment_filled,
            )?;
            stream.fragment_filled = false;
            stream.chunk_filled = false;

            if settings.manual_split || all_eos {
                if let Some(last_gop) = gops.last() {
                    if chunk_end_pts.is_none_or(|chunk_end_pts| chunk_end_pts < last_gop.end_pts) {
                        chunk_end_pts = Some(last_gop.end_pts);
                    }
                }
            } else if chunk_end_pts.is_none() {
                let fragment_end_pts = fragment_end_pts.unwrap();

                // If we don't have a next chunk start PTS then this is the first stream as above.
                if let Some(last_gop) = gops.last() {
                    // Dequeued something so let's take the end PTS of the last GOP
                    chunk_end_pts = Some(last_gop.end_pts);
                    gst::info!(
                        CAT,
                        obj = stream.sinkpad,
                        "Draining up to PTS {} for this chunk",
                        last_gop.end_pts,
                    );
                } else {
                    // If nothing was dequeued for the first stream then this is OK if we're at
                    // EOS or this stream simply has only buffers after this chunk: we just
                    // consider the next stream as first stream then.
                    let stream_after_chunk = stream.queued_gops.back().is_some_and(|gop| {
                        gop.start_pts
                            >= if fragment_filled {
                                fragment_end_pts
                            } else if settings.chunk_mode == ChunkMode::Keyframe {
                                // Stream after chunk can never be true here
                                chunk_start_pts + (gop.end_pts - gop.start_pts)
                            } else {
                                chunk_start_pts + settings.chunk_duration.unwrap()
                            }
                    });
                    if stream.sinkpad.is_eos() || stream_after_chunk {
                        // This is handled below generally if nothing was dequeued
                    } else {
                        if settings.chunk_duration.is_some() {
                            gst::debug!(
                                CAT,
                                obj = stream.sinkpad,
                                "Don't have anything to drain for the first stream on timeout in a live pipeline",
                            );
                        } else {
                            gst::warning!(
                                CAT,
                                obj = stream.sinkpad,
                                "Don't have a complete GOP for the first stream on timeout in a live pipeline",
                            );
                        }

                        // In this case we advance the timeout by 1s and hope that things are
                        // better then.
                        return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                    }
                }
            }

            let trak_timescale = stream.sinkpad.imp().state.lock().unwrap().trak_timescale;
            if gops.is_empty() {
                gst::info!(CAT, obj = stream.sinkpad, "Draining no buffers",);

                drained_streams.push((
                    FragmentHeaderStream {
                        caps: stream.caps.clone(),
                        start_time: None,
                        start_ntp_time: None,
                        delta_frames: stream.delta_frames,
                        trak_timescale,
                    },
                    VecDeque::new(),
                ));

                continue;
            }

            assert!(chunk_end_pts.is_some());

            if let Some((prev_gop, first_gop)) = Option::zip(
                stream.queued_gops.iter().find(|gop| gop.final_end_pts),
                stream.queued_gops.back(),
            ) {
                gst::debug!(
                    CAT,
                    obj = stream.sinkpad,
                    "Queued full GOPs duration updated to {}",
                    prev_gop.end_pts.saturating_sub(first_gop.earliest_pts),
                );
            }

            gst::debug!(
                CAT,
                obj = stream.sinkpad,
                "Queued duration updated to {}",
                Option::zip(stream.queued_gops.front(), stream.queued_gops.back())
                    .map(|(end, start)| end.end_pts.saturating_sub(start.start_pts))
                    .unwrap_or(gst::ClockTime::ZERO)
            );

            // First flatten all GOPs into a single `Vec`
            let buffers = self.flatten_gops(idx, stream, gops)?;
            let (
                buffers,
                earliest_pts,
                earliest_pts_position,
                end_pts,
                start_dts,
                start_dts_position,
                _end_dts,
                start_ntp_time,
            ) = match buffers {
                Some(res) => res,
                None => {
                    gst::info!(CAT, obj = stream.sinkpad, "Drained only gap buffers",);

                    drained_streams.push((
                        FragmentHeaderStream {
                            caps: stream.caps.clone(),
                            start_time: None,
                            start_ntp_time: None,
                            delta_frames: stream.delta_frames,
                            trak_timescale,
                        },
                        VecDeque::new(),
                    ));

                    continue;
                }
            };

            gst::info!(
                CAT,
                obj = stream.sinkpad,
                "Draining {} worth of buffers starting at PTS {} DTS {}",
                end_pts.saturating_sub(earliest_pts),
                earliest_pts,
                start_dts.display(),
            );

            if min_earliest_pts.opt_gt(earliest_pts).unwrap_or(true) {
                min_earliest_pts = Some(earliest_pts);
            }
            if min_earliest_pts_position
                .opt_gt(earliest_pts_position)
                .unwrap_or(true)
            {
                min_earliest_pts_position = Some(earliest_pts_position);
            }
            if let Some(start_dts_position) = start_dts_position {
                if min_start_dts_position
                    .opt_gt(start_dts_position)
                    .unwrap_or(true)
                {
                    min_start_dts_position = Some(start_dts_position);
                }
            }

            drained_streams.push((
                FragmentHeaderStream {
                    caps: stream.caps.clone(),
                    // We're setting the tfdt to the earliest PTS of the fragment as it is supposed
                    // to be the sum of all sample durations of all previous fragments.
                    //
                    // In case of negative DTS this is not the same as the start DTS of the
                    // fragment (actually negative or negative but the whole PTS/DTS timeline is
                    // shifted above zero) so instead we work with the earliest PTS.
                    start_time: Some(earliest_pts),
                    start_ntp_time,
                    delta_frames: stream.delta_frames,
                    trak_timescale,
                },
                buffers,
            ));
        }

        Ok((
            drained_streams,
            min_earliest_pts_position,
            min_earliest_pts,
            min_start_dts_position,
            chunk_end_pts,
            fragment_filled,
            fragment_start,
        ))
    }

    /// Interleave drained buffers of each stream for this chunk according to the settings.
    #[allow(clippy::type_complexity)]
    fn interleave_buffers(
        &self,
        settings: &Settings,
        mut drained_streams: Vec<(FragmentHeaderStream, VecDeque<Buffer>)>,
    ) -> Result<(Vec<Buffer>, Vec<FragmentHeaderStream>), gst::FlowError> {
        let mut interleaved_buffers =
            Vec::with_capacity(drained_streams.iter().map(|(_, bufs)| bufs.len()).sum());
        while let Some((_idx, (_, bufs))) =
            drained_streams
                .iter_mut()
                .enumerate()
                .min_by(|(a_idx, (_, a)), (b_idx, (_, b))| {
                    let (a, b) = match (a.front(), b.front()) {
                        (None, None) => return std::cmp::Ordering::Equal,
                        (None, _) => return std::cmp::Ordering::Greater,
                        (_, None) => return std::cmp::Ordering::Less,
                        (Some(a), Some(b)) => (a, b),
                    };

                    match a.timestamp.cmp(&b.timestamp) {
                        std::cmp::Ordering::Equal => a_idx.cmp(b_idx),
                        cmp => cmp,
                    }
                })
        {
            let start_time = match bufs.front() {
                None => {
                    // No more buffers now
                    break;
                }
                Some(buf) => buf.timestamp,
            };
            let mut current_end_time = start_time;
            let mut dequeued_bytes = 0;

            while settings
                .interleave_bytes
                .opt_ge(dequeued_bytes)
                .unwrap_or(true)
                && settings
                    .interleave_time
                    .opt_ge(current_end_time.saturating_sub(start_time))
                    .unwrap_or(true)
            {
                if let Some(buffer) = bufs.pop_front() {
                    current_end_time = buffer.timestamp + buffer.duration;
                    dequeued_bytes += buffer.buffer.size() as u64;
                    interleaved_buffers.push(buffer);
                } else {
                    // No buffers left in this stream, go to next stream
                    break;
                }
            }
        }

        // All buffers should be consumed now
        assert!(drained_streams.iter().all(|(_, bufs)| bufs.is_empty()));

        let streams = drained_streams
            .into_iter()
            .map(|(stream, _)| stream)
            .collect::<Vec<_>>();

        Ok((interleaved_buffers, streams))
    }

    /// Request a force-keyunit event for the given PTS.
    fn request_force_keyunit_event(
        &self,
        settings: &Settings,
        stream: &Stream,
        pts: Option<gst::ClockTime>,
        upstream_events: &mut Vec<(crate::isobmff::FMP4MuxPad, gst::Event)>,
    ) {
        if settings.manual_split || !settings.send_force_keyunit {
            return;
        }

        // Must be set or there's nothing to do here yet
        let Some(pts) = pts else {
            return;
        };

        let current_position = stream.current_position;

        // In case of ONVIF this needs to be converted back from UTC time to
        // the stream's running time
        let (fku_time, current_position) = if self.obj().class().as_ref().variant
            == Variant::FragmentedONVIF
        {
            let Some(fku_time) =
                utc_time_to_running_time(Some(pts), stream.running_time_utc_time_mapping.unwrap())
                    .and_then(|fku_time| fku_time.positive())
            else {
                return;
            };
            (
                fku_time,
                utc_time_to_running_time(
                    Some(current_position),
                    stream.running_time_utc_time_mapping.unwrap(),
                ),
            )
        } else {
            (pts, Some(current_position))
        };

        let fku_time =
            if current_position.is_some_and(|current_position| current_position > fku_time) {
                gst::warning!(
                    CAT,
                    obj = stream.sinkpad,
                    "Sending immediate force-keyunit event late for running time {:?} at {}",
                    fku_time,
                    current_position.display(),
                );
                None
            } else {
                gst::debug!(
                    CAT,
                    obj = stream.sinkpad,
                    "Sending force-keyunit event for running time {:?}",
                    fku_time,
                );
                Some(fku_time)
            };

        let fku = gst_video::UpstreamForceKeyUnitEvent::builder()
            .running_time(fku_time)
            .all_headers(true)
            .build();

        upstream_events.push((stream.sinkpad.clone(), fku));
    }

    /// Fills upstream events as needed and returns the caps the first time draining can happen.
    ///
    /// If it returns `(_, None)` then there's currently nothing to drain anymore.
    fn drain_one_chunk(
        &self,
        state: &mut State,
        settings: &Settings,
        timeout: bool,
        at_eos: bool,
        upstream_events: &mut Vec<(crate::isobmff::FMP4MuxPad, gst::Event)>,
    ) -> Result<(Option<gst::Caps>, Option<gst::BufferList>), gst::FlowError> {
        if at_eos {
            gst::info!(CAT, imp = self, "Draining at EOS");
        } else if timeout && !settings.manual_split {
            gst::info!(CAT, imp = self, "Draining at timeout");
        } else if state.need_new_header {
            gst::info!(CAT, imp = self, "Draining on caps change");
        } else {
            if state.streams.iter().any(|stream| {
                !stream.chunk_filled && !stream.fragment_filled && !stream.sinkpad.is_eos()
            }) {
                return Ok((None, None));
            }
            gst::info!(
                CAT,
                imp = self,
                "Draining because all streams have enough data queued"
            );
        }

        // The fragment_start is set once the first buffer has been
        // accepted and then just updated with the end time of the
        // previous chunk so once set it will always be set
        if state.fragment_start_pts.is_none() {
            gst::info!(CAT, imp = self, "Drain requested with nothing to drain yet");
            return Ok((None, None));
        }

        // Collect all buffers and their timing information that are to be drained right now.
        let (
            drained_streams,
            min_earliest_pts_position,
            min_earliest_pts,
            min_start_dts_position,
            chunk_end_pts,
            fragment_filled,
            fragment_start,
        ) = self.drain_buffers(state, settings, timeout, at_eos)?;

        // Create header now if it was not created before and return the caps
        let mut caps = None;
        if state.stream_header.is_none() {
            let (_, new_caps) = self.update_header(state, settings, false)?.unwrap();
            caps = Some(new_caps);
        }

        // Interleave buffers according to the settings into a single vec
        let (mut interleaved_buffers, mut streams) =
            self.interleave_buffers(settings, drained_streams)?;

        // Offset stream start time to start at 0 in ONVIF mode, or if 'offset-to-zero' is enabled,
        // instead of using the UTC time verbatim. This would be used for the tfdt box later.
        if self.obj().class().as_ref().variant == Variant::FragmentedONVIF
            || settings.offset_to_zero
        {
            let offset = state.earliest_pts.unwrap();
            for stream in &mut streams {
                if let Some(start_time) = stream.start_time {
                    stream.start_time = Some(start_time.checked_sub(offset).unwrap());
                }
            }
        }

        if settings.decode_time_offset != 0 {
            for stream in &mut streams {
                if let Some(start_time) = stream.start_time {
                    if settings.decode_time_offset >= 0 {
                        stream.start_time = Some(
                            start_time
                                .checked_add(gst::ClockTime::from_nseconds(
                                    settings.decode_time_offset as u64,
                                ))
                                .unwrap(),
                        );
                    } else {
                        stream.start_time = Some(
                            start_time
                                .checked_sub(gst::ClockTime::from_nseconds(
                                    (-settings.decode_time_offset) as u64,
                                ))
                                .unwrap(),
                        );
                    }
                }
            }
        }

        if interleaved_buffers.is_empty() {
            // Either at EOS or only gap buffers drained.
            return Ok((caps, None));
        }

        // If there are actual buffers to output then create headers as needed and create a
        // bufferlist for all buffers that have to be output.
        let min_earliest_pts_position = min_earliest_pts_position.unwrap();
        let min_earliest_pts = min_earliest_pts.unwrap();
        let chunk_end_pts = chunk_end_pts.unwrap();

        gst::debug!(
            CAT,
            imp = self,
            concat!(
                "Draining chunk (fragment start: {} fragment end: {}) ",
                "from PTS {} to {}"
            ),
            fragment_start,
            fragment_filled,
            min_earliest_pts,
            chunk_end_pts,
        );

        let mut fmp4_header = None;
        if !state.sent_headers {
            let mut buffer = state.stream_header.as_ref().unwrap().copy();
            {
                let buffer = buffer.get_mut().unwrap();

                buffer.set_pts(min_earliest_pts_position);
                buffer.set_dts(min_start_dts_position);
                buffer.set_offset(u64::MAX);
                buffer.set_offset_end(u64::MAX);

                // Header is DISCONT|HEADER
                buffer.set_flags(gst::BufferFlags::DISCONT | gst::BufferFlags::HEADER);
            }

            fmp4_header = Some(buffer);

            gst::debug!(CAT, imp = self, "Headers will be sent now");
            state.sent_headers = true;
        }

        // TODO: Write sidx boxes before moof and rewrite once offsets are known

        let add_keyframe_meta = state.streams.len() == 1 && settings.enable_keyframe_meta;
        let sequence_number = state.sequence_number;
        // If this is the last chunk of a fragment then increment the sequence number for the
        // start of the next fragment.
        if fragment_filled || get_chunk_strategy(settings).is_keyframe() {
            state.sequence_number = state.sequence_number.wrapping_add(1);
        }

        let (mut fmp4_fragment_header, moof_offset) =
            create_fmp4_fragment_header(FragmentHeaderConfiguration {
                variant: self.obj().class().as_ref().variant,
                sequence_number,
                chunk: !fragment_start,
                streams: streams.as_slice(),
                buffers: interleaved_buffers.as_slice(),
                last_fragment: at_eos,
            })
            .map_err(|err| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to create FMP4 fragment header: {}",
                    err
                );
                gst::FlowError::Error
            })?;

        {
            let buffer = fmp4_fragment_header.get_mut().unwrap();
            buffer.set_pts(min_earliest_pts_position);
            buffer.set_dts(min_start_dts_position);
            buffer.set_duration(chunk_end_pts.checked_sub(min_earliest_pts));
            buffer.set_offset(sequence_number as u64);
            buffer.set_offset_end(u64::MAX);

            // Fragment and chunk header is HEADER
            buffer.set_flags(gst::BufferFlags::HEADER);
            // Chunk header is DELTA_UNIT
            if !fragment_start {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }

            // Copy metas from the first actual buffer to the fragment header. This allows
            // getting things like the reference timestamp meta or the timecode meta to identify
            // the fragment.
            let _ = interleaved_buffers[0]
                .buffer
                .copy_into(buffer, gst::BufferCopyFlags::META, ..);
        }

        let moof_start = moof_offset /* Accounts for `styp` */;
        let moof_offset = state.current_offset
            + fmp4_header.as_ref().map(|h| h.size()).unwrap_or(0) as u64
            + moof_offset;
        let buffers_len = interleaved_buffers.len();

        for (idx, buffer) in interleaved_buffers.iter_mut().enumerate() {
            let is_keyframe = !buffer.buffer.flags().contains(gst::BufferFlags::DELTA_UNIT);
            let buffer_size = buffer.buffer.size() as u64;
            let buffer_ref = buffer.buffer.make_mut();

            // Fix up buffer flags, all other buffers are DELTA_UNIT
            buffer_ref.unset_flags(gst::BufferFlags::all());
            buffer_ref.set_flags(gst::BufferFlags::DELTA_UNIT);

            if add_keyframe_meta && is_keyframe {
                let kf_length = buffer_size + (fmp4_fragment_header.size() as u64 - moof_start);
                let kf_offset = moof_start;
                let kf_duration = fmp4_fragment_header
                    .duration()
                    .expect("fragment duration should be valid here");

                let header = fmp4_fragment_header.get_mut().unwrap();
                let mut m =
                    gst::meta::CustomMeta::add(header, "FMP4KeyframeMeta").map_err(|err| {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to add keyframe meta with err: {err}",
                        );
                        gst::FlowError::Error
                    })?;

                // We chunk on key frame boundaries, so we will only ever have one.
                let s = gst::Structure::builder("keyframe")
                    .field("keyframe-duration", kf_duration)
                    .field("keyframe-length", kf_length)
                    .field("keyframe-offset", kf_offset)
                    .build();

                m.mut_structure().set("keyframe", s);
                m.mut_structure().set("eos", at_eos);
            }

            // Set the marker flag for the last buffer of the segment
            if idx == buffers_len - 1 {
                buffer_ref.set_flags(gst::BufferFlags::MARKER);
            }
        }

        let buffer_list = fmp4_header
            .into_iter()
            .chain(Some(fmp4_fragment_header))
            .chain(interleaved_buffers.into_iter().map(|buffer| buffer.buffer))
            .inspect(|b| {
                state.current_offset += b.size() as u64;
            })
            .collect::<gst::BufferList>();

        if settings.write_mfra && fragment_start {
            // Write mfra only for the main stream on fragment starts, and if there are no
            // buffers for the main stream in this segment then don't write anything.
            if let Some(FragmentHeaderStream {
                start_time: Some(start_time),
                ..
            }) = streams.first()
            {
                state.fragment_offsets.push(FragmentOffset {
                    time: *start_time,
                    offset: moof_offset,
                });
            }
        }

        state.end_pts = Some(chunk_end_pts);

        // Update for the start PTS of the next fragment / chunk
        if fragment_filled || state.need_new_header {
            state.fragment_start_pts = Some(chunk_end_pts);
            self.calculate_fragment_end_pts(settings, state);
            gst::info!(
                CAT,
                imp = self,
                "Starting new fragment at {}",
                chunk_end_pts,
            );
        } else {
            gst::info!(CAT, imp = self, "Starting new chunk at {}", chunk_end_pts);
        }
        state.chunk_start_pts = Some(chunk_end_pts);

        // If the current fragment is filled we already have the next fragment's start
        // keyframe and can request the following one.
        if fragment_filled {
            for stream in &state.streams {
                self.request_force_keyunit_event(
                    settings,
                    stream,
                    state.fragment_end_pts,
                    upstream_events,
                );
            }
        }

        // Reset timeout delay now that we've output an actual fragment or chunk
        state.timeout_delay = gst::ClockTime::ZERO;

        // TODO: Write edit list at EOS
        // TODO: Rewrite bitrates at EOS

        Ok((caps, Some(buffer_list)))
    }

    /// Drain all chunks that can currently be drained.
    ///
    /// On error the `caps`, `buffers` or `upstream_events` can contain data of already finished
    /// chunks that were complete before the error.
    #[allow(clippy::too_many_arguments)]
    fn drain(
        &self,
        state: &mut State,
        settings: &Settings,
        all_eos: bool,
        mut timeout: bool,
        caps: &mut Option<gst::Caps>,
        buffers: &mut Vec<gst::BufferList>,
        upstream_events: &mut Vec<(crate::isobmff::FMP4MuxPad, gst::Event)>,
    ) -> Result<(), gst::FlowError> {
        // Loop as long as new chunks can be drained.
        loop {
            // If enough GOPs were queued, drain and create the output fragment or chunk
            let res = self.drain_one_chunk(state, settings, timeout, all_eos, upstream_events);
            let mut buffer_list = match res {
                Ok((new_caps, buffer_list)) => {
                    if caps.is_none() {
                        *caps = new_caps;
                    }

                    buffer_list
                }
                Err(err) => {
                    if err == gst_base::AGGREGATOR_FLOW_NEED_DATA {
                        assert!(!all_eos);
                        gst::debug!(CAT, imp = self, "Need more data");
                        if timeout {
                            state.timeout_delay += 1.seconds();
                        }
                    }

                    return Err(err);
                }
            };

            // If nothing can't be drained anymore then break the loop, and if all streams are
            // EOS add the footers.
            if buffer_list.is_none() {
                if settings.write_mfra && all_eos {
                    gst::debug!(CAT, imp = self, "Writing mfra box");
                    match create_mfra(&state.streams[0].caps, &state.fragment_offsets) {
                        Ok(mut mfra) => {
                            {
                                let mfra = mfra.get_mut().unwrap();
                                // mfra is DELTA_UNIT like other buffers
                                mfra.set_flags(gst::BufferFlags::DELTA_UNIT);
                            }

                            if buffer_list.is_none() {
                                buffer_list = Some(gst::BufferList::new_sized(1));
                            }
                            buffer_list.as_mut().unwrap().get_mut().unwrap().add(mfra);
                            buffers.extend(buffer_list);
                        }
                        Err(err) => {
                            gst::error!(CAT, imp = self, "Failed to create mfra box: {}", err);
                        }
                    }
                }

                break Ok(());
            }

            // Otherwise extend the list of bufferlists and check again if something can be
            // drained.
            buffers.extend(buffer_list);

            // Only the first iteration is considered a timeout.
            timeout = false;

            let fragment_start_pts = state.fragment_start_pts;
            let fragment_end_pts = state.fragment_end_pts;
            let chunk_start_pts = state.chunk_start_pts;
            for stream in &mut state.streams {
                // Check if this stream is still filled enough now.
                self.check_stream_filled(
                    settings,
                    stream,
                    fragment_start_pts,
                    fragment_end_pts,
                    chunk_start_pts,
                );
            }

            // And try draining a fragment again
        }
    }

    /// Create all streams.
    fn create_streams(&self, settings: &Settings, state: &mut State) -> Result<(), gst::FlowError> {
        for pad in self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|pad| pad.downcast::<crate::isobmff::FMP4MuxPad>().unwrap())
        {
            let caps = match pad.current_caps() {
                Some(caps) => caps,
                None => {
                    gst::warning!(CAT, obj = pad, "Skipping pad without caps");
                    continue;
                }
            };

            // Check if language or orientation tags have already been
            // received
            let mut stream_orientation = Default::default();
            let mut global_orientation = Default::default();
            let mut language_code = None;
            let mut avg_bitrate = None;
            let mut max_bitrate = None;
            pad.sticky_events_foreach(|ev| {
                if let gst::EventView::Tag(ev) = ev.view() {
                    let tag = ev.tag();
                    if let Some(lang) = tag.get::<gst::tags::LanguageCode>() {
                        let lang = lang.get();
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received language code from tags: {:?}",
                            lang
                        );

                        // There is no header field for global
                        // language code, maybe because it does not
                        // really make sense, global language tags are
                        // considered to be stream local
                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Language tags scoped 'global' are considered stream tags",
                            );
                        }
                        language_code = Stream::parse_language_code(lang);
                    }
                    if let Some(orientation) = tag.get::<gst::tags::ImageOrientation>() {
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received image orientation from tags: {:?}",
                            orientation.get(),
                        );

                        if tag.scope() == gst::TagScope::Global {
                            global_orientation = TransformMatrix::from_tag(self, ev);
                        } else {
                            stream_orientation = Some(TransformMatrix::from_tag(self, ev));
                        }
                    }
                    if let Some(bitrate) = tag
                        .get::<gst::tags::MaximumBitrate>()
                        .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                    {
                        let bitrate = bitrate.get();
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received maximum bitrate from tags: {:?}",
                            bitrate
                        );

                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Bitrate tags scoped 'global' are considered stream tags",
                            );
                        }
                        max_bitrate = Some(bitrate);
                    }
                    if let Some(bitrate) = tag
                        .get::<gst::tags::Bitrate>()
                        .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                    {
                        let bitrate = bitrate.get();
                        gst::trace!(CAT, obj = pad, "Received bitrate from tags: {:?}", bitrate);

                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Bitrate tags scoped 'global' are considered stream tags",
                            );
                        }
                        avg_bitrate = Some(bitrate);
                    }
                }
                std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
            });

            gst::info!(CAT, obj = pad, "Configuring caps {:?}", caps);

            let s = caps.structure(0).unwrap();

            let mut delta_frames = DeltaFrames::IntraOnly;
            let mut discard_header_buffers = false;
            let mut codec_specific_boxes = Vec::new();
            match s.name().as_str() {
                "video/x-h264" | "video/x-h265" => {
                    if !s.has_field_with_type("codec_data", gst::Buffer::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without codec_data");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                    delta_frames = DeltaFrames::Bidirectional;
                }
                "video/x-vp8" => {
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "video/x-vp9" => {
                    if !s.has_field_with_type("colorimetry", str::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without colorimetry");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "video/x-av1" => {
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "video/x-raw" => (),
                "image/jpeg" => (),
                "audio/mpeg" => {
                    if !s.has_field_with_type("codec_data", gst::Buffer::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without codec_data");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                }
                "audio/x-opus" => {
                    if let Some(header) = s
                        .get::<gst::ArrayRef>("streamheader")
                        .ok()
                        .and_then(|a| a.first().and_then(|v| v.get::<gst::Buffer>().ok()))
                    {
                        if gst_pbutils::codec_utils_opus_parse_header(&header, None).is_err() {
                            gst::error!(CAT, obj = pad, "Received invalid Opus header");
                            return Err(gst::FlowError::NotNegotiated);
                        }
                    } else if gst_pbutils::codec_utils_opus_parse_caps(&caps, None).is_err() {
                        gst::error!(CAT, obj = pad, "Received invalid Opus caps");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                }
                "audio/x-flac" => {
                    discard_header_buffers = true;
                }
                "audio/x-ac3" | "audio/x-eac3" => {
                    let Some(first_buffer) = pad.peek_buffer() else {
                        gst::error!(
                            CAT,
                            obj = pad,
                            "Need first buffer for AC-3 / EAC-3 when creating header"
                        );
                        return Err(gst::FlowError::NotNegotiated);
                    };

                    match s.name().as_str() {
                        "audio/x-ac3" => {
                            codec_specific_boxes = match create_dac3(&first_buffer) {
                                Ok(boxes) => boxes,
                                Err(err) => {
                                    gst::error!(
                                        CAT,
                                        obj = pad,
                                        "Failed to create AC-3 codec specific box: {err}"
                                    );
                                    return Err(gst::FlowError::NotNegotiated);
                                }
                            };
                        }
                        "audio/x-eac3" => {
                            codec_specific_boxes = match create_dec3(&first_buffer) {
                                Ok(boxes) => boxes,
                                Err(err) => {
                                    gst::error!(
                                        CAT,
                                        obj = pad,
                                        "Failed to create EAC-3 codec specific box: {err}"
                                    );
                                    return Err(gst::FlowError::NotNegotiated);
                                }
                            };
                        }
                        _ => unreachable!(),
                    }
                }
                "audio/x-alaw" | "audio/x-mulaw" => (),
                "audio/x-adpcm" => (),
                "application/x-onvif-metadata" => (),
                _ => unreachable!(),
            }

            state.streams.push(Stream {
                sinkpad: pad,
                caps,
                next_caps: None,
                tag_changed: false,
                pushed_incomplete_gop: false,
                delta_frames,
                discard_header_buffers,
                pre_queue: VecDeque::new(),
                queued_gops: VecDeque::new(),
                fragment_filled: false,
                chunk_filled: false,
                late_gop: false,
                current_position: gst::ClockTime::MIN_SIGNED,
                running_time_utc_time_mapping: None,
                extra_header_data: None,
                codec_specific_boxes,
                earliest_pts: None,
                end_pts: None,
                language_code,
                global_orientation,
                stream_orientation,
                avg_bitrate,
                max_bitrate,
                elst_infos: Vec::new(),
                pending_split_now: Vec::new(),
            });
        }

        if state.streams.is_empty() {
            gst::error!(CAT, imp = self, "No streams available");
            return Err(gst::FlowError::Error);
        }

        // Sort video streams first and then audio streams and then metadata streams, and each group by pad name.
        state.streams.sort_by(|a, b| {
            let order_of_caps = |caps: &gst::CapsRef| {
                let s = caps.structure(0).unwrap();

                if s.name().starts_with("video/") {
                    0
                } else if s.name().starts_with("audio/") {
                    1
                } else if s.name().starts_with("application/x-onvif-metadata") {
                    2
                } else {
                    unimplemented!();
                }
            };

            let st_a = order_of_caps(&a.caps);
            let st_b = order_of_caps(&b.caps);

            if st_a == st_b {
                return a.sinkpad.name().cmp(&b.sinkpad.name());
            }

            st_a.cmp(&st_b)
        });

        if settings.enable_keyframe_meta {
            let video_streams = state
                .streams
                .iter()
                .filter(|s| {
                    let s = s.caps.structure(0).unwrap();
                    s.name().starts_with("video/")
                })
                .collect::<Vec<_>>();

            if video_streams.is_empty() || video_streams.len() > 1 {
                gst::element_error!(
                    self.obj(),
                    gst::StreamError::WrongType,
                    ("Invalid configuration"),
                    ["Only single stream video is allowed with keyframe meta enabled"]
                );
            }
        }

        Ok(())
    }

    /// Generate an updated header at the end and the corresponding caps with the new streamheader.
    fn update_header(
        &self,
        state: &mut State,
        settings: &Settings,
        at_eos: bool,
    ) -> Result<Option<(gst::BufferList, gst::Caps)>, gst::FlowError> {
        let aggregator = self.obj();
        let class = aggregator.class();
        let variant = class.as_ref().variant;

        if [HeaderUpdateMode::None, HeaderUpdateMode::Caps].contains(&settings.header_update_mode)
            && at_eos
        {
            return Ok(None);
        }

        assert!(!at_eos || state.streams.iter().all(|s| s.queued_gops.is_empty()));

        let duration = if at_eos
            && [HeaderUpdateMode::Update, HeaderUpdateMode::Rewrite]
                .contains(&settings.header_update_mode)
        {
            state
                .end_pts
                .opt_checked_sub(state.earliest_pts)
                .ok()
                .flatten()
        } else {
            None
        };

        let streams = state
            .streams
            .iter()
            .map(|s| {
                let trak_timescale = { s.sinkpad.imp().settings.lock().unwrap().trak_timescale };
                TrackConfiguration {
                    trak_timescale,
                    delta_frames: s.delta_frames,
                    caps: s.caps.clone(),
                    extra_header_data: s.extra_header_data.clone(),
                    codec_specific_boxes: s.codec_specific_boxes.clone(),
                    language_code: s.language_code,
                    orientation: s.orientation(),
                    max_bitrate: s.max_bitrate,
                    avg_bitrate: s.avg_bitrate,
                    elst_infos: s.get_elst_infos().unwrap_or_else(|e| {
                        gst::error!(CAT, "Could not prepare edit lists: {e:?}");

                        Vec::new()
                    }),
                    earliest_pts: gst::ClockTime::from_seconds(0),
                    end_pts: gst::ClockTime::from_seconds(0),
                    chunks: vec![],
                    image_sequence: false,
                    tai_clock_info: None,
                    auxiliary_info: vec![],
                }
            })
            .collect::<Vec<_>>();

        let write_edts = match settings.write_edts_mode {
            WriteEdtsMode::Auto => self.obj().latency().is_none(),
            WriteEdtsMode::Always => true,
            WriteEdtsMode::Never => false,
        };

        let mut buffer = create_fmp4_header(PresentationConfiguration {
            variant,
            update: at_eos,
            movie_timescale: settings.movie_timescale,
            tracks: streams,
            write_mehd: settings.write_mehd,
            duration: if at_eos { duration } else { None },
            write_edts,
        })
        .map_err(|err| {
            gst::error!(CAT, imp = self, "Failed to create FMP4 header: {}", err);
            gst::FlowError::Error
        })?;

        {
            let buffer = buffer.get_mut().unwrap();

            // No timestamps

            // Header is DISCONT|HEADER
            buffer.set_flags(gst::BufferFlags::DISCONT | gst::BufferFlags::HEADER);
        }

        // Remember stream header for later
        state.stream_header = Some(buffer.clone());

        let variant = match variant {
            Variant::FragmentedISO | Variant::DASH | Variant::FragmentedONVIF => "iso-fragmented",
            Variant::CMAF => "cmaf",
            Variant::ISO => todo!(),
            Variant::ONVIF => todo!(),
        };
        let caps = gst::Caps::builder("video/quicktime")
            .field("variant", variant)
            .field("streamheader", gst::Array::new([&buffer]))
            .build();

        let mut list = gst::BufferList::new_sized(1);
        {
            let list = list.get_mut().unwrap();
            list.add(buffer);
        }

        Ok(Some((list, caps)))
    }

    /// Finish the stream be rewriting / updating headers.
    fn finish(&self, settings: &Settings) {
        // Do remaining EOS handling after the end of the stream was pushed.
        gst::debug!(CAT, imp = self, "Doing EOS handling");

        if settings.header_update_mode == HeaderUpdateMode::None {
            // Need to output new headers if started again after EOS
            self.state.lock().unwrap().sent_headers = false;
            return;
        }

        let updated_header = self.update_header(&mut self.state.lock().unwrap(), settings, true);
        match updated_header {
            Ok(Some((buffer_list, caps))) => {
                match settings.header_update_mode {
                    HeaderUpdateMode::None | HeaderUpdateMode::Caps => unreachable!(),
                    HeaderUpdateMode::Rewrite => {
                        let mut q = gst::query::Seeking::new(gst::Format::Bytes);
                        if self.obj().src_pad().peer_query(&mut q) && q.result().0 {
                            let aggregator = self.obj();

                            aggregator.set_src_caps(&caps);

                            // Seek to the beginning with a default bytes segment
                            aggregator.update_segment(
                                &gst::FormattedSegment::<gst::format::Bytes>::new(),
                            );

                            if let Err(err) = aggregator.finish_buffer_list(buffer_list) {
                                gst::error!(
                                    CAT,
                                    imp = self,
                                    "Failed pushing updated header buffer downstream: {:?}",
                                    err,
                                );
                            }
                        } else {
                            gst::error!(
                                CAT,
                                imp = self,
                                "Can't rewrite header because downstream is not seekable"
                            );
                        }
                    }
                    HeaderUpdateMode::Update => {
                        let aggregator = self.obj();

                        aggregator.set_src_caps(&caps);
                        if let Err(err) = aggregator.finish_buffer_list(buffer_list) {
                            gst::error!(
                                CAT,
                                imp = self,
                                "Failed pushing updated header buffer downstream: {:?}",
                                err,
                            );
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to generate updated header: {:?}",
                    err
                );
            }
        }

        // Need to output new headers if started again after EOS
        self.state.lock().unwrap().sent_headers = false;
    }
}

#[glib::object_subclass]
impl ObjectSubclass for FMP4Mux {
    const NAME: &'static str = "GstFMP4Mux";
    type Type = crate::isobmff::FMP4Mux;
    type ParentType = gst_base::Aggregator;
    type Class = Class;
    type Interfaces = (gst::ChildProxy,);
}

static FMP4_SIGNAL_SEND_HEADERS: &str = "send-headers";
static FMP4_SIGNAL_SPLIT_AT_RUNNING_TIME: &str = "split-at-running-time";

impl ObjectImpl for FMP4Mux {
    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder(FMP4_SIGNAL_SEND_HEADERS)
                    .action()
                    .class_handler(|args| {
                        let element = args[0].get::<crate::isobmff::FMP4Mux>().expect("signal arg");
                        let imp = element.imp();
                        let mut state = imp.state.lock().unwrap();

                        state.sent_headers = false;
                        gst::debug!(
                            CAT,
                            obj = element,
                            "Init headers will be re-sent alongside the next chunk"
                        );

                        None
                    })
                    .build(),
                glib::subclass::Signal::builder(FMP4_SIGNAL_SPLIT_AT_RUNNING_TIME)
                    .param_types([gst::ClockTime::static_type()])
                    .action()
                    .class_handler(|args| {
                        let element = args[0].get::<crate::isobmff::FMP4Mux>().expect("signal arg");
                        let imp = element.imp();

                        let settings = imp.settings.lock().unwrap().clone();

                        let mut state = imp.state.lock().unwrap();
                        let time = args[1]
                            .get::<Option<gst::ClockTime>>()
                            .expect("time arg")
                            .unwrap_or(gst::ClockTime::ZERO);

                        if settings.manual_split {
                            gst::warning!(
                                CAT,
                                obj = element,
                                "split-at-running-time has no effect in manual-split mode",
                            );
                            return None;
                        }

                        if let Some(fragment_start_pts) = state.fragment_start_pts {
                            if time < fragment_start_pts {
                                gst::warning!(
                                    CAT,
                                    obj = element,
                                    "Ignoring split-at-running-time request for {time} before current fragment start {fragment_start_pts}",
                                );
                                return None;
                            }

                        }

                        gst::debug!(
                            CAT,
                            obj = element,
                            "New split-at-running-time request added at {time}",
                        );

                        state.pending_split_at_running_time_requests.insert(time);

                        None
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("fragment-duration")
                    .nick("Fragment Duration")
                    .blurb("Duration for each FMP4 fragment in nanoseconds")
                    .default_value(DEFAULT_FRAGMENT_DURATION.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("chunk-duration")
                    .nick("Chunk Duration")
                    .blurb("Duration for each FMP4 chunk (default = no chunks)")
                    .default_value(u64::MAX)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("header-update-mode", DEFAULT_HEADER_UPDATE_MODE)
                    .nick("Header update mode")
                    .blurb("Mode for updating the header at the end of the stream")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("write-mfra")
                    .nick("Write mfra box")
                    .blurb("Write fragment random access box at the end of the stream")
                    .default_value(DEFAULT_WRITE_MFRA)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("write-mehd")
                    .nick("Write mehd box")
                    .blurb("Write movie extends header box with the duration at the end of the stream (needs a header-update-mode enabled)")
                    .default_value(DEFAULT_WRITE_MEHD)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("interleave-bytes")
                    .nick("Interleave Bytes")
                    .blurb("Interleave between streams in bytes")
                    .default_value(DEFAULT_INTERLEAVE_BYTES.unwrap_or(0))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("interleave-time")
                    .nick("Interleave Time")
                    .blurb("Interleave between streams in nanoseconds")
                    .default_value(DEFAULT_INTERLEAVE_TIME.map(gst::ClockTime::nseconds).unwrap_or(u64::MAX))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("movie-timescale")
                    .nick("Movie Timescale")
                    .blurb("Timescale to use for the movie (units per second, 0 is automatic)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("write-edts-mode", DEFAULT_WRITE_EDTS_MODE)
                    .nick("Write edts mode")
                    .blurb("Mode for writing EDTS, when in auto mode, edts written only for non-live streams.")
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:send-force-keyunit:
                 *
                 * Send force-keyunit events to request keyframes for the start of each fragment.
                 * If this is disabled then the application needs to ensure that keyframes are
                 * provided at appropriate times.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecBoolean::builder("send-force-keyunit")
                    .nick("Send force-keyunit Events")
                    .blurb("Send force-keyunit events to request keyframes for the start of each fragment")
                    .default_value(DEFAULT_SEND_FORCE_KEYUNIT)
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:manual-split:
                 *
                 * In `manual-split=true` mode the `chunk-duration` / `fragment-duration`
                 * properties are only used for latency reporting. Similarly, the
                 * `split-at-running-time` action signal has no effect at all and no automatic
                 * `force-keyunit` events are sent.
                 *
                 * Instead, providing suitable input for splitting is the job of the application.
                 * The application is supposed to send a (serialized) `custom-downstream` event
                 * with a structure named `FMP4MuxSplitNow` to signal when fragments or chunks
                 * should be split off. A split will happen between the buffer received right
                 * before the event and the buffer received right after the event.
                 *
                 * The event has an optional boolean `chunk` field (defaults to `false`) to signal
                 * that a chunk should be created instead of a full fragment.
                 *
                 * Whenever a fragment should be created then this event should be sent right
                 * before the next keyframe. Sending the event at any other time will cause a
                 * warning but a full fragment will still be created, just that the next fragment
                 * will not start on a keyframe as requested by the application.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecBoolean::builder("manual-split")
                    .nick("Manual Split")
                    .blurb("Don't split automatically based on the fragment-duration and chunk-duration properties")
                    .default_value(DEFAULT_MANUAL_SPLIT)
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:decode-time-offset:
                 *
                 * Offset to apply to the decode time in the tfdt box. By default the offset is the
                 * running time of the first DTS or earliest PTS of the stream.
                 *
                 * This can be used to shift the decoding timeline.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecInt64::builder("decode-time-offset")
                    .nick("Decode Time Offset")
                    .blurb("Offset to apply to the tfdt")
                    .default_value(DEFAULT_DECODE_TIME_OFFSET)
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:start-fragment-sequence-number:
                 *
                 * Initial sequence number to use in the mfhd box.
                 *
                 * This is incremented with every fragment by one and stays the same between
                 * chunks.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecUInt::builder("start-fragment-sequence-number")
                    .nick("Start Fragment Sequence Number")
                    .blurb("Initial sequence number to use in the mfhd")
                    .default_value(DEFAULT_START_FRAGMENT_SEQUENCE_NUMBER)
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:enable-keyframe-meta:
                 *
                 * Writes key frame meta for use by `hlscmafsink`. The meta is a
                 * GstStructure with the name `FMP4KeyframeMeta` with the following
                 * fields.
                 *
                 * * "keyframe" GST_TYPE_STRUCTURE
                 * * "eos"      G_TYPE_BOOLEAN
                 *
                 * The "keyframe" GstStructure in turn has the following fields.
                 *
                 * * "keyframe-length"   G_TYPE_UINT64
                 * * "keyframe-offset"   G_TYPE_UINT64
                 * * "keyframe-duration" GST_TYPE_CLOCKTIME
                 *
                 * Since: plugins-rs-0.15.0
                 */
                glib::ParamSpecBoolean::builder("enable-keyframe-meta")
                    .nick("Write key frame meta")
                    .blurb("Writes key frame meta for use by `hlscmafsink`")
                    .default_value(DEFAULT_ENABLE_KEYFRAME_META)
                    .mutable_ready()
                    .build(),
               /**
                 * GstFMP4Mux:chunk-mode:
                 *
                 * Chunks on duration or key frames.
                 *
                 * Since: plugins-rs-0.15.0
                 */
                glib::ParamSpecEnum::builder_with_default("chunk-mode", DEFAULT_CHUNK_MODE)
                    .nick("Chunk mode")
                    .blurb("Mode to control chunking on key frame or duration")
                    .mutable_ready()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "fragment-duration" => {
                let mut settings = self.settings.lock().unwrap();
                let fragment_duration = value.get().expect("type checked upstream");
                if settings.fragment_duration != fragment_duration {
                    settings.fragment_duration = fragment_duration;
                    let latency = settings
                        .chunk_duration
                        .unwrap_or(settings.fragment_duration);
                    drop(settings);
                    self.obj().set_latency(latency, None);
                }
            }

            "chunk-duration" => {
                let mut settings = self.settings.lock().unwrap();
                let chunk_duration = value.get().expect("type checked upstream");
                if settings.chunk_duration != chunk_duration {
                    settings.chunk_mode = ChunkMode::Duration;
                    settings.chunk_duration = chunk_duration;
                    let latency = settings
                        .chunk_duration
                        .unwrap_or(settings.fragment_duration);
                    drop(settings);
                    self.obj().set_latency(latency, None);
                }
            }

            "header-update-mode" => {
                let mut settings = self.settings.lock().unwrap();
                settings.header_update_mode = value.get().expect("type checked upstream");
            }

            "write-mfra" => {
                let mut settings = self.settings.lock().unwrap();
                settings.write_mfra = value.get().expect("type checked upstream");
            }

            "write-mehd" => {
                let mut settings = self.settings.lock().unwrap();
                settings.write_mehd = value.get().expect("type checked upstream");
            }

            "interleave-bytes" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interleave_bytes = match value.get().expect("type checked upstream") {
                    0 => None,
                    v => Some(v),
                };
            }

            "interleave-time" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interleave_time = match value.get().expect("type checked upstream") {
                    Some(gst::ClockTime::ZERO) | None => None,
                    v => v,
                };
            }

            "movie-timescale" => {
                let mut settings = self.settings.lock().unwrap();
                settings.movie_timescale = value.get().expect("type checked upstream");
            }
            "write-edts-mode" => {
                let mut settings = self.settings.lock().unwrap();
                settings.write_edts_mode = value.get().expect("type checked upstream");
            }
            "send-force-keyunit" => {
                let mut settings = self.settings.lock().unwrap();
                settings.send_force_keyunit = value.get().expect("type checked upstream");
            }
            "manual-split" => {
                let mut settings = self.settings.lock().unwrap();
                settings.manual_split = value.get().expect("type checked upstream");
            }
            "decode-time-offset" => {
                let mut settings = self.settings.lock().unwrap();
                settings.decode_time_offset = value.get().expect("type checked upstream");
            }
            "start-fragment-sequence-number" => {
                let mut settings = self.settings.lock().unwrap();
                settings.start_fragment_sequence_number =
                    value.get().expect("type checked upstream");
            }
            "enable-keyframe-meta" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_keyframe_meta = value.get().expect("type checked upstream");
            }
            "chunk-mode" => {
                let mut settings = self.settings.lock().unwrap();
                settings.chunk_mode = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "fragment-duration" => {
                let settings = self.settings.lock().unwrap();
                settings.fragment_duration.to_value()
            }

            "chunk-duration" => {
                let settings = self.settings.lock().unwrap();
                settings.chunk_duration.to_value()
            }

            "header-update-mode" => {
                let settings = self.settings.lock().unwrap();
                settings.header_update_mode.to_value()
            }

            "write-mfra" => {
                let settings = self.settings.lock().unwrap();
                settings.write_mfra.to_value()
            }

            "write-mehd" => {
                let settings = self.settings.lock().unwrap();
                settings.write_mehd.to_value()
            }

            "interleave-bytes" => {
                let settings = self.settings.lock().unwrap();
                settings.interleave_bytes.unwrap_or(0).to_value()
            }

            "interleave-time" => {
                let settings = self.settings.lock().unwrap();
                settings.interleave_time.to_value()
            }

            "movie-timescale" => {
                let settings = self.settings.lock().unwrap();
                settings.movie_timescale.to_value()
            }
            "write-edts-mode" => {
                let settings = self.settings.lock().unwrap();
                settings.write_edts_mode.to_value()
            }
            "send-force-keyunit" => {
                let settings = self.settings.lock().unwrap();
                settings.send_force_keyunit.to_value()
            }
            "manual-split" => {
                let settings = self.settings.lock().unwrap();
                settings.manual_split.to_value()
            }
            "decode-time-offset" => {
                let settings = self.settings.lock().unwrap();
                settings.decode_time_offset.to_value()
            }
            "start-fragment-sequence-number" => {
                let settings = self.settings.lock().unwrap();
                settings.start_fragment_sequence_number.to_value()
            }
            "enable-keyframe-meta" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_keyframe_meta.to_value()
            }
            "chunk-mode" => {
                let settings = self.settings.lock().unwrap();
                settings.chunk_mode.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        let class = obj.class();
        for templ in class.pad_template_list().into_iter().filter(|templ| {
            templ.presence() == gst::PadPresence::Always
                && templ.direction() == gst::PadDirection::Sink
        }) {
            let sinkpad = gst::PadBuilder::<gst_base::AggregatorPad>::from_template(&templ)
                .flags(gst::PadFlags::ACCEPT_INTERSECT)
                .build();

            obj.add_pad(&sinkpad).unwrap();
        }

        let settings = self.settings.lock().unwrap();
        let latency = settings
            .chunk_duration
            .unwrap_or(settings.fragment_duration);
        drop(settings);
        self.obj().set_latency(latency, None);
    }
}

impl GstObjectImpl for FMP4Mux {}

impl ElementImpl for FMP4Mux {
    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let state = self.state.lock().unwrap();
        if state.stream_header.is_some() {
            gst::error!(
                CAT,
                imp = self,
                "Can't request new pads after header was generated"
            );
            return None;
        }
        drop(state);

        let pad = self.parent_request_new_pad(templ, name, caps);

        if let Some(ref pad) = pad {
            let element = self.obj();
            element.child_added(pad, &pad.name());
        }

        pad
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let element = self.obj();
        element.child_removed(pad, &pad.name());
        self.parent_release_pad(pad);
    }
}

impl AggregatorImpl for FMP4Mux {
    fn next_time(&self) -> Option<gst::ClockTime> {
        let state = self.state.lock().unwrap();
        state.chunk_start_pts.opt_add(state.timeout_delay)
    }

    fn sink_query(
        &self,
        aggregator_pad: &gst_base::AggregatorPad,
        query: &mut gst::QueryRef,
    ) -> bool {
        use gst::QueryViewMut;

        gst::trace!(CAT, obj = aggregator_pad, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let mut allowed_caps = aggregator_pad
                    .current_caps()
                    .unwrap_or_else(|| aggregator_pad.pad_template_caps());

                // Allow framerate change
                for s in allowed_caps.make_mut().iter_mut() {
                    s.remove_field("framerate");
                }

                if let Some(filter_caps) = q.filter() {
                    let mut res = filter_caps
                        .intersect_with_mode(&allowed_caps, gst::CapsIntersectMode::First);

                    // if the caps changed build new caps and reset
                    // the stream header
                    if res.is_empty() || !res.is_fixed() {
                        res = filter_caps.intersect_with_mode(
                            &aggregator_pad.pad_template_caps(),
                            gst::CapsIntersectMode::First,
                        );
                    }

                    q.set_result(&res);
                } else {
                    q.set_result(&allowed_caps);
                }

                true
            }
            _ => self.parent_sink_query(aggregator_pad, query),
        }
    }

    fn sink_event_pre_queue(
        &self,
        aggregator_pad: &gst_base::AggregatorPad,
        mut event: gst::Event,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        use gst::EventView;

        gst::trace!(CAT, obj = aggregator_pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Segment(ev) => {
                if ev.segment().format() != gst::Format::Time {
                    gst::warning!(
                        CAT,
                        obj = aggregator_pad,
                        "Received non-TIME segment, replacing with default TIME segment"
                    );
                    let segment = gst::FormattedSegment::<gst::ClockTime>::new();
                    event = gst::event::Segment::builder(&segment)
                        .seqnum(event.seqnum())
                        .build();
                }
                self.parent_sink_event_pre_queue(aggregator_pad, event)
            }
            _ => self.parent_sink_event_pre_queue(aggregator_pad, event),
        }
    }

    fn sink_event(&self, aggregator_pad: &gst_base::AggregatorPad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::trace!(CAT, obj = aggregator_pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Segment(ev) => {
                // Already fixed-up above to always be a TIME segment
                let segment = ev
                    .segment()
                    .clone()
                    .downcast::<gst::ClockTime>()
                    .expect("non-TIME segment");
                gst::info!(CAT, obj = aggregator_pad, "Received segment {:?}", segment);

                // Only forward the segment event verbatim if this is a single stream variant.
                // Otherwise we have to produce a default segment and re-timestamp all buffers
                // with their running time.
                let aggregator = self.obj();
                let class = aggregator.class();
                if class.as_ref().variant.is_single_stream() {
                    aggregator.update_segment(&segment);
                }

                self.parent_sink_event(aggregator_pad, event)
            }
            EventView::Caps(caps) => {
                let caps = caps.caps_owned();
                let s = caps.structure(0).unwrap();

                gst::trace!(CAT, obj = aggregator_pad, "Received caps {}", caps);

                if s.name().starts_with("audio/") {
                    let settings = self.settings.lock().unwrap();
                    let enable_keyframe_meta = settings.enable_keyframe_meta;
                    drop(settings);

                    if enable_keyframe_meta {
                        gst::element_error!(
                            self.obj(),
                            gst::StreamError::WrongType,
                            ("Invalid configuration"),
                            ["Audio not allowed with keyframe meta enabled"]
                        );
                    }
                }

                // Only care of caps if streams have been setup and the
                // caps actually change in an incompatible way
                let mut state = self.state.lock().unwrap();
                let stream = state.stream_from_pad(aggregator_pad);

                if state.streams.is_empty()
                    || stream.is_none_or(|s| s.caps == caps || self.caps_compatible(s, &caps))
                {
                    // No streams created yet or caps are compatible, don't have to do anything
                    drop(state);
                    self.parent_sink_event(aggregator_pad, event)
                } else if self.header_update_allowed("caps") {
                    // Stream created and caps are not compatible, but header updates are allowed
                    state.need_new_header = true;
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Update caps and send new headers {:?}",
                        caps
                    );
                    let stream = state.mut_stream_from_pad(aggregator_pad).unwrap();
                    stream.next_caps = Some(caps);
                    drop(state);
                    self.parent_sink_event(aggregator_pad, event)
                } else {
                    // Stream created and caps are not compatible, but header updates are not allowed
                    gst::warning!(
                        CAT,
                        obj = aggregator_pad,
                        "Updated caps not accepted {:?}",
                        caps
                    );

                    false
                }
            }
            EventView::Tag(ev) => {
                let tag = ev.tag();
                if let Some(tag_value) = tag.get::<gst::tags::LanguageCode>() {
                    let lang = tag_value.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received language code from tags: {:?}",
                        lang
                    );

                    // Language as ISO-639-2/T
                    if let Some(language_code) = Stream::parse_language_code(lang) {
                        let mut state = self.state.lock().unwrap();

                        if !state.streams.is_empty()
                            && state
                                .stream_from_pad(aggregator_pad)
                                .is_some_and(|s| s.language_code != Some(language_code))
                            && self.header_update_allowed("language code")
                        {
                            if tag.scope() == gst::TagScope::Global {
                                gst::info!(
                                    CAT,
                                    obj = aggregator_pad,
                                    "Language tags scoped 'global' are considered stream tags",
                                );
                            }

                            state.need_new_header = true;
                            let stream = state.mut_stream_from_pad(aggregator_pad).unwrap();
                            stream.tag_changed = true;
                            stream.language_code = Some(language_code);
                        }
                    }
                }
                if let Some(tag_value) = tag.get::<gst::tags::ImageOrientation>() {
                    let orientation = tag_value.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received image orientation from tags: {:?}",
                        orientation
                    );

                    let mut state = self.state.lock().unwrap();
                    let orientation = TransformMatrix::from_tag(self, ev);

                    if !state.streams.is_empty()
                        && state.stream_from_pad(aggregator_pad).is_some_and(|s| {
                            if tag.scope() == gst::TagScope::Stream {
                                s.stream_orientation != Some(orientation)
                            } else {
                                s.global_orientation != orientation
                            }
                        })
                        && self.header_update_allowed("orientation")
                    {
                        state.need_new_header = true;
                        let stream = state.mut_stream_from_pad(aggregator_pad).unwrap();
                        if tag.scope() == gst::TagScope::Stream {
                            stream.stream_orientation = Some(orientation);
                        } else {
                            stream.global_orientation = orientation;
                        }
                        stream.tag_changed = true;
                    }
                }

                self.parent_sink_event(aggregator_pad, event)
            }
            gst::EventView::CustomDownstream(ev) => {
                if let Some(split_now) = SplitNowEvent::try_parse(ev) {
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received split-now event {split_now:?}",
                    );

                    let settings = self.settings.lock().unwrap().clone();

                    if settings.manual_split {
                        let mut state = self.state.lock().unwrap();
                        if let Some(stream) = state.mut_stream_from_pad(aggregator_pad) {
                            match split_now {
                                Err(err) => {
                                    gst::warning!(
                                        CAT,
                                        obj = aggregator_pad,
                                        "Invalid split-now event received: {err}",
                                    );
                                }
                                Ok(split_now) => {
                                    stream.pending_split_now.push(split_now);
                                }
                            }
                        } else {
                            gst::warning!(CAT, obj = aggregator_pad, "No stream created for pad");
                        }
                    } else {
                        gst::warning!(
                            CAT,
                            obj = aggregator_pad,
                            "split-now events only have an effect in manual-split mode",
                        );
                    }
                    true
                } else {
                    self.parent_sink_event(aggregator_pad, event)
                }
            }
            _ => self.parent_sink_event(aggregator_pad, event),
        }
    }

    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::trace!(CAT, imp = self, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Seeking(q) => {
                // We can't really handle seeking, it would break everything
                q.set(false, gst::ClockTime::ZERO, gst::ClockTime::NONE);
                true
            }
            _ => self.parent_src_query(query),
        }
    }

    fn src_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::trace!(CAT, imp = self, "Handling event {:?}", event);

        match event.view() {
            EventView::Seek(_ev) => false,
            _ => self.parent_src_event(event),
        }
    }

    fn flush(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Flush");
        self.state.lock().unwrap().flush();
        self.parent_flush()
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::trace!(CAT, imp = self, "Stopping");

        let _ = self.parent_stop();

        *self.state.lock().unwrap() = State::default();

        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::trace!(CAT, imp = self, "Starting");

        for pad in self.obj().sink_pads() {
            let pad = pad
                .downcast_ref::<crate::isobmff::FMP4MuxPad>()
                .unwrap()
                .imp();

            pad.state.lock().unwrap().trak_timescale = pad.settings.lock().unwrap().trak_timescale;
        }

        self.parent_start()?;

        // For non-single-stream variants configure a default segment that allows for negative
        // DTS so that we can correctly re-timestamp buffers with their running times.
        let aggregator = self.obj();
        let class = aggregator.class();
        if !class.as_ref().variant.is_single_stream() {
            let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
            segment.set_start(SEGMENT_OFFSET);
            segment.set_position(SEGMENT_OFFSET);
            aggregator.update_segment(&segment);
        }

        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();
        *state = State {
            sequence_number: settings.start_fragment_sequence_number,
            ..Default::default()
        };
        drop(settings);
        drop(state);

        Ok(())
    }

    fn negotiate(&self) -> bool {
        true
    }

    fn aggregate(&self, timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();

        gst::trace!(CAT, imp = self, "Aggregate (timeout: {timeout})");

        let all_eos;
        let mut caps = None;
        let mut buffers = vec![];
        let mut upstream_events = vec![];
        let (need_new_header, res) = {
            let mut state = self.state.lock().unwrap();

            // Create streams
            if state.streams.is_empty() {
                self.create_streams(&settings, &mut state)?;
            }

            self.calculate_fragment_end_pts(&settings, &mut state);

            self.queue_available_buffers(&mut state, &settings, timeout)?;

            all_eos = state.streams.iter().all(|stream| stream.sinkpad.is_eos());
            if all_eos {
                gst::debug!(CAT, imp = self, "All streams are EOS now");

                let fragment_start_pts = state.fragment_start_pts;
                let fragment_end_pts = state.fragment_end_pts;
                let chunk_start_pts = state.chunk_start_pts;

                for stream in &mut state.streams {
                    // Check if this stream is filled enough now that everything is EOS.
                    self.check_stream_filled(
                        &settings,
                        stream,
                        fragment_start_pts,
                        fragment_end_pts,
                        chunk_start_pts,
                    );
                }
            }

            // Calculate the earliest PTS, i.e. the start of the first fragment, if not known yet.
            self.calculate_earliest_pts(
                &settings,
                &mut state,
                &mut upstream_events,
                all_eos,
                timeout,
            );

            // Drain everything that can be drained at this point
            let res = self.drain(
                &mut state,
                &settings,
                all_eos,
                timeout,
                &mut caps,
                &mut buffers,
                &mut upstream_events,
            );

            if state.need_new_header {
                for stream in state
                    .streams
                    .iter()
                    .filter(|s| s.next_caps.is_some() || s.pushed_incomplete_gop)
                {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Incomplete GOP pushed or caps change - send force-key-unit event"
                    );
                    self.request_force_keyunit_event(
                        &settings,
                        stream,
                        state.fragment_start_pts,
                        &mut upstream_events,
                    );
                }
            }

            (state.need_new_header, res)
        };

        for (sinkpad, event) in upstream_events {
            sinkpad.push_event(event);
        }

        if let Some(caps) = caps {
            gst::debug!(CAT, imp = self, "Setting caps on source pad: {:?}", caps);
            self.obj().set_src_caps(&caps);
        }

        for buffer_list in buffers {
            gst::trace!(CAT, imp = self, "Pushing buffer list {:?}", buffer_list);
            self.obj().finish_buffer_list(buffer_list)?;
        }

        // all drained then check if a new header is needed
        if need_new_header {
            let mut state = self.state.lock().unwrap();
            gst::info!(
                CAT,
                imp = self,
                "Reset state, update stream caps and send new header"
            );
            state.need_new_header = false;
            state.stream_header = None;
            state.sent_headers = false;
            for stream in state
                .streams
                .iter_mut()
                .filter(|s| s.tag_changed || s.next_caps.is_some())
            {
                stream.tag_changed = false;
                if let Some(caps) = stream.next_caps.take() {
                    stream.caps = caps;
                }
            }
        }

        // If an error happened above while draining, return this now after pushing
        // any output that was produced before the error.
        res?;

        if !all_eos {
            return Ok(gst::FlowSuccess::Ok);
        }

        // Finish the stream.
        self.finish(&settings);

        Err(gst::FlowError::Eos)
    }
}

#[repr(C)]
pub(crate) struct Class {
    parent: gst_base::ffi::GstAggregatorClass,
    variant: Variant,
}

unsafe impl ClassStruct for Class {
    type Type = FMP4Mux;
}

impl std::ops::Deref for Class {
    type Target = glib::Class<gst_base::Aggregator>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(&self.parent as *const _ as *const _) }
    }
}

unsafe impl<T: FMP4MuxImpl> IsSubclassable<T> for crate::isobmff::FMP4Mux {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let class = class.as_mut();
        class.variant = T::VARIANT;
    }
}

pub(crate) trait FMP4MuxImpl:
    AggregatorImpl + ObjectSubclass<Type: IsA<crate::isobmff::FMP4Mux>>
{
    const VARIANT: Variant;
}

#[derive(Default)]
pub(crate) struct ISOFMP4Mux;

#[glib::object_subclass]
impl ObjectSubclass for ISOFMP4Mux {
    const NAME: &'static str = "GstISOFMP4Mux";
    type Type = crate::isobmff::ISOFMP4Mux;
    type ParentType = crate::isobmff::FMP4Mux;
}

impl ObjectImpl for ISOFMP4Mux {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("offset-to-zero")
                .nick("Offset to Zero")
                .blurb("Offsets all streams so that the earliest stream starts at 0")
                .mutable_ready()
                .build()]
        });

        &PROPERTIES
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let obj = self.obj();
        let fmp4mux = obj.upcast_ref::<crate::isobmff::FMP4Mux>().imp();

        match pspec.name() {
            "offset-to-zero" => {
                let settings = fmp4mux.settings.lock().unwrap();
                settings.offset_to_zero.to_value()
            }

            _ => unimplemented!(),
        }
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let obj = self.obj();
        let fmp4mux = obj.upcast_ref::<crate::isobmff::FMP4Mux>().imp();

        match pspec.name() {
            "offset-to-zero" => {
                let mut settings = fmp4mux.settings.lock().unwrap();
                settings.offset_to_zero = value.get().expect("type checked upstream");
            }

            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for ISOFMP4Mux {}

impl ElementImpl for ISOFMP4Mux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ISOFMP4Mux",
                "Codec/Muxer",
                "ISO fragmented MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "iso-fragmented")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp8")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp9")
                        .field("profile", gst::List::new(["0", "1", "2", "3"]))
                        .field("chroma-format", gst::List::new(["4:2:0", "4:2:2", "4:4:4"]))
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-av1")
                        .field("stream-format", "obu-stream")
                        .field("alignment", "tu")
                        .field("profile", gst::List::new(["main", "high", "professional"]))
                        .field(
                            "chroma-format",
                            gst::List::new(["4:0:0", "4:2:0", "4:2:2", "4:4:4"]),
                        )
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // formats that do not use subsampling
                            // Plus NV12 and NV21 because that works OK with the interleaved planes
                            gst::List::new([
                                "IYU2",
                                "RGB",
                                "BGR",
                                "NV12",
                                "NV21",
                                "RGBA",
                                "ARGB",
                                "ABGR",
                                "BGRA",
                                "RGBx",
                                "BGRx",
                                "Y444",
                                "AYUV",
                                "GRAY8",
                                "GRAY16_BE",
                                "GBR",
                                "RGBP",
                                "BGRP",
                                "v308",
                                "r210",
                            ]),
                        )
                        .field("width", gst::IntRange::new(1, i32::MAX))
                        .field("height", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // Formats that use horizontal subsampling, but not vertical subsampling (4:2:2 and 4:1:1)
                            gst::List::new(["Y41B", "NV16", "NV61", "Y42B"]),
                        )
                        .field(
                            "width",
                            gst::IntRange::with_step(4, i32::MAX.prev_multiple_of(&4), 4),
                        )
                        .field("height", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // Formats that use both horizontal and vertical subsampling (4:2:0)
                            gst::List::new(["I420", "YV12", "YUY2", "YVYU", "UYVY", "VYUY"]),
                        )
                        .field(
                            "width",
                            gst::IntRange::with_step(4, i32::MAX.prev_multiple_of(&4), 4),
                        )
                        .field(
                            "height",
                            gst::IntRange::with_step(2, i32::MAX.prev_multiple_of(&2), 2),
                        )
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-opus")
                        .field("channel-mapping-family", gst::IntRange::new(0i32, 255))
                        .field("channels", gst::IntRange::new(1i32, 8))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-flac")
                        .field("framed", true)
                        .field("channels", gst::IntRange::<i32>::new(1, 8))
                        .field("rate", gst::IntRange::<i32>::new(1, 10 * u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/x-ac3")
                        .field("framed", true)
                        .field("alignment", "frame")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-eac3")
                        .field("framed", true)
                        .field("alignment", "iec61937")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::FMP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for ISOFMP4Mux {}

impl FMP4MuxImpl for ISOFMP4Mux {
    const VARIANT: Variant = Variant::FragmentedISO;
}

#[derive(Default)]
pub(crate) struct CMAFMux;

#[glib::object_subclass]
impl ObjectSubclass for CMAFMux {
    const NAME: &'static str = "GstCMAFMux";
    type Type = crate::isobmff::CMAFMux;
    type ParentType = crate::isobmff::FMP4Mux;
}

impl ObjectImpl for CMAFMux {}

impl GstObjectImpl for CMAFMux {}

impl ElementImpl for CMAFMux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CMAFMux",
                "Codec/Muxer",
                "CMAF fragmented MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "cmaf")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-av1")
                        .field("stream-format", "obu-stream")
                        .field("alignment", "tu")
                        .field("profile", gst::List::new(["main", "high", "professional"]))
                        .field(
                            "chroma-format",
                            gst::List::new(["4:0:0", "4:2:0", "4:2:2", "4:4:4"]),
                        )
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-opus")
                        .field("channel-mapping-family", gst::IntRange::new(0i32, 255))
                        .field("channels", gst::IntRange::new(1i32, 8))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-eac3")
                        .field("framed", true)
                        .field("alignment", "iec61937")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::FMP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for CMAFMux {}

impl FMP4MuxImpl for CMAFMux {
    const VARIANT: Variant = Variant::CMAF;
}

#[derive(Default)]
pub(crate) struct DASHMP4Mux;

#[glib::object_subclass]
impl ObjectSubclass for DASHMP4Mux {
    const NAME: &'static str = "GstDASHMP4Mux";
    type Type = crate::isobmff::DASHMP4Mux;
    type ParentType = crate::isobmff::FMP4Mux;
}

impl ObjectImpl for DASHMP4Mux {}

impl GstObjectImpl for DASHMP4Mux {}

impl ElementImpl for DASHMP4Mux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "DASHMP4Mux",
                "Codec/Muxer",
                "DASH fragmented MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "iso-fragmented")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp8")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp9")
                        .field("profile", gst::List::new(["0", "1", "2", "3"]))
                        .field("chroma-format", gst::List::new(["4:2:0", "4:2:2", "4:4:4"]))
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-av1")
                        .field("stream-format", "obu-stream")
                        .field("alignment", "tu")
                        .field("profile", gst::List::new(["main", "high", "professional"]))
                        .field(
                            "chroma-format",
                            gst::List::new(["4:0:0", "4:2:0", "4:2:2", "4:4:4"]),
                        )
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-opus")
                        .field("channel-mapping-family", gst::IntRange::new(0i32, 255))
                        .field("channels", gst::IntRange::new(1i32, 8))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-ac3")
                        .field("framed", true)
                        .field("alignment", "frame")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-eac3")
                        .field("framed", true)
                        .field("alignment", "iec61937")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::FMP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for DASHMP4Mux {}

impl FMP4MuxImpl for DASHMP4Mux {
    const VARIANT: Variant = Variant::DASH;
}

#[derive(Default)]
pub(crate) struct ONVIFFMP4Mux;

#[glib::object_subclass]
impl ObjectSubclass for ONVIFFMP4Mux {
    const NAME: &'static str = "GstONVIFFMP4Mux";
    type Type = crate::isobmff::ONVIFFMP4Mux;
    type ParentType = crate::isobmff::FMP4Mux;
}

impl ObjectImpl for ONVIFFMP4Mux {}

impl GstObjectImpl for ONVIFFMP4Mux {}

impl ElementImpl for ONVIFFMP4Mux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIFFMP4Mux",
                "Codec/Muxer",
                "ONVIF fragmented MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "iso-fragmented")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("image/jpeg")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-alaw")
                        .field("channels", gst::IntRange::<i32>::new(1, 2))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-mulaw")
                        .field("channels", gst::IntRange::<i32>::new(1, 2))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-adpcm")
                        .field("layout", "g726")
                        .field("channels", 1i32)
                        .field("rate", 8000i32)
                        .field("bitrate", gst::List::new([16000i32, 24000, 32000, 40000]))
                        .build(),
                    gst::Structure::builder("application/x-onvif-metadata")
                        .field("parsed", true)
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::FMP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for ONVIFFMP4Mux {}

impl FMP4MuxImpl for ONVIFFMP4Mux {
    const VARIANT: Variant = Variant::FragmentedONVIF;
}

#[derive(Default, Clone)]
struct PadSettings {
    trak_timescale: u32,
}

#[derive(Default, Clone)]
struct PadState {
    // the selected trak_timescale
    trak_timescale: u32,
}

#[derive(Default)]
pub(crate) struct FMP4MuxPad {
    settings: Mutex<PadSettings>,
    state: Mutex<PadState>,
}

#[glib::object_subclass]
impl ObjectSubclass for FMP4MuxPad {
    const NAME: &'static str = "GstFMP4MuxPad";
    type Type = crate::isobmff::FMP4MuxPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for FMP4MuxPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecUInt::builder("trak-timescale")
                .nick("Track Timescale")
                .blurb("Timescale to use for the track (units per second, 0 is automatic)")
                .mutable_ready()
                .build()]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "trak-timescale" => {
                let mut settings = self.settings.lock().unwrap();
                settings.trak_timescale = value.get().expect("type checked upstream");
            }

            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "trak-timescale" => {
                let settings = self.settings.lock().unwrap();
                settings.trak_timescale.to_value()
            }

            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for FMP4MuxPad {}

impl PadImpl for FMP4MuxPad {}

impl AggregatorPadImpl for FMP4MuxPad {
    fn flush(&self, aggregator: &gst_base::Aggregator) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mux = aggregator
            .downcast_ref::<crate::isobmff::FMP4Mux>()
            .unwrap();
        let mut mux_state = mux.imp().state.lock().unwrap();

        if let Some(stream) =
            mux_state.mut_stream_from_pad(self.obj().upcast_ref::<gst_base::AggregatorPad>())
        {
            stream.flush();
        }

        drop(mux_state);

        self.parent_flush(aggregator)
    }
}

impl ChildProxyImpl for FMP4Mux {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_sink_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .sink_pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}
