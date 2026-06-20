// Copyright (C) 2017 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-togglerecord:
 *
 * {{ utils/togglerecord/README.md[2:30] }}
 *
 */
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use parking_lot::{Condvar, Mutex};
use std::cmp;
use std::collections::HashMap;
use std::iter;
use std::sync::Arc;
use std::sync::LazyLock;

const DEFAULT_RECORD: bool = false;
const DEFAULT_LIVE: bool = false;

// Mutex order:
// - self.state (used with self.main_stream_cond)
// - self.main_stream.state
// - stream.state with stream coming from either self.state.pads or self.state.other_streams
// - self.settings

#[derive(Debug)]
struct Settings {
    record: bool,
    live: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            record: DEFAULT_RECORD,
            live: DEFAULT_LIVE,
        }
    }
}

#[derive(Clone, Debug)]
struct Stream {
    sinkpad: gst::Pad,
    srcpad: gst::Pad,
    state: Arc<Mutex<StreamState>>,
}

impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.sinkpad == other.sinkpad && self.srcpad == other.srcpad
    }
}

impl Eq for Stream {}

impl Stream {
    fn new(sinkpad: gst::Pad, srcpad: gst::Pad) -> Self {
        Self {
            sinkpad,
            srcpad,
            state: Arc::new(Mutex::new(StreamState::default())),
        }
    }
}

#[derive(Debug)]
struct StreamState {
    in_segment: gst::FormattedSegment<gst::ClockTime>,
    out_segment: gst::FormattedSegment<gst::ClockTime>,
    segment_seqnum: gst::Seqnum,
    // Start/end running time of the current/last buffer
    current_running_time: Option<gst::ClockTime>,
    current_running_time_end: Option<gst::ClockTime>,
    eos: bool,
    flushing: bool,
    segment_pending: bool,
    discont_pending: bool,
    upstream_live: Option<bool>,
    pending_events: Vec<gst::Event>,
    audio_info: Option<gst_audio::AudioInfo>,
    video_info: Option<gst_video::VideoInfo>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            in_segment: gst::FormattedSegment::new(),
            out_segment: gst::FormattedSegment::new(),
            segment_seqnum: gst::Seqnum::next(),
            current_running_time: None,
            current_running_time_end: None,
            eos: false,
            flushing: false,
            segment_pending: false,
            discont_pending: true,
            upstream_live: None,
            pending_events: Vec::new(),
            audio_info: None,
            video_info: None,
        }
    }
}

// Recording behaviour:
//
// Secondary streams are *always* behind main stream
// Main stream EOS stops recording (-> Stopping), makes secondary streams go EOS
//
// Recording: Passing through all data
// Stopping: Main stream remembering current last_recording_stop, waiting for all
//           other streams to reach this position
// Stopped: Dropping (live input) or blocking (non-live input) all data
// Starting: Main stream waiting until next keyframe and setting last_recording_start, waiting
//           for all other streams to reach this position
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordingState {
    Recording,
    Stopping,
    Stopped,
    Starting,
}

#[derive(Debug)]
struct State {
    other_streams: Vec<Stream>,
    next_pad_id: u32,
    pads: HashMap<gst::Pad, Stream>,

    recording_state: RecordingState,
    last_recording_start: Option<gst::ClockTime>,
    last_recording_stop: Option<gst::ClockTime>,

    // Accumulated duration of previous recording segments,
    // updated whenever going to Stopped
    recording_duration: gst::ClockTime,

    // Accumulated duration of blocked segments
    blocked_duration: gst::ClockTime,

    // What time we started blocking
    time_start_block: Option<gst::ClockTime>,

    // Updated whenever going to Recording
    running_time_offset: i64,

    // Copied from settings
    live: bool,
}

impl State {
    fn new(pads: HashMap<gst::Pad, Stream>) -> Self {
        Self {
            other_streams: Vec::new(),
            next_pad_id: 0,
            pads,
            recording_state: RecordingState::Stopped,
            last_recording_start: None,
            last_recording_stop: None,
            recording_duration: gst::ClockTime::ZERO,
            blocked_duration: gst::ClockTime::ZERO,
            time_start_block: gst::ClockTime::NONE,
            running_time_offset: 0,
            live: false,
        }
    }

