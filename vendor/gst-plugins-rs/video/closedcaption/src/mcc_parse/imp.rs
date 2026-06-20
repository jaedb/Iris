// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_video::ValidVideoTimeCode;

use std::cmp;
use std::sync::{Mutex, MutexGuard};

use std::sync::LazyLock;

use super::parser::{MccLine, MccParser};
use crate::line_reader::LineReader;
use crate::parser_utils::TimeCode;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "mccparse",
        gst::DebugColorFlags::empty(),
        Some("Mcc Parser Element"),
    )
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Cea708Cdp,
    Cea608,
}

#[derive(Debug)]
struct PullState {
    need_stream_start: bool,
    stream_id: String,
    offset: u64,
    duration: Option<gst::ClockTime>,
}

impl PullState {
    fn new(imp: &MccParse, pad: &gst::Pad) -> Self {
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
    parser: MccParser,
    format: Option<Format>,
    need_segment: bool,
    pending_events: Vec<gst::Event>,
    last_position: Option<gst::ClockTime>,
    last_timecode: Option<gst_video::ValidVideoTimeCode>,
    timecode_rate: Option<(u8, bool)>,
    segment: gst::FormattedSegment<gst::ClockTime>,

    // Pull mode
    pull: Option<PullState>,

    // seeking
    seeking: bool,
    discont: bool,
    seek_seqnum: Option<gst::Seqnum>,
    last_raw_line: Vec<u8>,
    replay_last_line: bool,
    need_flush_stop: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            reader: LineReader::new(),
            parser: MccParser::new(),
            format: None,
            need_segment: true,
            pending_events: Vec::new(),
            last_position: None,
            last_timecode: None,
            timecode_rate: None,
            segment: gst::FormattedSegment::new(),
            pull: None,
            seeking: false,
            discont: false,
            seek_seqnum: None,
            last_raw_line: Vec::new(),
            replay_last_line: false,
            need_flush_stop: false,
        }
    }
}

