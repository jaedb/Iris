// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019 Jordan Petridis <jordan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::cmp;
use std::sync::{Mutex, MutexGuard};

use std::sync::LazyLock;

use super::parser::{SccLine, SccParser};
use crate::line_reader::LineReader;
use crate::parser_utils::TimeCode;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "sccparse",
        gst::DebugColorFlags::empty(),
        Some("Scc Parser Element"),
    )
});

#[derive(Debug)]
struct PullState {
    need_stream_start: bool,
    stream_id: String,
    offset: u64,
    duration: Option<gst::ClockTime>,
}

impl PullState {
    fn new(imp: &SccParse, pad: &gst::Pad) -> Self {
        Self {
            need_stream_start: true,
            stream_id: pad.create_stream_id(&*imp.obj(), Some("src")).to_string(),
            offset: 0,
            duration: gst::ClockTime::NONE,
        }
    }
}

#[derive(Debug)]
struct State {
    reader: LineReader<gst::MappedBuffer<gst::buffer::Readable>>,
    parser: SccParser,
    need_segment: bool,
    pending_events: Vec<gst::Event>,
    framerate: Option<gst::Fraction>,
    last_position: Option<gst::ClockTime>,
    last_timecode: Option<gst_video::ValidVideoTimeCode>,
    segment: gst::FormattedSegment<gst::ClockTime>,

    // Pull mode
    pull: Option<PullState>,

    // seeking
    seeking: bool,
    discont: bool,
    seek_seqnum: Option<gst::Seqnum>,
    need_flush_stop: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            reader: LineReader::new(),
            parser: SccParser::new(),
            need_segment: true,
            pending_events: Vec::new(),
            framerate: None,
            last_position: None,
            last_timecode: None,
            segment: gst::FormattedSegment::new(),
            pull: None,
            seeking: false,
            discont: false,
            seek_seqnum: None,
            need_flush_stop: false,
        }
    }
}

fn parse_timecode(
    framerate: gst::Fraction,
    tc: &TimeCode,
) -> Result<gst_video::ValidVideoTimeCode, gst::FlowError> {
    let mut tc = tc.clone();
    // Workaround for various SCC files having invalid drop frame timecodes:
    // Every full minute the first two timecodes are skipped, except for every tenth minute.
    if tc.drop_frame
        && tc.seconds == 0
        && tc.minutes % 10 != 0
        && (tc.frames == 0 || tc.frames == 1)
    {
        tc.frames = 2;
    }

    let timecode = gst_video::VideoTimeCode::new(
        framerate,
        None,
        if tc.drop_frame {
            gst_video::VideoTimeCodeFlags::DROP_FRAME
        } else {
            gst_video::VideoTimeCodeFlags::empty()
        },
        tc.hours,
        tc.minutes,
        tc.seconds,
        tc.frames,
        0,
    );

    gst_video::ValidVideoTimeCode::try_from(timecode).map_err(|_| gst::FlowError::Error)
}

impl State {
    #[allow(clippy::type_complexity)]
    fn line(
        &mut self,
        drain: bool,
    ) -> Result<Option<SccLine>, (&[u8], winnow::error::ContextError)> {
        let line = match self.reader.line_with_drain(drain) {
            None => {
                return Ok(None);
            }
            Some(line) => line,
        };

        self.parser
            .parse_line(line)
            .map(Option::Some)
            .map_err(|err| (line, err))
    }

    fn handle_timecode(
        &mut self,
        tc: &TimeCode,
        framerate: gst::Fraction,
        imp: &SccParse,
    ) -> Result<gst_video::ValidVideoTimeCode, gst::FlowError> {
        match parse_timecode(framerate, tc) {
            Ok(timecode) => Ok(timecode),
            Err(err) => {
                let last_timecode = self.last_timecode.clone().ok_or_else(|| {
                    gst::element_imp_error!(
                        imp,
                        gst::StreamError::Decode,
                        ["Invalid first timecode {:?}", err]
                    );

                    gst::FlowError::Error
                })?;

                gst::warning!(
                    CAT,
                    imp = imp,
                    "Invalid timecode {:?}, using previous {:?}",
                    err,
                    last_timecode
                );

                Ok(last_timecode)
            }
        }
    }