    fn reset(&mut self) {
        self.recording_state = RecordingState::Stopped;
        self.last_recording_start = None;
        self.last_recording_stop = None;
        self.recording_duration = gst::ClockTime::ZERO;
        self.blocked_duration = gst::ClockTime::ZERO;
        self.time_start_block = gst::ClockTime::NONE;
        self.running_time_offset = 0;
        self.live = false;
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HandleResult<T> {
    Pass(T),
    Drop,
    Eos(bool),
}

trait HandleData: Sized {
    fn pts(&self) -> Option<gst::ClockTime>;
    fn dts(&self) -> Option<gst::ClockTime>;
    fn dts_or_pts(&self) -> Option<gst::ClockTime> {
        let dts = self.dts();
        if dts.is_some() {
            dts
        } else {
            self.pts()
        }
    }
    fn duration(&self, state: &StreamState) -> Option<gst::ClockTime>;
    fn is_keyframe(&self) -> bool;
    fn can_clip(&self, state: &StreamState) -> bool;
    fn clip(
        self,
        state: &StreamState,
        segment: &gst::FormattedSegment<gst::ClockTime>,
    ) -> Option<Self>;
}

impl HandleData for (gst::ClockTime, Option<gst::ClockTime>) {
    fn pts(&self) -> Option<gst::ClockTime> {
        Some(self.0)
    }

    fn dts(&self) -> Option<gst::ClockTime> {
        Some(self.0)
    }

    fn duration(&self, _state: &StreamState) -> Option<gst::ClockTime> {
        self.1
    }

    fn is_keyframe(&self) -> bool {
        true
    }

    fn can_clip(&self, _state: &StreamState) -> bool {
        true
    }

    fn clip(
        self,
        _state: &StreamState,
        segment: &gst::FormattedSegment<gst::ClockTime>,
    ) -> Option<Self> {
        let stop = self.0 + self.1.unwrap_or(gst::ClockTime::ZERO);

        segment.clip(self.0, stop).map(|(start, stop)| {
            let start = start.expect("provided a defined value");
            (start, stop.opt_sub(start))
        })
    }
}

impl HandleData for gst::Buffer {
    fn pts(&self) -> Option<gst::ClockTime> {
        gst::BufferRef::pts(self)
    }

    fn dts(&self) -> Option<gst::ClockTime> {
        gst::BufferRef::dts(self)
    }

    fn duration(&self, state: &StreamState) -> Option<gst::ClockTime> {
        let duration = gst::BufferRef::duration(self);

        if duration.is_some() {
            duration
        } else if let Some(ref video_info) = state.video_info {
            if video_info.fps() != 0.into() {
                gst::ClockTime::SECOND.mul_div_floor(
                    video_info.fps().denom() as u64,
                    video_info.fps().numer() as u64,
                )
            } else {
                gst::ClockTime::NONE
            }
        } else if let Some(ref audio_info) = state.audio_info {
            if audio_info.bpf() == 0 || audio_info.rate() == 0 {
                return gst::ClockTime::NONE;
            }

            let size = self.size() as u64;
            let num_samples = size / audio_info.bpf() as u64;
            gst::ClockTime::SECOND.mul_div_floor(num_samples, audio_info.rate() as u64)
        } else {
            gst::ClockTime::NONE
        }
    }

    fn is_keyframe(&self) -> bool {
        !gst::BufferRef::flags(self).contains(gst::BufferFlags::DELTA_UNIT)
    }

    fn can_clip(&self, state: &StreamState) -> bool {
        // Only do actual clipping for raw audio/video
        if let Some(ref audio_info) = state.audio_info {
            if audio_info.format() == gst_audio::AudioFormat::Unknown
                || audio_info.format() == gst_audio::AudioFormat::Encoded
                || audio_info.rate() == 0
                || audio_info.bpf() == 0
            {
                return false;
            }
        } else if let Some(ref video_info) = state.video_info {
            if video_info.format() == gst_video::VideoFormat::Unknown
                || video_info.format() == gst_video::VideoFormat::Encoded
                || self.dts_or_pts() != self.pts()
            {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    fn clip(
        mut self,
        state: &StreamState,
        segment: &gst::FormattedSegment<gst::ClockTime>,
    ) -> Option<Self> {
        // Only do actual clipping for raw audio/video
        if !self.can_clip(state) {
            return Some(self);
        }

        let pts = HandleData::pts(&self);
        let duration = HandleData::duration(&self, state);
        let stop = pts.map(|pts| pts + duration.unwrap_or(gst::ClockTime::ZERO));

        if let Some(ref audio_info) = state.audio_info {
            gst_audio::audio_buffer_clip(
                self,
                segment.upcast_ref(),
                audio_info.rate(),
                audio_info.bpf(),
            )
        } else if state.video_info.is_some() {
            segment.clip(pts, stop).map(move |(start, stop)| {
                {
                    let buffer = self.make_mut();
                    buffer.set_pts(start);
                    buffer.set_duration(
                        stop.opt_checked_sub(start).ok().flatten(), // FIXME we could expect here
                    );
                }

                self
            })
        } else {
            unreachable!();
        }
    }
}

pub struct ToggleRecord {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    main_stream: Stream,
    // Always must have main_stream.state locked!
    // If multiple stream states have to be locked, the
    // main_stream always comes first
    main_stream_cond: Condvar,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "togglerecord",
        gst::DebugColorFlags::empty(),
        Some("Toggle Record Element"),
    )
});

impl ToggleRecord {
    // called without lock
    fn block_if_upstream_not_live(
        &self,
        pad: &gst::Pad,
        stream: &Stream, // main stream
        upstream_live: bool,
    ) -> Result<bool, gst::FlowError> {
        if !upstream_live {
            let clock = self.obj().clock();
            let mut rec_state = self.state.lock();
            let mut state = stream.state.lock();
            let mut settings = self.settings.lock();

            if rec_state.time_start_block.is_none() {
                rec_state.time_start_block = clock.as_ref().map(gst::Clock::time);
            }
            while !settings.record && !state.flushing {
                gst::debug!(CAT, obj = pad, "Waiting for record=true");
                drop(state);
                drop(settings);
                self.main_stream_cond.wait(&mut rec_state);
                state = stream.state.lock();
                settings = self.settings.lock();
            }
            if state.flushing {
                gst::debug!(CAT, obj = pad, "Flushing");
                return Err(gst::FlowError::Flushing);
            }
            state.segment_pending = true;
            state.discont_pending = true;
            for other_stream in &rec_state.other_streams {
                // safe from deadlock as `state` is a lock on the main stream which is not in `other_streams`
                let mut other_state = other_stream.state.lock();
                other_state.segment_pending = true;
                other_state.discont_pending = true;
            }
            if let Some(time_start_block) = rec_state.time_start_block {
                // If we have a time_start_block it means the clock is there
                let clock = clock.expect("Cannot find pipeline clock");
                rec_state.blocked_duration += clock.time() - time_start_block;
                if settings.live {
                    rec_state.running_time_offset = rec_state.blocked_duration.nseconds() as i64;
                }
                rec_state.time_start_block = gst::ClockTime::NONE;
            } else {
                // How did we even get here?
                gst::warning!(
                    CAT,
                    obj = pad,
                    "Have no clock and no current running time. Will not offset buffers"
                );
            }
            drop(rec_state);
            gst::log!(CAT, obj = pad, "Done blocking main stream");
            Ok(true)
        } else {
            gst::log!(CAT, obj = pad, "Dropping buffer (stopped)");
            Ok(false)
        }
    }

    // called without lock
    fn handle_main_stream<T: HandleData>(
        &self,
        pad: &gst::Pad,
        stream: &Stream,
        data: T,
        upstream_live: bool,
    ) -> Result<HandleResult<T>, gst::FlowError> {
        let mut rec_state = self.state.lock();
        let mut state = stream.state.lock();

        let data = match data.clip(&state, &state.in_segment) {
            Some(data) => data,
            None => {
                gst::log!(CAT, obj = pad, "Dropping raw data outside segment");
                return Ok(HandleResult::Drop);
            }
        };

        let mut dts_or_pts = data.dts_or_pts().ok_or_else(|| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Buffer without DTS or PTS"]
            );
            gst::FlowError::Error
        })?;

        let mut dts_or_pts_end = dts_or_pts + data.duration(&state).unwrap_or(gst::ClockTime::ZERO);

        // This will only do anything for non-raw data
        dts_or_pts = state.in_segment.start().unwrap().max(dts_or_pts);
        dts_or_pts_end = state.in_segment.start().unwrap().max(dts_or_pts_end);
        if let Some(stop) = state.in_segment.stop() {
            dts_or_pts = stop.min(dts_or_pts);
            dts_or_pts_end = stop.min(dts_or_pts_end);
        }

        let current_running_time = state.in_segment.to_running_time(dts_or_pts);
        let current_running_time_end = state.in_segment.to_running_time(dts_or_pts_end);

        state.current_running_time = current_running_time
            .opt_max(state.current_running_time)
            .or(current_running_time);
        state.current_running_time_end = current_running_time_end
            .opt_max(state.current_running_time_end)
            .or(current_running_time_end);

        // Wake up everybody, we advanced a bit
        // Important: They will only be able to advance once we're done with this
        // function or waiting for them to catch up below, otherwise they might
        // get the wrong state
        self.main_stream_cond.notify_all();

        gst::log!(
            CAT,
            obj = pad,
            "Main stream current running time {}-{} (position: {}-{})",
            current_running_time.display(),
            current_running_time_end.display(),
            dts_or_pts,
            dts_or_pts_end,
        );

        let settings = self.settings.lock();

        // First check if we need to block for non-live input

        // Check if we have to update our recording state
        let settings_changed = match rec_state.recording_state {
            RecordingState::Recording if !settings.record => {
                let clock = self.obj().clock().expect("Cannot find pipeline clock");
                rec_state.time_start_block = Some(clock.time());
                gst::debug!(CAT, obj = pad, "Stopping recording");
                rec_state.recording_state = RecordingState::Stopping;
                true
            }
            RecordingState::Stopped if settings.record => {
                gst::debug!(CAT, obj = pad, "Starting recording");
                rec_state.recording_state = RecordingState::Starting;
                true
            }
            _ => false,
        };
        drop(settings);

        match rec_state.recording_state {
            RecordingState::Recording => {
                // Remember where we stopped last, in case of EOS
                rec_state.last_recording_stop = current_running_time_end;
                gst::log!(CAT, obj = pad, "Passing buffer (recording)");
                Ok(HandleResult::Pass(data))
            }
            RecordingState::Stopping => {
                if !data.is_keyframe() {
                    // Remember where we stopped last, in case of EOS
                    rec_state.last_recording_stop = current_running_time_end;
                    gst::log!(CAT, obj = pad, "Passing non-keyframe buffer (stopping)");

                    drop(rec_state);
                    drop(state);
                    if settings_changed {
                        gst::debug!(CAT, obj = pad, "Requesting a new keyframe");
                        stream
                            .sinkpad
                            .push_event(gst_video::UpstreamForceKeyUnitEvent::builder().build());
                    }

                    return Ok(HandleResult::Pass(data));
                }

                // Remember the time when we stopped: now, i.e. right before the current buffer!
                rec_state.last_recording_stop = current_running_time;
                let last_recording_duration = rec_state
                    .last_recording_stop
                    .opt_checked_sub(rec_state.last_recording_start)
                    .ok()
                    .flatten();
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Stopping at {}, started at {}, current duration {}, previous accumulated recording duration {}",
                    rec_state.last_recording_stop.display(),
                    rec_state.last_recording_start.display(),
                    last_recording_duration.display(),
                    rec_state.recording_duration
                );

                // Then unlock and wait for all other streams to reach a buffer that is completely
                // after/at the recording stop position (i.e. can be dropped completely) or go EOS
                // instead.

                while !state.flushing
                    && !rec_state.other_streams.iter().all(|s| {
                        // safe from deadlock as `state` is a lock on the main stream which is not in `other_streams`
                        let s = s.state.lock();
                        s.eos
                            || s.current_running_time
                                .opt_ge(current_running_time)
                                .unwrap_or(false)
                    })
                {
                    gst::log!(CAT, obj = pad, "Waiting for other streams to stop");
                    drop(state);
                    self.main_stream_cond.wait(&mut rec_state);
                    state = stream.state.lock();
                }

                if state.flushing {
                    gst::debug!(CAT, obj = pad, "Flushing");
                    return Err(gst::FlowError::Flushing);
                }

                rec_state.recording_state = RecordingState::Stopped;
                rec_state.recording_duration +=
                    last_recording_duration.unwrap_or(gst::ClockTime::ZERO);
                rec_state.last_recording_start = None;
                rec_state.last_recording_stop = None;

                gst::debug!(
                    CAT,
                    obj = pad,
                    "Stopped at {}, recording duration {}",
                    current_running_time.display(),
                    rec_state.recording_duration.display(),
                );

                // Then become Stopped and drop this buffer. We always stop right before
                // a keyframe
                drop(state);
                drop(rec_state);

                let ret = self.block_if_upstream_not_live(pad, stream, upstream_live)?;
                self.obj().notify("recording");

                if ret {
                    Ok(HandleResult::Pass(data))
                } else {
                    Ok(HandleResult::Drop)
                }
            }
            RecordingState::Stopped => {
                if !upstream_live {
                    rec_state.recording_state = RecordingState::Starting;
                }
                drop(rec_state);
                drop(state);
                if self.block_if_upstream_not_live(pad, stream, upstream_live)? {
                    Ok(HandleResult::Pass(data))
                } else {
                    Ok(HandleResult::Drop)
                }
            }
            RecordingState::Starting => {
                // If this is no keyframe, we can directly go out again here and drop the frame
                if !data.is_keyframe() {
                    gst::log!(CAT, obj = pad, "Dropping non-keyframe buffer (starting)");

                    drop(rec_state);
                    drop(state);
                    if settings_changed {
                        gst::debug!(CAT, obj = pad, "Requesting a new keyframe");
                        stream
                            .sinkpad
                            .push_event(gst_video::UpstreamForceKeyUnitEvent::builder().build());
                    }

                    if !upstream_live {
                        gst::log!(
                            CAT,
                            obj = pad,
                            "Always passing data when upstream is not live"
                        );
                        return Ok(HandleResult::Pass(data));
                    }
                    return Ok(HandleResult::Drop);
                }

                // Remember the time when we started: now!
                rec_state.last_recording_start = current_running_time;
                // We made sure a few lines above, but let's be sure again
                let settings = self.settings.lock();
                if !settings.live || upstream_live {
                    rec_state.running_time_offset =
                        0 - current_running_time.map_or(0, |current_running_time| {
                            current_running_time
                                .saturating_sub(rec_state.recording_duration)
                                .nseconds()
                        }) as i64
                };
                drop(settings);
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Starting at {}, previous accumulated recording duration {}, offset {}",
                    current_running_time.display(),
                    rec_state.recording_duration,
                    rec_state.running_time_offset,
                );

                state.segment_pending = true;
                state.discont_pending = true;
                for other_stream in &rec_state.other_streams {
                    // safe from deadlock as `state` is a lock on the main stream which is not in `other_streams`
                    let mut other_state = other_stream.state.lock();
                    other_state.segment_pending = true;
                    other_state.discont_pending = true;
                }

                // Then unlock and wait for all other streams to reach a buffer that is completely
                // after/at the recording start position (i.e. can be passed through completely) or
                // go EOS instead.

                while !state.flushing
                    && !rec_state.other_streams.iter().all(|s| {
                        let s = s.state.lock();
                        s.eos
                            || s.current_running_time
                                .opt_ge(current_running_time)
                                .unwrap_or(false)
                    })
                {
                    gst::log!(CAT, obj = pad, "Waiting for other streams to start");
                    drop(state);
                    self.main_stream_cond.wait(&mut rec_state);
                    state = stream.state.lock();
                }

                if state.flushing {
                    gst::debug!(CAT, obj = pad, "Flushing");
                    return Err(gst::FlowError::Flushing);
                }

                rec_state.recording_state = RecordingState::Recording;
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Started at {}, recording duration {}",
                    current_running_time.display(),
                    rec_state.recording_duration
                );

                gst::log!(CAT, obj = pad, "Passing buffer (recording)");

                drop(rec_state);
                drop(state);
                self.obj().notify("recording");

                Ok(HandleResult::Pass(data))
            }
        }
    }

    #[allow(clippy::blocks_in_conditions)]
    // called without lock
    fn handle_secondary_stream<T: HandleData>(
        &self,
        pad: &gst::Pad,
        stream: &Stream,
        data: T,
        upstream_live: bool,
    ) -> Result<HandleResult<T>, gst::FlowError> {
        // Calculate end pts & current running time and make sure we stay in the segment
        let mut state = stream.state.lock();

        let mut pts = data.pts().ok_or_else(|| {
            gst::element_imp_error!(self, gst::StreamError::Format, ["Buffer without PTS"]);
            gst::FlowError::Error
        })?;

        if data.dts().is_some_and(|dts| dts != pts) {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["DTS != PTS not supported for secondary streams"]
            );
            return Err(gst::FlowError::Error);
        }

        if !data.is_keyframe() {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Delta-units not supported for secondary streams"]
            );
            return Err(gst::FlowError::Error);
        }

        let mut pts_end = pts + data.duration(&state).unwrap_or(gst::ClockTime::ZERO);

        let data = match data.clip(&state, &state.in_segment) {
            None => {
                gst::log!(CAT, obj = pad, "Dropping raw data outside segment");
                return Ok(HandleResult::Drop);
            }
            Some(data) => data,
        };

        // This will only do anything for non-raw data
        pts = state.in_segment.start().unwrap().max(pts);
        pts_end = state.in_segment.start().unwrap().max(pts_end);
        if let Some(stop) = state.in_segment.stop() {
            pts = stop.min(pts);
            pts_end = stop.min(pts_end);
        }

        let current_running_time = state.in_segment.to_running_time(pts);
        let current_running_time_end = state.in_segment.to_running_time(pts_end);
        state.current_running_time = current_running_time
            .opt_max(state.current_running_time)
            .or(current_running_time);
        state.current_running_time_end = current_running_time_end
            .opt_max(state.current_running_time_end)
            .or(current_running_time_end);

        gst::log!(
            CAT,
            obj = pad,
            "Secondary stream current running time {}-{} (position: {}-{}",
            current_running_time.display(),
            current_running_time_end.display(),
            pts,
            pts_end
        );

        drop(state);

        // Wake up, in case the main stream is waiting for us to progress up to here. We progressed
        // above but all notifying must happen while the main_stream state is locked as per above.
        self.main_stream_cond.notify_all();

        let mut rec_state = self.state.lock();
        let mut main_state = self.main_stream.state.lock();
        state = stream.state.lock();

        // Wait until the main stream advanced completely past our current running time in
        // Recording/Stopped modes to make sure we're not already outputting/dropping data that
        // should actually be dropped/output if recording is started/stopped now.
        //
        // In Starting/Stopping mode we wait if we the start of this buffer is after last recording
        // start/stop as in that case we should be in Recording/Stopped mode already. The main
        // stream is waiting for us to reach that position to switch to Recording/Stopped mode so
        // that in those modes we only have to pass through/drop the whole buffers.
        while (main_state.current_running_time.is_none()
            || rec_state.recording_state != RecordingState::Starting
                && rec_state.recording_state != RecordingState::Stopping
                && main_state
                    .current_running_time_end
                    .opt_lt(current_running_time_end)
                    .unwrap_or(false)
            || rec_state.recording_state == RecordingState::Starting
                && (rec_state.last_recording_start.is_none()
                    || rec_state
                        .last_recording_start
                        .opt_le(current_running_time)
                        .unwrap_or(false))
            || rec_state.recording_state == RecordingState::Stopping
                && (rec_state.last_recording_stop.is_none()
                    || rec_state
                        .last_recording_stop
                        .opt_le(current_running_time)
                        .unwrap_or(false)))
            && !main_state.eos
            && !state.flushing
        {
            gst::log!(
                CAT,
                obj = pad,
                "Waiting at {}-{} in {:?} state, main stream at {}-{}",
                current_running_time.display(),
                current_running_time_end.display(),
                rec_state.recording_state,
                main_state.current_running_time.display(),
                main_state.current_running_time_end.display(),
            );

            drop(main_state);
            drop(state);
            self.main_stream_cond.wait(&mut rec_state);
            main_state = self.main_stream.state.lock();
            state = stream.state.lock();
        }

        if state.flushing {
            gst::debug!(CAT, obj = pad, "Flushing");
            return Err(gst::FlowError::Flushing);
        }

        // If the main stream is EOS, we are also EOS unless we are
        // before the final last recording stop running time
        if main_state.eos {
            // If we have no start or stop position (we never recorded) then we're EOS too now
            if rec_state.last_recording_stop.is_none() || rec_state.last_recording_start.is_none() {
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and recording never started",
                );
                drop(main_state);

                return Ok(HandleResult::Eos(self.check_and_update_eos(
                    pad,
                    stream,
                    &mut state,
                    &mut rec_state,
                )));
            }

            let last_recording_start = rec_state.last_recording_start.expect("recording started");

            // FIXME it would help a lot if we could expect current_running_time
            // and possibly current_running_time_end at some point.

            if data.can_clip(&state)
                && current_running_time.is_some_and(|cur_rt| cur_rt < last_recording_start)
                && current_running_time_end
                    .is_some_and(|cur_rt_end| cur_rt_end > last_recording_start)
            {
                // Otherwise if we're before the recording start but the end of the buffer is after
                // the start and we can clip, clip the buffer and pass it onwards.
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and we're not EOS yet (overlapping recording start, {} < {} < {})",
                    current_running_time.display(),
                    last_recording_start,
                    current_running_time_end.display(),
                );

                let mut clip_start = state
                    .in_segment
                    .position_from_running_time(rec_state.last_recording_start);
                if clip_start.is_none() {
                    clip_start = state.in_segment.start();
                }
                let mut clip_stop = state
                    .in_segment
                    .position_from_running_time(rec_state.last_recording_stop);
                if clip_stop.is_none() {
                    clip_stop = state.in_segment.stop();
                }
                let mut segment = state.in_segment.clone();
                segment.set_start(clip_start);
                segment.set_stop(clip_stop);

                gst::log!(CAT, obj = pad, "Clipping to segment {:?}", segment);

                if let Some(data) = data.clip(&state, &segment) {
                    return Ok(HandleResult::Pass(data));
                } else {
                    gst::warning!(CAT, obj = pad, "Complete buffer clipped!");
                    return Ok(HandleResult::Drop);
                }
            } else if current_running_time
                .opt_lt(last_recording_start)
                .unwrap_or(false)
            {
                // Otherwise if the buffer starts before the recording start, drop it. This
                // means that we either can't clip, or that the end is also before the
                // recording start
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and we're not EOS yet (before recording start, {} < {})",
                    current_running_time.display(),
                    last_recording_start,
                );
                return Ok(HandleResult::Drop);
            } else if data.can_clip(&state)
                && current_running_time
                    .opt_lt(rec_state.last_recording_stop)
                    .unwrap_or(false)
                && current_running_time_end
                    .opt_gt(rec_state.last_recording_stop)
                    .unwrap_or(false)
            {
                // Similarly if the end is after the recording stop but the start is before and we
                // can clip, clip the buffer and pass it through.
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and we're not EOS yet (overlapping recording end, {} < {} < {})",
                    current_running_time.display(),
                    rec_state.last_recording_stop.display(),
                    current_running_time_end.display(),
                );

                let mut clip_start = state
                    .in_segment
                    .position_from_running_time(rec_state.last_recording_start);
                if clip_start.is_none() {
                    clip_start = state.in_segment.start();
                }
                let mut clip_stop = state
                    .in_segment
                    .position_from_running_time(rec_state.last_recording_stop);
                if clip_stop.is_none() {
                    clip_stop = state.in_segment.stop();
                }
                let mut segment = state.in_segment.clone();
                segment.set_start(clip_start);
                segment.set_stop(clip_stop);

                gst::log!(CAT, obj = pad, "Clipping to segment {:?}", segment,);

                if let Some(data) = data.clip(&state, &segment) {
                    return Ok(HandleResult::Pass(data));
                } else {
                    gst::warning!(CAT, obj = pad, "Complete buffer clipped!");
                    return Ok(HandleResult::Eos(self.check_and_update_eos(
                        pad,
                        stream,
                        &mut state,
                        &mut rec_state,
                    )));
                }
            } else if current_running_time_end
                .opt_gt(rec_state.last_recording_stop)
                .unwrap_or(false)
            {
                // Otherwise if the end of the buffer is after the recording stop, we're EOS
                // now. This means that we either couldn't clip or that the start is also after
                // the recording stop
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and we're EOS too (after recording end, {} > {})",
                    current_running_time_end.display(),
                    rec_state.last_recording_stop.display(),
                );
                return Ok(HandleResult::Eos(self.check_and_update_eos(
                    pad,
                    stream,
                    &mut state,
                    &mut rec_state,
                )));
            } else {
                // In all other cases the buffer is fully between recording start and end and
                // can be passed through as is
                assert!(current_running_time
                    .opt_ge(last_recording_start)
                    .unwrap_or(false));
                assert!(current_running_time_end
                    .opt_le(rec_state.last_recording_stop)
                    .unwrap_or(false));

                gst::debug!(
                    CAT,
                    obj = pad,
                    "Main stream EOS and we're not EOS yet (before recording end, {} <= {} <= {})",
                    last_recording_start,
                    current_running_time.display(),
                    rec_state.last_recording_stop.display(),
                );
                return Ok(HandleResult::Pass(data));
            }
        }

        if !upstream_live {
            return Ok(HandleResult::Pass(data));
        }

        match rec_state.recording_state {
            RecordingState::Recording => {
                // The end of our buffer must be before/at the end of the previous buffer of the main
                // stream
                assert!(current_running_time_end
                    .opt_le(main_state.current_running_time_end)
                    .unwrap_or(false));

                // We're properly started, must have a start position and
                // be actually after that start position
                if !current_running_time
                    .opt_ge(rec_state.last_recording_start)
                    .unwrap_or(false)
                {
                    panic!(
                        "current RT ({current_running_time:?}) < last_recording_start ({:?})",
                        rec_state.last_recording_start
                    );
                }
                gst::log!(CAT, obj = pad, "Passing buffer (recording)");
                Ok(HandleResult::Pass(data))
            }
            RecordingState::Stopping => {
                // If we have no start position yet, the main stream is waiting for a key-frame
                let last_recording_stop = match rec_state.last_recording_stop {
                    Some(last_recording_stop) => last_recording_stop,
                    None => {
                        gst::log!(
                            CAT,
                            obj = pad,
                            "Passing buffer (stopping: waiting for keyframe)",
                        );
                        return Ok(HandleResult::Pass(data));
                    }
                };

                // The start of our buffer must be before the last recording stop as
                // otherwise we would be in Stopped state already
                assert!(current_running_time.is_some_and(|cur_rt| cur_rt < last_recording_stop));
                let current_running_time = current_running_time.expect("checked above");

                if current_running_time_end
                    .is_some_and(|cur_rt_end| cur_rt_end <= last_recording_stop)
                {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Passing buffer (stopping: {} <= {})",
                        current_running_time_end.display(),
                        last_recording_stop,
                    );
                    Ok(HandleResult::Pass(data))
                } else if data.can_clip(&state)
                    && current_running_time < last_recording_stop
                    && current_running_time_end
                        .is_some_and(|cur_rt_end| cur_rt_end > last_recording_stop)
                {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Passing buffer (stopping: {} < {} < {})",
                        current_running_time,
                        last_recording_stop,
                        current_running_time_end.display(),
                    );

                    let mut clip_stop = state
                        .in_segment
                        .position_from_running_time(rec_state.last_recording_stop);
                    if clip_stop.is_none() {
                        clip_stop = state.in_segment.stop();
                    }
                    let mut segment = state.in_segment.clone();
                    segment.set_stop(clip_stop);

                    gst::log!(CAT, obj = pad, "Clipping to segment {:?}", segment,);

                    if let Some(data) = data.clip(&state, &segment) {
                        Ok(HandleResult::Pass(data))
                    } else {
                        gst::warning!(CAT, obj = pad, "Complete buffer clipped!");
                        Ok(HandleResult::Drop)
                    }
                } else {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Dropping buffer (stopping: {} > {})",
                        current_running_time_end.display(),
                        rec_state.last_recording_stop.display(),
                    );
                    Ok(HandleResult::Drop)
                }
            }
            RecordingState::Stopped => {
                // The end of our buffer must be before/at the end of the previous buffer of the main
                // stream
                assert!(current_running_time_end
                    .opt_le(main_state.current_running_time_end)
                    .unwrap_or(false));

                // We're properly stopped
                gst::log!(CAT, obj = pad, "Dropping buffer (stopped)");
                Ok(HandleResult::Drop)
            }
            RecordingState::Starting => {
                // If we have no start position yet, the main stream is waiting for a key-frame
                let last_recording_start = match rec_state.last_recording_start {
                    Some(last_recording_start) => last_recording_start,
                    None => {
                        gst::log!(
                            CAT,
                            obj = pad,
                            "Dropping buffer (starting: waiting for keyframe)",
                        );
                        return Ok(HandleResult::Drop);
                    }
                };

                // The start of our buffer must be before the last recording start as
                // otherwise we would be in Recording state already
                assert!(current_running_time.is_some_and(|cur_rt| cur_rt < last_recording_start));
                let current_running_time = current_running_time.expect("checked_above");

                if current_running_time >= last_recording_start {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Passing buffer (starting: {} >= {})",
                        current_running_time,
                        last_recording_start,
                    );
                    Ok(HandleResult::Pass(data))
                } else if data.can_clip(&state)
                    && current_running_time < last_recording_start
                    && current_running_time_end
                        .is_some_and(|cur_rt_end| cur_rt_end > last_recording_start)
                {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Passing buffer (starting: {} < {} < {})",
                        current_running_time,
                        last_recording_start,
                        current_running_time_end.display(),
                    );

                    let mut clip_start = state
                        .in_segment
                        .position_from_running_time(rec_state.last_recording_start);
                    if clip_start.is_none() {
                        clip_start = state.in_segment.start();
                    }
                    let mut segment = state.in_segment.clone();
                    segment.set_start(clip_start);

                    gst::log!(CAT, obj = pad, "Clipping to segment {:?}", segment);

                    if let Some(data) = data.clip(&state, &segment) {
                        Ok(HandleResult::Pass(data))
                    } else {
                        gst::warning!(CAT, obj = pad, "Complete buffer clipped!");
                        Ok(HandleResult::Drop)
                    }
                } else {
                    gst::log!(
                        CAT,
                        obj = pad,
                        "Dropping buffer (starting: {} < {})",
                        current_running_time,
                        last_recording_start,
                    );
                    Ok(HandleResult::Drop)
                }
            }
        }
    }

    // should be called only if main stream is in eos state
    // Called while holding stream.state on either the primary or a secondary stream (stream_state)
    // and self.state (rec_state).
    fn check_and_update_eos(
        &self,
        pad: &gst::Pad,
        stream: &Stream,
        stream_state: &mut StreamState,
        rec_state: &mut State,
    ) -> bool {
        stream_state.eos = true;

        // Check whether all secondary streams are in eos. If so, update recording
        // state to Stopped
        if rec_state.recording_state != RecordingState::Stopped {
            let mut all_others_eos = true;

            // Check eos state of all secondary streams
            rec_state.other_streams.iter().all(|s| {
                if s == stream {
                    return true;
                }

                let s = s.state.lock();
                if !s.eos {
                    all_others_eos = false;
                }
                all_others_eos
            });

            if all_others_eos {
                gst::debug!(
                    CAT,
                    obj = pad,
                    "All streams are in EOS state, change state to Stopped"
                );

                rec_state.recording_state = RecordingState::Stopped;
                return true;
            }
        }

        false
    }

    // should be called only if main stream stops being in eos state
    // Called while holding stream.state on either the primary or a secondary stream (stream_state)
    // and self.state (rec_state).
    fn check_and_update_stream_start(
        &self,
        pad: &gst::Pad,
        stream: &Stream,
        stream_state: &mut StreamState,
        rec_state: &mut State,
    ) -> bool {
        stream_state.eos = false;

        // Check whether no secondary streams are in eos. If so, update recording
        // state according to the record property
        if rec_state.recording_state == RecordingState::Stopped {
            let mut all_others_not_eos = false;

            // Check eos state of all secondary streams
            rec_state.other_streams.iter().any(|s| {
                if s == stream {
                    return false;
                }

                let s = s.state.lock();
                if !s.eos {
                    all_others_not_eos = true;
                }
                all_others_not_eos
            });

            if !all_others_not_eos {
                let settings = self.settings.lock();
                if settings.record {
                    gst::debug!(CAT, obj = pad, "Restarting recording after EOS");
                    rec_state.recording_state = RecordingState::Starting;
                }
            }
        }

        false
    }

    // called without lock
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let rec_state = self.state.lock();
        let stream = rec_state.pads.get(pad).cloned().ok_or_else(|| {
            gst::element_imp_error!(self, gst::CoreError::Pad, ["Unknown pad {:?}", pad.name()]);
            gst::FlowError::Error
        })?;
        drop(rec_state);

        let upstream_live;

        {
            let mut state = stream.state.lock();
            if state.eos {
                return Err(gst::FlowError::Eos);
            }
            if state.flushing {
                return Err(gst::FlowError::Flushing);
            }
            match state.upstream_live {
                None => {
                    // Not handling anything here, the pad's query function will catch it
                    let mut query = gst::query::Latency::new();
                    let success = pad.peer_query(&mut query);
                    if success {
                        (upstream_live, _, _) = query.result();
                        state.upstream_live = Some(upstream_live);
                    } else {
                        state.upstream_live = None;
                        upstream_live = false;
                        gst::warning!(
                            CAT,
                            obj = pad,
                            "Latency query failed, assuming non-live input, will retry"
                        );
                    }
                }
                Some(is_live) => upstream_live = is_live,
            }
        }

        let handle_result = if stream != self.main_stream {
            self.handle_secondary_stream(pad, &stream, buffer, upstream_live)
        } else {
            self.handle_main_stream(pad, &stream, buffer, upstream_live)
        }?;

        let mut buffer = match handle_result {
            HandleResult::Drop => {
                return Ok(gst::FlowSuccess::Ok);
            }
            HandleResult::Eos(recording_state_updated) => {
                stream.srcpad.push_event(
                    gst::event::Eos::builder()
                        .seqnum(stream.state.lock().segment_seqnum)
                        .build(),
                );

                if recording_state_updated {
                    self.obj().notify("recording");
                }

                return Err(gst::FlowError::Eos);
            }
            HandleResult::Pass(buffer) => {
                // Pass through and actually push the buffer
                buffer
            }
        };

        let out_running_time = {
            let rec_state = self.state.lock();
            let main_state = if stream != self.main_stream {
                Some(self.main_stream.state.lock())
            } else {
                None
            };

            let mut state = stream.state.lock();

            if state.discont_pending {
                gst::debug!(CAT, obj = pad, "Pending discont");
                let buffer = buffer.make_mut();
                buffer.set_flags(gst::BufferFlags::DISCONT);
                state.discont_pending = false;
            }

            let mut events = Vec::with_capacity(state.pending_events.len() + 1);

            if state.segment_pending {
                // Adjust so that last_recording_start has running time of
                // recording_duration

                state.out_segment = state.in_segment.clone();

                // state.upstream_live should have a value from a few lines above
                // segment offset is taken into account in case upstream is live and we are not
                // (collapse gap)
                if rec_state.live != upstream_live {
                    state
                        .out_segment
                        .offset_running_time(rec_state.running_time_offset)
                        .expect("Adjusting record duration");
                }
                events.push(
                    gst::event::Segment::builder(&state.out_segment)
                        .seqnum(state.segment_seqnum)
                        .build(),
                );
                state.segment_pending = false;
                gst::debug!(CAT, obj = pad, "Pending Segment {:?}", &state.out_segment);
            }

            if !state.pending_events.is_empty() {
                gst::debug!(CAT, obj = pad, "Pushing pending events");
            }

            events.append(&mut state.pending_events);

            let out_running_time = state.out_segment.to_running_time(buffer.pts());

            // Unlock before pushing
            drop(rec_state);
            drop(state);
            drop(main_state);

            for e in events.drain(..) {
                stream.srcpad.push_event(e);
            }

            out_running_time
        };

        gst::log!(
            CAT,
            obj = pad,
            "Pushing buffer with running time {}: {:?}",
            out_running_time.display(),
            buffer,
        );
        stream.srcpad.push(buffer)
    }

    // called without lock
    fn sink_event(&self, pad: &gst::Pad, mut event: gst::Event) -> bool {
        let mut rec_state = self.state.lock();
        use gst::EventView;

        let stream = match rec_state.pads.get(pad) {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Pad,
                    ["Unknown pad {:?}", pad.name()]
                );
                return false;
            }
            Some(stream) => stream.clone(),
        };

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        let mut forward = true;
        let mut send_pending = false;
        let mut recording_state_changed = false;

        match event.view() {
            EventView::FlushStart(..) => {
                let _main_state = if stream != self.main_stream {
                    Some(self.main_stream.state.lock())
                } else {
                    None
                };
                let mut state = stream.state.lock();

                state.flushing = true;
                self.main_stream_cond.notify_all();
            }
            EventView::FlushStop(..) => {
                let mut state = stream.state.lock();

                state.eos = false;
                state.flushing = false;
                state.segment_pending = true;
                state.discont_pending = true;
                state.current_running_time = None;
                state.current_running_time_end = None;
            }
            EventView::Caps(c) => {
                let mut state = stream.state.lock();
                let caps = c.caps();
                let s = caps.structure(0).unwrap();
                if s.name().starts_with("audio/") {
                    state.audio_info = gst_audio::AudioInfo::from_caps(caps).ok();
                    gst::log!(CAT, obj = pad, "Got audio caps {:?}", state.audio_info);
                    state.video_info = None;
                } else if s.name().starts_with("video/") {
                    state.audio_info = None;
                    state.video_info = gst_video::VideoInfo::from_caps(caps).ok();
                    gst::log!(CAT, obj = pad, "Got video caps {:?}", state.video_info);
                } else {
                    state.audio_info = None;
                    state.video_info = None;
                }
            }
            EventView::Segment(e) => {
                let mut state = stream.state.lock();

                let segment = match e.segment().clone().downcast::<gst::ClockTime>() {
                    Err(segment) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["Only Time segments supported, got {:?}", segment.format(),]
                        );
                        return false;
                    }
                    Ok(segment) => segment,
                };

                if (segment.rate() - 1.0).abs() > f64::EPSILON {
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Format,
                        [
                            "Only rate==1.0 segments supported, got {:?}",
                            segment.rate(),
                        ]
                    );
                    return false;
                }

                state.in_segment = segment;
                state.segment_seqnum = event.seqnum();
                state.segment_pending = true;
                state.current_running_time = None;
                state.current_running_time_end = None;

                gst::debug!(CAT, obj = pad, "Got new Segment {:?}", state.in_segment);

                forward = false;
            }
            EventView::Gap(e) => {
                gst::debug!(CAT, obj = pad, "Handling Gap event {:?}", event);
                let (pts, duration) = e.get();
                let upstream_live;

                {
                    let mut state = stream.state.lock();
                    match state.upstream_live {
                        None => {
                            // Not handling anything here, the pad's query function will catch it
                            let mut query = gst::query::Latency::new();
                            let success = pad.peer_query(&mut query);
                            if success {
                                (upstream_live, _, _) = query.result();
                                state.upstream_live = Some(upstream_live);
                            } else {
                                state.upstream_live = None;
                                upstream_live = false;
                                gst::warning!(
                                    CAT,
                                    obj = pad,
                                    "Latency query failed, assuming non-live input, will retry"
                                );
                            }
                        }
                        Some(is_live) => upstream_live = is_live,
                    }
                }

                drop(rec_state);
                let handle_result = if stream == self.main_stream {
                    self.handle_main_stream(pad, &stream, (pts, duration), upstream_live)
                } else {
                    self.handle_secondary_stream(pad, &stream, (pts, duration), upstream_live)
                };

                forward = match handle_result {
                    Ok(HandleResult::Pass((new_pts, new_duration))) => {
                        if new_pts != pts || new_duration.is_some() && new_duration != duration {
                            event = gst::event::Gap::builder(new_pts)
                                .duration(new_duration)
                                .build();
                        }
                        true
                    }
                    _ => false,
                };
            }
            EventView::StreamStart(..) => {
                let main_state = if stream != self.main_stream {
                    Some(self.main_stream.state.lock())
                } else {
                    None
                };
                let mut state = stream.state.lock();
                state.eos = false;

                let main_is_eos = main_state.as_ref().is_some_and(|main_state| main_state.eos);

                if !main_is_eos {
                    recording_state_changed = self.check_and_update_stream_start(
                        pad,
                        &stream,
                        &mut state,
                        &mut rec_state,
                    );
                }

                self.main_stream_cond.notify_all();
                gst::debug!(CAT, obj = pad, "Stream is not EOS now");
            }
            EventView::Eos(..) => {
                let main_state = if stream != self.main_stream {
                    Some(self.main_stream.state.lock())
                } else {
                    None
                };
                let mut state = stream.state.lock();
                state.eos = true;

                let main_is_eos = main_state.as_ref().is_none_or(|main_state| main_state.eos);
                drop(main_state);

                if main_is_eos {
                    recording_state_changed =
                        self.check_and_update_eos(pad, &stream, &mut state, &mut rec_state);
                }

                self.main_stream_cond.notify_all();
                gst::debug!(
                    CAT,
                    obj = pad,
                    "Stream is EOS now, sending any pending events"
                );

                send_pending = true;
            }
            _ => (),
        };

        if recording_state_changed {
            self.obj().notify("recording");
        }

        // If a serialized event and coming after Segment and a new Segment is pending,
        // queue up and send at a later time (buffer/gap) after we sent the Segment
        let type_ = event.type_();
        if forward
            && type_ != gst::EventType::Eos
            && type_.is_serialized()
            && type_.partial_cmp(&gst::EventType::Segment) == Some(cmp::Ordering::Greater)
        {
            let mut state = stream.state.lock();
            if state.segment_pending {
                gst::log!(CAT, obj = pad, "Storing event for later pushing");
                state.pending_events.push(event);
                return true;
            }
        }

        if send_pending {
            let mut state = stream.state.lock();
            let mut events = Vec::with_capacity(state.pending_events.len() + 1);

            // Got not a single buffer on this stream before EOS, forward
            // the input segment
            if state.segment_pending {
                events.push(
                    gst::event::Segment::builder(&state.in_segment)
                        .seqnum(state.segment_seqnum)
                        .build(),
                );
            }
            events.append(&mut state.pending_events);
            drop(state);

            for e in events.drain(..) {
                stream.srcpad.push_event(e);
            }
        }

        if forward {
            gst::log!(CAT, obj = pad, "Forwarding event {:?}", event);
            stream.srcpad.push_event(event)
        } else {
            gst::log!(CAT, obj = pad, "Dropping event {:?}", event);
            true
        }
    }

    // called without lock
    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        let rec_state = self.state.lock();
        let stream = match rec_state.pads.get(pad) {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Pad,
                    ["Unknown pad {:?}", pad.name()]
                );
                return false;
            }
            Some(stream) => stream.clone(),
        };

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        drop(rec_state);
        let success = stream.srcpad.peer_query(query);

        if let gst::QueryView::Latency(latency) = query.view() {
            let mut state = stream.state.lock();
            if success {
                let (is_live, _, _) = latency.result();
                state.upstream_live = Some(is_live);
            } else {
                state.upstream_live = None;
            }
        }

        success
    }

    // called without lock
    fn src_event(&self, pad: &gst::Pad, mut event: gst::Event) -> bool {
        let rec_state = self.state.lock();
        use gst::EventView;

        let stream = match rec_state.pads.get(pad) {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Pad,
                    ["Unknown pad {:?}", pad.name()]
                );
                return false;
            }
            Some(stream) => stream.clone(),
        };

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        let forward = !matches!(event.view(), EventView::Seek(..));

        let offset = event.running_time_offset();
        event
            .make_mut()
            .set_running_time_offset(offset - rec_state.running_time_offset);
        drop(rec_state);

        if forward {
            gst::log!(CAT, obj = pad, "Forwarding event {:?}", event);
            stream.sinkpad.push_event(event)
        } else {
            gst::log!(CAT, obj = pad, "Dropping event {:?}", event);
            false
        }
    }

    // called without lock
    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        let rec_state = self.state.lock();
        use gst::QueryViewMut;

        let stream = match rec_state.pads.get(pad) {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Pad,
                    ["Unknown pad {:?}", pad.name()]
                );
                return false;
            }
            Some(stream) => stream.clone(),
        };

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);
        match query.view_mut() {
            QueryViewMut::Scheduling(q) => {
                let mut new_query = gst::query::Scheduling::new();
                let res = stream.sinkpad.peer_query(&mut new_query);
                if !res {
                    return res;
                }

                gst::log!(CAT, obj = pad, "Downstream returned {:?}", new_query);

                let (flags, min, max, align) = new_query.result();
                q.set(flags, min, max, align);
                q.add_scheduling_modes(
                    new_query
                        .scheduling_modes()
                        .filter(|m| m != &gst::PadMode::Pull),
                );
                gst::log!(CAT, obj = pad, "Returning {:?}", q.query_mut());
                true
            }
            QueryViewMut::Seeking(q) => {
                // Seeking is not possible here
                let format = q.format();
                q.set(
                    false,
                    gst::GenericFormattedValue::none_for_format(format),
                    gst::GenericFormattedValue::none_for_format(format),
                );

                gst::log!(CAT, obj = pad, "Returning {:?}", q.query_mut());
                true
            }
            // Position and duration is always the current recording position
            QueryViewMut::Position(q) => {
                if q.format() == gst::Format::Time {
                    let state = stream.state.lock();
                    let mut recording_duration = rec_state.recording_duration;
                    if rec_state.recording_state == RecordingState::Recording
                        || rec_state.recording_state == RecordingState::Stopping
                    {
                        if let Some(delta) = state
                            .current_running_time_end
                            .opt_checked_sub(rec_state.last_recording_start)
                            .ok()
                            .flatten()
                        {
                            gst::debug!(
                                CAT,
                                obj = pad,
                                "Returning position {} = {} - ({} + {})",
                                recording_duration + delta,
                                recording_duration,
                                state.current_running_time_end.display(),
                                rec_state.last_recording_start.display(),
                            );
                            recording_duration += delta;
                        }
                    } else {
                        gst::debug!(CAT, obj = pad, "Returning position {}", recording_duration);
                    }
                    q.set(recording_duration);
                    true
                } else {
                    false
                }
            }
            QueryViewMut::Duration(q) => {
                if q.format() == gst::Format::Time {
                    let state = stream.state.lock();
                    let mut recording_duration = rec_state.recording_duration;
                    if rec_state.recording_state == RecordingState::Recording
                        || rec_state.recording_state == RecordingState::Stopping
                    {
                        if let Some(delta) = state
                            .current_running_time_end
                            .opt_checked_sub(rec_state.last_recording_start)
                            .ok()
                            .flatten()
                        {
                            gst::debug!(
                                CAT,
                                obj = pad,
                                "Returning duration {} = {} - ({} + {})",
                                recording_duration + delta,
                                recording_duration,
                                state.current_running_time_end.display(),
                                rec_state.last_recording_start.display(),
                            );
                            recording_duration += delta;
                        }
                    } else {
                        gst::debug!(CAT, obj = pad, "Returning duration {}", recording_duration);
                    }
                    q.set(recording_duration);
                    true
                } else {
                    false
                }
            }
            _ => {
                gst::log!(CAT, obj = pad, "Forwarding query {:?}", query);
                drop(rec_state);
                stream.sinkpad.peer_query(query)
            }
        }
    }

    // called without lock
    fn iterate_internal_links(&self, pad: &gst::Pad) -> gst::Iterator<gst::Pad> {
        let rec_state = self.state.lock();
        let stream = match rec_state.pads.get(pad) {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Pad,
                    ["Unknown pad {:?}", pad.name()]
                );
                return gst::Iterator::from_vec(vec![]);
            }
            Some(stream) => stream.clone(),
        };

        if pad == &stream.srcpad {
            gst::Iterator::from_vec(vec![stream.sinkpad])
        } else {
            gst::Iterator::from_vec(vec![stream.srcpad])
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ToggleRecord {
    const NAME: &'static str = "GstToggleRecord";
    type Type = super::ToggleRecord;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |togglerecord| togglerecord.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.sink_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |togglerecord| togglerecord.iterate_internal_links(pad),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.src_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |togglerecord| togglerecord.iterate_internal_links(pad),
                )
            })
            .build();

        let main_stream = Stream::new(sinkpad, srcpad);

        let mut pads = HashMap::new();
        pads.insert(main_stream.sinkpad.clone(), main_stream.clone());
        pads.insert(main_stream.srcpad.clone(), main_stream.clone());

        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::new(pads)),
            main_stream,
            main_stream_cond: Condvar::new(),
        }
    }
}