fn parse_timecode(
    framerate: gst::Fraction,
    drop_frame: bool,
    tc: TimeCode,
) -> Result<ValidVideoTimeCode, gst::FlowError> {
    let timecode = gst_video::VideoTimeCode::new(
        framerate,
        None,
        if drop_frame {
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

    timecode.try_into().map_err(|_| gst::FlowError::Error)
}

fn parse_timecode_rate(
    timecode_rate: Option<(u8, bool)>,
) -> Result<(gst::Fraction, bool), gst::FlowError> {
    let (framerate, drop_frame) = match timecode_rate {
        Some((rate, false)) => (gst::Fraction::new(rate as i32, 1), false),
        Some((rate, true)) => (gst::Fraction::new(rate as i32 * 1000, 1001), true),
        None => {
            return Err(gst::FlowError::Error);
        }
    };

    Ok((framerate, drop_frame))
}

impl State {
    #[allow(clippy::type_complexity)]
    fn line(
        &mut self,
        drain: bool,
    ) -> Result<Option<MccLine<'_>>, (&[u8], winnow::error::ContextError)> {
        let line = if self.replay_last_line {
            self.replay_last_line = false;
            &self.last_raw_line
        } else {
            match self.reader.line_with_drain(drain) {
                None => {
                    return Ok(None);
                }
                Some(line) => {
                    self.last_raw_line = line.to_vec();
                    line
                }
            }
        };

        self.parser
            .parse_line(line, !self.seeking)
            .map(Option::Some)
            .map_err(|err| (line, err))
    }

    fn handle_timecode(
        &mut self,
        imp: &MccParse,
        framerate: gst::Fraction,
        drop_frame: bool,
        tc: TimeCode,
    ) -> Result<ValidVideoTimeCode, gst::FlowError> {
        match parse_timecode(framerate, drop_frame, tc) {
            Ok(timecode) => Ok(timecode),
            Err(timecode) => {
                let last_timecode = self.last_timecode.clone().ok_or_else(|| {
                    gst::element_imp_error!(
                        imp,
                        gst::StreamError::Decode,
                        ["Invalid first timecode {:?}", timecode]
                    );

                    gst::FlowError::Error
                })?;

                gst::warning!(
                    CAT,
                    imp = imp,
                    "Invalid timecode {:?}, using previous {:?}",
                    timecode,
                    last_timecode
                );

                Ok(last_timecode)
            }
        }
    }

    /// Calculate a timestamp from the timecode and make sure to
    /// not produce timestamps jumping backwards
    fn update_timestamp(&mut self, imp: &MccParse, timecode: &gst_video::ValidVideoTimeCode) {
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
        imp: &MccParse,
        buffer: &mut gst::buffer::Buffer,
        timecode: &gst_video::ValidVideoTimeCode,
        framerate: gst::Fraction,
    ) {
        let buffer = buffer.get_mut().unwrap();
        gst_video::VideoTimeCodeMeta::add(buffer, timecode);

        self.update_timestamp(imp, timecode);

        buffer.set_pts(self.last_position);

        if self.discont {
            buffer.set_flags(gst::BufferFlags::DISCONT);
            self.discont = false;
        }

        buffer.set_duration(
            gst::ClockTime::SECOND.mul_div_ceil(framerate.denom() as u64, framerate.numer() as u64),
        );
    }

    fn create_events(
        &mut self,
        imp: &MccParse,
        format: Option<Format>,
        framerate: gst::Fraction,
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

        if let Some(format) = format {
            if self.format != Some(format) {
                self.format = Some(format);

                let caps = match format {
                    Format::Cea708Cdp => gst::Caps::builder("closedcaption/x-cea-708")
                        .field("format", "cdp")
                        .field("framerate", framerate)
                        .build(),
                    Format::Cea608 => gst::Caps::builder("closedcaption/x-cea-608")
                        .field("format", "s334-1a")
                        .field("framerate", framerate)
                        .build(),
                };

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

pub struct MccParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

#[derive(Debug)]
struct OffsetVec {
    vec: Vec<u8>,
    offset: usize,
    len: usize,
}

impl AsRef<[u8]> for OffsetVec {
    fn as_ref(&self) -> &[u8] {
        &self.vec[self.offset..(self.offset + self.len)]
    }
}

impl AsMut<[u8]> for OffsetVec {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.vec[self.offset..(self.offset + self.len)]
    }
}

impl MccParse {
    fn handle_buffer(
        &self,
        buffer: Option<gst::Buffer>,
        scan_tc_rate: bool,
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
                Ok(Some(MccLine::Caption(tc, Some(data)))) => {
                    assert!(!state.seeking);

                    gst::debug!(
                        CAT,
                        imp = self,
                        "Got caption buffer with timecode {:?} and size {}",
                        tc,
                        data.len()
                    );

                    if scan_tc_rate {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Decode,
                            ["Found caption line while scanning for timecode rate"]
                        );

                        break Err(gst::FlowError::Error);
                    }

                    if data.len() < 3 {
                        gst::debug!(CAT, imp = self, "Too small caption packet: {}", data.len(),);
                        continue;
                    }

                    let format = match (data[0], data[1]) {
                        (0x61, 0x01) => Format::Cea708Cdp,
                        (0x61, 0x02) => Format::Cea608,
                        (did, sdid) => {
                            gst::debug!(CAT, imp = self, "Unknown DID {:x} SDID {:x}", did, sdid);
                            continue;
                        }
                    };

                    let len = data[2];
                    if data.len() < 3 + len as usize {
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Too small caption packet: {} < {}",
                            data.len(),
                            3 + len,
                        );
                        continue;
                    }

                    state = self.handle_line(tc, data, format, state)?;
                }
                Ok(Some(MccLine::Caption(tc, None))) => {
                    assert!(state.seeking);

                    if scan_tc_rate {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Decode,
                            ["Found caption line while scanning for timecode rate"]
                        );

                        break Err(gst::FlowError::Error);
                    }

                    state = self.handle_skipped_line(tc, state)?;
                }
                Ok(Some(MccLine::TimeCodeRate(rate, df))) => {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Got timecode rate {} (drop frame {})",
                        rate,
                        df
                    );
                    state.timecode_rate = Some((rate, df));

                    if scan_tc_rate {
                        break Ok(gst::FlowSuccess::Ok);
                    }
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
                    if scan_tc_rate {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Decode,
                            ["Found end of input while scanning for timecode rate"]
                        );

                        break Err(gst::FlowError::Error);
                    }

                    if drain && state.pull.is_some() {
                        break Err(gst::FlowError::Eos);
                    }
                    break Ok(gst::FlowSuccess::Ok);
                }
            }
        }
    }

    fn handle_skipped_line(
        &self,
        tc: TimeCode,
        mut state: MutexGuard<'_, State>,
    ) -> Result<MutexGuard<'_, State>, gst::FlowError> {
        let (framerate, drop_frame) = parse_timecode_rate(state.timecode_rate)?;
        let timecode = state.handle_timecode(self, framerate, drop_frame, tc)?;
        let nsecs = timecode.time_since_daily_jam();

        state.last_timecode = Some(timecode);

        if state
            .segment
            .start()
            .is_some_and(|seg_start| nsecs >= seg_start)
        {
            state.seeking = false;
            state.discont = true;
            state.replay_last_line = true;
            state.need_flush_stop = true;
        }

        drop(state);

        Ok(self.state.lock().unwrap())
    }

    fn handle_line(
        &self,
        tc: TimeCode,
        data: Vec<u8>,
        format: Format,
        mut state: MutexGuard<'_, State>,
    ) -> Result<MutexGuard<'_, State>, gst::FlowError> {
        let (framerate, drop_frame) = parse_timecode_rate(state.timecode_rate)?;
        let events = state.create_events(self, Some(format), framerate);
        let timecode = state.handle_timecode(self, framerate, drop_frame, tc)?;

        let len = data[2] as usize;
        let mut buffer = gst::Buffer::from_mut_slice(OffsetVec {
            vec: data,
            offset: 3,
            len,
        });

        state.add_buffer_metadata(self, &mut buffer, &timecode, framerate);

        // Update the last_timecode to the current one
        state.last_timecode = Some(timecode);

        let send_eos = state
            .segment
            .stop()
            .opt_lt(buffer.pts().opt_add(buffer.duration()))
            .unwrap_or(false);

        // Drop our state mutex while we push out buffers or events
        drop(state);

        for event in events {
            gst::debug!(CAT, imp = self, "Pushing event {:?}", event);
            self.srcpad.push_event(event);
        }

        self.srcpad.push(buffer).inspect_err(|&err| {
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

    fn scan_duration(&self) -> Result<Option<ValidVideoTimeCode>, gst::FlowError> {
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
            let mut parser = MccParser::new_scan_captions();

            for buf in buffers.iter().rev() {
                let buf = buf.clone().into_mapped_buffer_readable().map_err(|_| {
                    gst::error!(CAT, imp = self, "Failed to map buffer readable");
                    gst::FlowError::Error
                })?;

                reader.push(buf);
            }

            while let Some(line) = reader.line_with_drain(true) {
                if let Ok(MccLine::Caption(tc, None)) =
                    parser.parse_line(line, false).map_err(|err| (line, err))
                {
                    let state = self.state.lock().unwrap();
                    let (framerate, drop_frame) = parse_timecode_rate(state.timecode_rate)
                        .inspect_err(|flow| {
                            gst::error!(
                                CAT,
                                imp = self,
                                "Failed to parse timecode rate: {:?}",
                                flow
                            );
                        })?;
                    if let Ok(mut timecode) = parse_timecode(framerate, drop_frame, tc) {
                        /* We're looking for the total duration */
                        timecode.increment_frame();
                        last_tc = Some(timecode);
                    }
                }
                /* We ignore everything else including errors */
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

        match parse_timecode_rate(state.timecode_rate) {
            Ok((framerate, _)) => {
                let mut events = state.create_events(self, None, framerate);
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
            Err(_) => {
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Failed,
                    ["Streaming stopped, failed to parse timecode rate"]
                );
            }
        }
    }

    fn loop_fn(&self) {
        let mut state = self.state.lock().unwrap();
        let State {
            ref timecode_rate,
            ref mut pull,
            ..
        } = *state;
        let pull = pull.as_mut().unwrap();
        let scan_tc_rate = timecode_rate.is_none() && pull.duration.is_none();
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

        if let Err(flow) = self.handle_buffer(buffer, scan_tc_rate) {
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

        if scan_tc_rate && self.state.lock().unwrap().timecode_rate.is_some() {
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

        self.handle_buffer(Some(buffer), false)
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
        state.timecode_rate = None;
        state.last_raw_line = [].to_vec();

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
                if let Err(err) = self.handle_buffer(None, false) {
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
impl ObjectSubclass for MccParse {
    const NAME: &'static str = "GstMccParse";
    type Type = super::MccParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .activate_function(|pad, parent| {
                MccParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::loggable_error!(CAT, "Panic activating sink pad")),
                    |parse| parse.sink_activate(pad),
                )
            })
            .activatemode_function(|pad, parent, mode, active| {
                MccParse::catch_panic_pad_function(
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
                MccParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                MccParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                MccParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                MccParse::catch_panic_pad_function(
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

impl ObjectImpl for MccParse {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for MccParse {}

impl ElementImpl for MccParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Mcc Parse",
                "Parser/ClosedCaption",
                "Parses MCC Closed Caption Files",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let mut caps = gst::Caps::new_empty();
            {
                let caps = caps.get_mut().unwrap();
                let framerate = gst::FractionRange::new(
                    gst::Fraction::new(1, i32::MAX),
                    gst::Fraction::new(i32::MAX, 1),
                );

                let s = gst::Structure::builder("closedcaption/x-cea-708")
                    .field("format", "cdp")
                    .field("framerate", framerate)
                    .build();
                caps.append_structure(s);

                let s = gst::Structure::builder("closedcaption/x-cea-608")
                    .field("format", "s334-1a")
                    .field("framerate", framerate)
                    .build();
                caps.append_structure(s);
            }
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-mcc")
                .field("version", gst::List::new([1i32, 2i32]))
                .build();
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