    /// Calculate a timestamp from the timecode and make sure to
    /// not produce timestamps jumping backwards
    fn update_timestamp(&mut self, timecode: &gst_video::ValidVideoTimeCode, imp: &SccParse) {
        let nsecs = timecode.time_since_daily_jam();

        if self
            .last_position
            .is_none_or(|last_position| nsecs >= last_position)
        {
            self.last_position = Some(nsecs);
        } else {
            gst::fixme!(
                CAT,
                imp = imp,
                "New position {} < last position {}",
                nsecs,
                self.last_position.display(),
            );
        }
    }

    fn add_buffer_metadata(
        &mut self,
        buffer: &mut gst::buffer::Buffer,
        timecode: &gst_video::ValidVideoTimeCode,
        framerate: gst::Fraction,
        imp: &SccParse,
    ) {
        let buffer = buffer.get_mut().unwrap();
        gst_video::VideoTimeCodeMeta::add(buffer, timecode);

        self.update_timestamp(timecode, imp);

        buffer.set_pts(self.last_position);
        buffer.set_duration(
            gst::ClockTime::SECOND.mul_div_ceil(framerate.denom() as u64, framerate.numer() as u64),
        );
    }

    fn create_events(
        &mut self,
        imp: &SccParse,
        framerate: Option<gst::Fraction>,
    ) -> Vec<gst::Event> {
        let mut events = Vec::new();

        if self.need_flush_stop {
            events.push(
                gst::event::FlushStop::builder(true)
                    .seqnum_if_some(self.seek_seqnum)
                    .build(),
            );
            self.need_flush_stop = false;
        }

        if let Some(pull) = &mut self.pull {
            if pull.need_stream_start {
                events.push(gst::event::StreamStart::new(&pull.stream_id));
                pull.need_stream_start = false;
            }
        }

        if let Some(framerate) = framerate {
            if self.framerate != Some(framerate) {
                self.framerate = Some(framerate);

                let caps = gst::Caps::builder("closedcaption/x-cea-608")
                    .field("format", "raw")
                    .field("framerate", framerate)
                    .build();
                self.framerate = Some(framerate);

                events.push(gst::event::Caps::new(&caps));
                gst::info!(CAT, imp = imp, "Caps changed to {:?}", &caps);
            }
        }

        if self.need_segment {
            events.push(
                gst::event::Segment::builder(&self.segment)
                    .seqnum_if_some(self.seek_seqnum)
                    .build(),
            );
            self.need_segment = false;
        }

        events.append(&mut self.pending_events);
        events
    }
}

pub struct SccParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

impl SccParse {
    fn handle_buffer(
        &self,
        buffer: Option<gst::Buffer>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        let drain = if let Some(buffer) = buffer {
            let buffer = buffer.into_mapped_buffer_readable().map_err(|_| {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Read,
                    ["Failed to map buffer readable"]
                );

                gst::FlowError::Error
            })?;

            state.reader.push(buffer);
            false
        } else {
            true
        };