impl ObjectImpl for ToggleRecord {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("record")
                    .nick("Record")
                    .blurb("Enable/disable recording")
                    .default_value(DEFAULT_RECORD)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("recording")
                    .nick("Recording")
                    .blurb("Whether recording is currently taking place")
                    .default_value(DEFAULT_RECORD)
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("is-live")
                    .nick("Live output mode")
                    .blurb(
                        "Live output mode: no \"gap eating\", \
                        forward incoming segment for live input, \
                        create a gap to fill the paused duration for non-live input",
                    )
                    .default_value(DEFAULT_LIVE)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "record" => {
                let mut settings = self.settings.lock();
                let record = value.get().expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Setting record from {:?} to {:?}",
                    settings.record,
                    record
                );

                settings.record = record;
                drop(settings);
                self.main_stream_cond.notify_all();
            }
            "is-live" => {
                let mut settings = self.settings.lock();
                let live = value.get().expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Setting live from {:?} to {:?}",
                    settings.live,
                    live
                );

                settings.live = live;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "record" => {
                let settings = self.settings.lock();
                settings.record.to_value()
            }
            "recording" => {
                let rec_state = self.state.lock();
                (rec_state.recording_state == RecordingState::Recording).to_value()
            }
            "is-live" => {
                let settings = self.settings.lock();
                settings.live.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.main_stream.sinkpad).unwrap();
        obj.add_pad(&self.main_stream.srcpad).unwrap();
    }
}

impl GstObjectImpl for ToggleRecord {}

impl ElementImpl for ToggleRecord {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Toggle Record",
                "Generic",
                "Valve that ensures multiple streams start/end at the same time. \
                If the input comes from a live stream, when not recording it will be dropped. \
                If it comes from a non-live stream, when not recording it will be blocked.",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let secondary_src_pad_template = gst::PadTemplate::new(
                "src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &caps,
            )
            .unwrap();

            let secondary_sink_pad_template = gst::PadTemplate::new(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
            )
            .unwrap();

            vec![
                src_pad_template,
                sink_pad_template,
                secondary_src_pad_template,
                secondary_sink_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut rec_state = self.state.lock();
                rec_state.reset();

                for s in rec_state
                    .other_streams
                    .iter()
                    .chain(iter::once(&self.main_stream))
                {
                    let mut state = s.state.lock();
                    *state = StreamState::default();
                }

                let settings = self.settings.lock();
                rec_state.live = settings.live;
            }
            gst::StateChange::PausedToReady => {
                let rec_state = self.state.lock();

                for s in &rec_state.other_streams {
                    let mut state = s.state.lock();
                    state.flushing = true;
                }

                let mut state = self.main_stream.state.lock();
                state.flushing = true;
                self.main_stream_cond.notify_all();
            }
            _ => (),
        }

        let success = self.parent_change_state(transition)?;

        if transition == gst::StateChange::PausedToReady {
            let mut rec_state = self.state.lock();

            for s in rec_state
                .other_streams
                .iter()
                .chain(iter::once(&self.main_stream))
            {
                let mut state = s.state.lock();

                state.pending_events.clear();
            }

            rec_state.reset();
            drop(rec_state);
            self.obj().notify("recording");
        }

        Ok(success)
    }

    fn request_new_pad(
        &self,
        _templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut rec_state = self.state.lock();
        let id = rec_state.next_pad_id;
        rec_state.next_pad_id += 1;

        let templ = self.obj().pad_template("sink_%u").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .name(format!("sink_{id}").as_str())
            .chain_function(|pad, parent, buffer| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |togglerecord| togglerecord.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.sink_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |togglerecord| togglerecord.iterate_internal_links(pad),
                )
            })
            .build();

        let templ = self.obj().pad_template("src_%u").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .name(format!("src_{id}").as_str())
            .event_function(|pad, parent, event| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || false,
                    |togglerecord| togglerecord.src_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                ToggleRecord::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |togglerecord| togglerecord.iterate_internal_links(pad),
                )
            })
            .build();

        sinkpad.set_active(true).unwrap();
        srcpad.set_active(true).unwrap();

        let stream = Stream::new(sinkpad.clone(), srcpad.clone());

        rec_state
            .pads
            .insert(stream.sinkpad.clone(), stream.clone());
        rec_state.pads.insert(stream.srcpad.clone(), stream.clone());

        rec_state.other_streams.push(stream);

        drop(rec_state);

        self.obj().add_pad(&sinkpad).unwrap();
        self.obj().add_pad(&srcpad).unwrap();

        Some(sinkpad)
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut rec_state = self.state.lock();

        let stream = match rec_state.pads.get(pad) {
            None => return,
            Some(stream) => stream.clone(),
        };

        rec_state.pads.remove(&stream.sinkpad).unwrap();
        rec_state.pads.remove(&stream.srcpad).unwrap();

        // TODO: Replace with Vec::remove_item() once stable
        let pos = rec_state.other_streams.iter().position(|x| *x == stream);
        pos.map(|pos| rec_state.other_streams.swap_remove(pos));

        drop(rec_state);

        let main_state = self.main_stream.state.lock();
        self.main_stream_cond.notify_all();
        drop(main_state);

        stream.srcpad.set_active(false).unwrap();
        stream.sinkpad.set_active(false).unwrap();

        self.obj().remove_pad(&stream.sinkpad).unwrap();
        self.obj().remove_pad(&stream.srcpad).unwrap();
    }
}