        loop {
            let line = state.line(drain);
            match line {
                Ok(Some(SccLine::Caption(tc, data))) => {
                    state = self.handle_line(tc, data, state)?;
                }
                Ok(Some(line)) => {
                    gst::debug!(CAT, imp = self, "Got line '{:?}'", line);
                }
                Err((line, err)) => {
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Decode,
                        ["Couldn't parse line '{:?}': {:?}", line, err]
                    );

                    break Err(gst::FlowError::Error);
                }
                Ok(None) => {
                    if drain && state.pull.is_some() {
                        break Err(gst::FlowError::Eos);
                    }
                    break Ok(gst::FlowSuccess::Ok);
                }
            }
        }
    }

    fn handle_line(
        &self,
        tc: TimeCode,
        data: Vec<u8>,
        mut state: MutexGuard<'_, State>,
    ) -> Result<MutexGuard<'_, State>, gst::FlowError> {
        gst::trace!(
            CAT,
            imp = self,
            "Got caption buffer with timecode {:?} and size {}",
            tc,
            data.len()
        );

        // The framerate is defined as 30 or 30000/1001 according to:
        // http://www.theneitherworld.com/mcpoodle/SCC_TOOLS/DOCS/SCC_FORMAT.HTML
        let framerate = if tc.drop_frame {
            gst::Fraction::new(30000, 1001)
        } else {
            gst::Fraction::new(30, 1)
        };

        let mut timecode = state.handle_timecode(&tc, framerate, self)?;
        let start_time = timecode.time_since_daily_jam();
        let segment_start = state.segment.start();
        let clip_buffers = if state.seeking {
            // If we are in the middle of seeking, check whether this line
            // contains start frame, and if so, unset seeking flag
            let num_bufs = (data.len() / 2) as i64;
            let mut end_timecode = timecode.clone();
            // add one more frame here so that add duration of the last frame
            end_timecode.add_frames(num_bufs + 1);
            let stop_time = end_timecode.time_since_daily_jam();

            gst::trace!(
                CAT,
                imp = self,
                "Checking inside of segment, line start {} line stop {} segment start {} num bufs {}",
                start_time,
                stop_time,
                segment_start.display(),
                num_bufs,
            );

            if segment_start.is_some_and(|seg_start| stop_time > seg_start) {
                state.seeking = false;
                state.discont = true;
                state.need_flush_stop = true;
            }

            // Still need to scan lines to find the first buffer
            if state.seeking {
                // Remember this timecode in order to fallback to this one
                // if invalid timecode is detected during scanning
                state.last_timecode = Some(timecode);

                drop(state);
                return Ok(self.state.lock().unwrap());
            }

            true
        } else {
            false
        };

        let mut buffers = if clip_buffers {
            gst::BufferList::new()
        } else {
            gst::BufferList::new_sized(data.len() / 2)
        };

        let mut send_eos = false;
        for d in data.chunks_exact(2) {
            let mut buffer = gst::Buffer::with_size(d.len()).unwrap();
            {
                let buf_mut = buffer.get_mut().unwrap();
                buf_mut.copy_from_slice(0, d).unwrap();
            }

            state.add_buffer_metadata(&mut buffer, &timecode, framerate, self);
            timecode.increment_frame();

            if clip_buffers {
                let end_time = buffer.pts().opt_add(buffer.duration());
                if end_time.opt_lt(segment_start).unwrap_or(false) {
                    gst::trace!(CAT, imp = self, "Skip segment clipped buffer {:?}", buffer,);

                    continue;
                }
            }

            send_eos = buffer
                .pts()
                .opt_add(buffer.duration())
                .opt_ge(state.segment.stop())
                .unwrap_or(false);

            let buffers = buffers.get_mut().unwrap();
            buffers.add(buffer);

            // Terminate loop once we found EOS boundary buffer
            if send_eos {
                break;
            }
        }

        // Update the last_timecode to the current one
        state.last_timecode = Some(timecode);

        let events = state.create_events(self, Some(framerate));

        // Drop our state mutex while we push out buffers or events
        drop(state);

        for event in events {
            gst::debug!(CAT, imp = self, "Pushing event {:?}", event);
            self.srcpad.push_event(event);
        }

        self.srcpad.push_list(buffers).inspect_err(|&err| {
            if err != gst::FlowError::Flushing && err != gst::FlowError::Eos {
                gst::error!(CAT, imp = self, "Pushing buffer returned {:?}", err);
            }
        })?;

        if send_eos {
            return Err(gst::FlowError::Eos);
        }

        Ok(self.state.lock().unwrap())
    }

    fn sink_activate(&self, pad: &gst::Pad) -> Result<(), gst::LoggableError> {
        let mode = {
            let mut query = gst::query::Scheduling::new();
            let mut state = self.state.lock().unwrap();

            state.pull = None;

            if !pad.peer_query(&mut query) {
                gst::debug!(CAT, obj = pad, "Scheduling query failed on peer");
                gst::PadMode::Push
            } else if query
                .has_scheduling_mode_with_flags(gst::PadMode::Pull, gst::SchedulingFlags::SEEKABLE)
            {
                gst::debug!(CAT, obj = pad, "Activating in Pull mode");

                state.pull = Some(PullState::new(self, &self.srcpad));

                gst::PadMode::Pull
            } else {
                gst::debug!(CAT, obj = pad, "Activating in Push mode");
                gst::PadMode::Push
            }
        };

        pad.activate_mode(mode, true)?;
        Ok(())
    }

    fn start_task(&self) -> Result<(), gst::LoggableError> {
        let imp = self.ref_counted();
        let res = self.sinkpad.start_task(move || {
            imp.loop_fn();
        });
        if res.is_err() {
            return Err(gst::loggable_error!(CAT, "Failed to start pad task"));
        }
        Ok(())
    }

    fn sink_activatemode(
        &self,
        _pad: &gst::Pad,
        mode: gst::PadMode,
        active: bool,
    ) -> Result<(), gst::LoggableError> {
        if mode == gst::PadMode::Pull {
            if active {
                self.start_task()?;
            } else {
                let _ = self.sinkpad.stop_task();
            }
        }

        Ok(())
    }

    fn scan_duration(&self) -> Result<Option<gst_video::ValidVideoTimeCode>, gst::FlowError> {
        gst::debug!(CAT, imp = self, "Scanning duration");

        /* First let's query the bytes duration upstream */
        let mut q = gst::query::Duration::new(gst::Format::Bytes);

        if !self.sinkpad.peer_query(&mut q) {
            gst::error!(CAT, imp = self, "Failed to query upstream duration");
            return Err(gst::FlowError::Error);
        }

        let size = match q.result() {
            gst::GenericFormattedValue::Bytes(Some(size)) => *size,
            _ => {
                gst::error!(CAT, imp = self, "Failed to query upstream duration");
                return Err(gst::FlowError::Error);
            }
        };

        let mut offset = size;
        let mut buffers = Vec::new();
        let mut last_tc = None;

        loop {
            let scan_size = cmp::min(offset, 4096);

            offset -= scan_size;

            match self.sinkpad.pull_range(offset, scan_size as u32) {
                Ok(buffer) => {
                    buffers.push(buffer);
                }
                Err(flow) => {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Failed to pull buffer while scanning duration: {:?}",
                        flow
                    );
                    return Err(flow);
                }
            }

            let mut reader = LineReader::new();
            let mut parser = SccParser::new_scan_captions();

            for buf in buffers.iter().rev() {
                let buf = buf.clone().into_mapped_buffer_readable().map_err(|_| {
                    gst::error!(CAT, imp = self, "Failed to map buffer readable");
                    gst::FlowError::Error
                })?;

                reader.push(buf);
            }

            while let Some(line) = reader.line_with_drain(true) {
                if let Ok(SccLine::Caption(tc, data)) =
                    parser.parse_line(line).map_err(|err| (line, err))
                {
                    let framerate = if tc.drop_frame {
                        gst::Fraction::new(30000, 1001)
                    } else {
                        gst::Fraction::new(30, 1)
                    };

                    if let Ok(mut timecode) = parse_timecode(framerate, &tc) {
                        /* We're looking for the total duration */
                        timecode.add_frames((data.len() / 2) as i64 + 1);
                        last_tc = Some(timecode);
                    }
                }
            }

            if last_tc.is_some() || offset == 0 {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Duration scan done, last_tc: {:?}",
                    last_tc
                );
                break Ok(last_tc);
            }
        }
    }

    fn push_eos(&self) {
        let mut state = self.state.lock().unwrap();

        if state.seeking {
            state.need_flush_stop = true;
        }

        let mut events = state.create_events(self, None);
        events.push(
            gst::event::Eos::builder()
                .seqnum_if_some(state.seek_seqnum)
                .build(),
        );

        // Drop our state mutex while we push out events
        drop(state);

        for event in events {
            gst::debug!(CAT, imp = self, "Pushing event {:?}", event);
            self.srcpad.push_event(event);
        }
    }

    fn loop_fn(&self) {
        let mut state = self.state.lock().unwrap();
        let State {
            ref framerate,
            ref mut pull,
            ..
        } = *state;
        let pull = pull.as_mut().unwrap();
        let scan_duration = framerate.is_none() && pull.duration.is_none();
        let offset = pull.offset;

        pull.offset += 4096;

        drop(state);

        let buffer = match self.sinkpad.pull_range(offset, 4096) {
            Ok(buffer) => Some(buffer),
            Err(gst::FlowError::Eos) => None,
            Err(gst::FlowError::Flushing) => {
                gst::debug!(
                    CAT,
                    obj = self.sinkpad,
                    "Pausing after pulling buffer, reason: flushing"
                );

                let _ = self.sinkpad.pause_task();
                return;
            }
            Err(flow) => {
                gst::error!(
                    CAT,
                    obj = self.sinkpad,
                    "Failed to pull, reason: {:?}",
                    flow
                );

                gst::element_imp_error!(
                    self,
                    gst::StreamError::Failed,
                    ["Streaming stopped, failed to pull buffer"]
                );

                let _ = self.sinkpad.pause_task();
                return;
            }
        };

        if let Err(flow) = self.handle_buffer(buffer) {
            match flow {
                gst::FlowError::Flushing => {
                    gst::debug!(CAT, imp = self, "Pausing after flow {:?}", flow);
                }
                gst::FlowError::Eos => {
                    self.push_eos();

                    gst::debug!(CAT, imp = self, "Pausing after flow {:?}", flow);
                }
                _ => {
                    self.push_eos();

                    gst::error!(CAT, imp = self, "Pausing after flow {:?}", flow);

                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Failed,
                        ["Streaming stopped, reason: {:?}", flow]
                    );
                }
            }

            let _ = self.sinkpad.pause_task();
            return;
        }

        if scan_duration {
            let duration = match self.scan_duration() {
                Ok(Some(tc)) => tc.time_since_daily_jam(),
                Ok(None) => gst::ClockTime::ZERO,
                Err(gst::FlowError::Flushing) => {
                    gst::debug!(CAT, imp = self, "Failed to scan duration, reason: flushing");

                    let _ = self.sinkpad.pause_task();
                    return;
                }
                Err(flow) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to scan duration, reason: {:?}",
                        flow
                    );

                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Decode,
                        ["Failed to scan duration"]
                    );

                    let _ = self.sinkpad.pause_task();
                    return;
                }
            };

            let mut state = self.state.lock().unwrap();
            let pull = state.pull.as_mut().unwrap();
            pull.duration = Some(duration);
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        self.handle_buffer(Some(buffer))
    }

    fn flush(&self, mut state: MutexGuard<'_, State>) -> MutexGuard<'_, State> {
        state.reader.clear();
        state.parser.reset();
        if let Some(pull) = &mut state.pull {
            pull.offset = 0;
        }
        state.segment = gst::FormattedSegment::new();
        state.need_segment = true;
        state.pending_events.clear();
        state.last_position = None;
        state.last_timecode = None;

        drop(state);

        self.state.lock().unwrap()
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(_) => {
                // We send a proper caps event from the chain function later
                gst::log!(CAT, obj = pad, "Dropping caps event");
                true
            }
            EventView::Segment(_) => {
                // We send a gst::Format::Time segment event later when needed
                gst::log!(CAT, obj = pad, "Dropping segment event");
                true
            }
            EventView::FlushStop(_) => {
                let state = self.state.lock().unwrap();
                let state = self.flush(state);
                drop(state);

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Eos(_) => {
                gst::log!(CAT, obj = pad, "Draining");
                if let Err(err) = self.handle_buffer(None) {
                    gst::error!(CAT, obj = pad, "Failed to drain parser: {:?}", err);
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => {
                if event.is_sticky()
                    && !self.srcpad.has_current_caps()
                    && event.type_() > gst::EventType::Caps
                {
                    gst::log!(CAT, obj = pad, "Deferring sticky event until we have caps");
                    let mut state = self.state.lock().unwrap();
                    state.pending_events.push(event);
                    true
                } else {
                    gst::Pad::event_default(pad, Some(&*self.obj()), event)
                }
            }
        }
    }

    fn perform_seek(&self, event: &gst::event::Seek) -> bool {
        if self.state.lock().unwrap().pull.is_none() {
            gst::error!(CAT, imp = self, "seeking is only supported in pull mode");
            return false;
        }

        let (rate, flags, start_type, start, stop_type, stop) = event.get();

        let mut start: Option<gst::ClockTime> = match start.try_into() {
            Ok(start) => start,
            Err(_) => {
                gst::error!(CAT, imp = self, "seek has invalid format");
                return false;
            }
        };

        let mut stop: Option<gst::ClockTime> = match stop.try_into() {
            Ok(stop) => stop,
            Err(_) => {
                gst::error!(CAT, imp = self, "seek has invalid format");
                return false;
            }
        };

        if !flags.contains(gst::SeekFlags::FLUSH) {
            gst::error!(CAT, imp = self, "only flushing seeks are supported");
            return false;
        }

        if start_type == gst::SeekType::End || stop_type == gst::SeekType::End {
            gst::error!(CAT, imp = self, "Relative seeks are not supported");
            return false;
        }

        let seek_seqnum = event.seqnum();

        let event = gst::event::FlushStart::builder()
            .seqnum(seek_seqnum)
            .build();

        gst::debug!(CAT, imp = self, "Sending event {:?} upstream", event);
        self.sinkpad.push_event(event);

        let event = gst::event::FlushStart::builder()
            .seqnum(seek_seqnum)
            .build();

        gst::debug!(CAT, imp = self, "Pushing event {:?}", event);
        self.srcpad.push_event(event);

        let _ = self.sinkpad.pause_task();

        let mut state = self.state.lock().unwrap();
        let pull = state.pull.as_ref().unwrap();

        if start_type == gst::SeekType::Set {
            start = start.opt_min(pull.duration).or(start);
        }

        if stop_type == gst::SeekType::Set {
            stop = stop.opt_min(pull.duration).or(stop);
        }

        state.seeking = true;
        state.seek_seqnum = Some(seek_seqnum);

        state = self.flush(state);

        let event = gst::event::FlushStop::builder(true)
            .seqnum(seek_seqnum)
            .build();

        /* Drop our state while we push a serialized event upstream */
        drop(state);

        gst::debug!(CAT, imp = self, "Sending event {:?} upstream", event);
        self.sinkpad.push_event(event);

        state = self.state.lock().unwrap();

        state
            .segment
            .do_seek(rate, flags, start_type, start, stop_type, stop);

        match self.start_task() {
            Err(error) => {
                error.log();
                false
            }
            _ => true,
        }
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Seek(e) => self.perform_seek(e),
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Seeking(q) => {
                let state = self.state.lock().unwrap();

                let fmt = q.format();

                if fmt == gst::Format::Time {
                    if let Some(pull) = state.pull.as_ref() {
                        q.set(true, gst::ClockTime::ZERO, pull.duration);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            QueryViewMut::Position(q) => {
                // For Time answer ourselves, otherwise forward
                if q.format() == gst::Format::Time {
                    let state = self.state.lock().unwrap();
                    q.set(state.last_position);
                    true
                } else {
                    self.sinkpad.peer_query(query)
                }
            }
            QueryViewMut::Duration(q) => {
                // For Time answer ourselves, otherwise forward
                let state = self.state.lock().unwrap();
                if q.format() == gst::Format::Time {
                    if let Some(pull) = state.pull.as_ref() {
                        if pull.duration.is_some() {
                            q.set(state.pull.as_ref().unwrap().duration);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    self.sinkpad.peer_query(query)
                }
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SccParse {
    const NAME: &'static str = "GstSccParse";
    type Type = super::SccParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .activate_function(|pad, parent| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::loggable_error!(CAT, "Panic activating sink pad")),
                    |parse| parse.sink_activate(pad),
                )
            })
            .activatemode_function(|pad, parent, mode, active| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating sink pad with mode"
                        ))
                    },
                    |parse| parse.sink_activatemode(pad, mode, active),
                )
            })
            .chain_function(|pad, parent, buffer| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                SccParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_query(pad, query),
                )
            })
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
        }
    }
}

impl ObjectImpl for SccParse {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for SccParse {}

impl ElementImpl for SccParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
            "Scc Parse",
            "Parser/ClosedCaption",
            "Parses SCC Closed Caption Files",
            "Sebastian Dröge <sebastian@centricular.com>, Jordan Petridis <jordan@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .field(
                    "framerate",
                    gst::List::new([gst::Fraction::new(30000, 1001), gst::Fraction::new(30, 1)]),
                )
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-scc").build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused | gst::StateChange::PausedToReady => {
                // Reset the whole state
                let mut state = self.state.lock().unwrap();
                *state = State::default();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
