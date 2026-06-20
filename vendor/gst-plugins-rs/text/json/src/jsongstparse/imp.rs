// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::cmp;
use std::sync::{Mutex, MutexGuard};

use serde::Deserialize;

use crate::line_reader::LineReader;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "jsongstparse",
        gst::DebugColorFlags::empty(),
        Some("GStreamer Json Parser Element"),
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
    fn new(imp: &JsonGstParse, pad: &gst::Pad) -> Self {
        Self {
            need_stream_start: true,
            stream_id: pad.create_stream_id(&*imp.obj(), Some("src")).to_string(),
            offset: 0,
            duration: None,
        }
    }
}

#[derive(Debug)]
struct State {
    reader: LineReader<gst::MappedBuffer<gst::buffer::Readable>>,
    need_segment: bool,
    need_caps: bool,
    format: Option<String>,
    pending_events: Vec<gst::Event>,
    last_position: Option<gst::ClockTime>,
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
            need_segment: true,
            need_caps: true,
            format: None,
            pending_events: Vec::new(),
            last_position: None,
            segment: gst::FormattedSegment::<gst::ClockTime>::new(),
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

#[derive(Deserialize, Debug)]
enum Line<'a> {
    Header {
        format: String,
    },
    Buffer {
        pts: Option<gst::ClockTime>,
        duration: Option<gst::ClockTime>,
        #[serde(borrow)]
        data: &'a serde_json::value::RawValue,
    },
}

impl State {
    fn line(&mut self, drain: bool) -> Result<Option<Line<'_>>, (&[u8], serde_json::Error)> {
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

        let line: Line = serde_json::from_slice(line).map_err(|err| (line, err))?;

        Ok(Some(line))
    }

    fn create_events(&mut self, imp: &JsonGstParse) -> Vec<gst::Event> {
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

        if self.need_caps {
            let caps = gst::Caps::builder("application/x-json")
                .field_if_some("format", self.format.as_ref())
                .build();

            events.push(gst::event::Caps::new(&caps));
            gst::info!(CAT, imp = imp, "Caps changed to {:?}", &caps);
            self.need_caps = false;
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

    fn add_buffer_metadata(
        &mut self,
        buffer: &mut gst::buffer::Buffer,
        pts: Option<gst::ClockTime>,
        duration: Option<gst::ClockTime>,
    ) {
        let buffer = buffer.get_mut().unwrap();

        self.last_position = pts.opt_add(duration);

        buffer.set_pts(pts);

        if self.discont {
            buffer.set_flags(gst::BufferFlags::DISCONT);
            self.discont = false;
        }

        buffer.set_duration(duration);
    }
}

pub struct JsonGstParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

impl JsonGstParse {
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
            let seeking = state.seeking;
            let line = state.line(drain);
            match line {
                Ok(Some(Line::Buffer {
                    pts,
                    duration,
                    data,
                })) => {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Got buffer with timestamp {} and duration {}",
                        pts.display(),
                        duration.display(),
                    );

                    if !seeking {
                        let data = data.to_string();
                        let mut events = state.create_events(self);

                        let mut buffer = gst::Buffer::from_mut_slice(data.into_bytes());

                        if let Some(last_position) = state.last_position {
                            if let Ok(Some(duration)) = pts.opt_checked_sub(last_position) {
                                events.push(
                                    gst::event::Gap::builder(last_position)
                                        .duration(duration)
                                        .build(),
                                );
                            }
                        }

                        state.add_buffer_metadata(&mut buffer, pts, duration);

                        let send_eos = buffer
                            .pts()
                            .opt_add(buffer.duration())
                            .opt_ge(state.segment.stop())
                            .unwrap_or(false);

                        // Drop our state mutex while we push out buffers or events
                        drop(state);

                        for event in events {
                            gst::debug!(CAT, imp = self, "Pushing event {:?}", event);
                            self.srcpad.push_event(event);
                        }

                        self.srcpad.push(buffer).inspect_err(|&err| {
                            if err != gst::FlowError::Flushing {
                                gst::error!(CAT, imp = self, "Pushing buffer returned {:?}", err);
                            }
                        })?;

                        if send_eos {
                            return Err(gst::FlowError::Eos);
                        }

                        state = self.state.lock().unwrap();
                    } else {
                        state = self.handle_skipped_line(pts, state);
                    }
                }
                Ok(Some(Line::Header { format })) => {
                    if state.format.is_none() {
                        state.format = Some(format);
                    } else {
                        gst::warning!(CAT, imp = self, "Ignoring format change",);
                    }
                }
                Err((line, err)) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Couldn't parse line '{:?}': {:?}",
                        std::str::from_utf8(line),
                        err
                    );

                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Decode,
                        ["Couldn't parse line '{:?}': {:?}", line, err]
                    );

                    break Err(gst::FlowError::Error);
                }
                Ok(None) => {
                    if drain && state.pull.is_some() {
                        gst::debug!(CAT, imp = self, "Finished draining");
                        break Err(gst::FlowError::Eos);
                    }
                    break Ok(gst::FlowSuccess::Ok);
                }
            }
        }
    }

    fn handle_skipped_line(
        &self,
        pts: impl Into<Option<gst::ClockTime>>,
        mut state: MutexGuard<State>,
    ) -> MutexGuard<'_, State> {
        if pts.into().opt_ge(state.segment.start()).unwrap_or(false) {
            state.seeking = false;
            state.discont = true;
            state.replay_last_line = true;
            state.need_flush_stop = true;

            gst::debug!(CAT, imp = self, "Done seeking");
        }

        drop(state);

        self.state.lock().unwrap()
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
        let self_ = self.ref_counted();
        let res = self.sinkpad.start_task(move || {
            self_.loop_fn();
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

    fn scan_duration(&self) -> Result<Option<gst::ClockTime>, gst::FlowError> {
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
        let mut last_pts = None;

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

            for buf in buffers.iter().rev() {
                let buf = buf.clone().into_mapped_buffer_readable().map_err(|_| {
                    gst::error!(CAT, imp = self, "Failed to map buffer readable");
                    gst::FlowError::Error
                })?;

                reader.push(buf);
            }

            while let Some(line) = reader.line_with_drain(true) {
                if let Ok(Line::Buffer {
                    pts,
                    duration,
                    data: _data,
                }) = serde_json::from_slice(line)
                {
                    last_pts = pts.opt_add(duration);
                }
            }

            if last_pts.is_some() || offset == 0 {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Duration scan done, last_pts: {:?}",
                    last_pts
                );
                break Ok(last_pts);
            }
        }
    }

    fn push_eos(&self) {
        let mut state = self.state.lock().unwrap();

        if state.seeking {
            state.need_flush_stop = true;
        }

        let mut events = state.create_events(self);
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
        let State { ref mut pull, .. } = *state;
        let pull = pull.as_mut().unwrap();
        let offset = pull.offset;
        let scan_duration = pull.duration.is_none();

        pull.offset += 4096;

        drop(state);

        if scan_duration {
            let duration = match self.scan_duration() {
                Ok(Some(pts)) => pts,
                Ok(None) => gst::ClockTime::ZERO,
                Err(gst::FlowError::Flushing) => {
                    gst::debug!(CAT, imp = self, "Failed to scan duration, reason: flushing");

                    self.sinkpad.pause_task().unwrap();
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

                    self.sinkpad.pause_task().unwrap();
                    return;
                }
            };

            let mut state = self.state.lock().unwrap();
            let pull = state.pull.as_mut().unwrap();
            pull.duration = Some(duration);
        }

        let buffer = match self.sinkpad.pull_range(offset, 4096) {
            Ok(buffer) => Some(buffer),
            Err(gst::FlowError::Eos) => None,
            Err(gst::FlowError::Flushing) => {
                gst::debug!(
                    CAT,
                    obj = self.sinkpad,
                    "Pausing after pulling buffer, reason: flushing"
                );

                self.sinkpad.pause_task().unwrap();
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

                self.sinkpad.pause_task().unwrap();
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

            self.sinkpad.pause_task().unwrap();
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

    fn flush(&self, state: &mut State) {
        state.reader.clear();
        if let Some(pull) = &mut state.pull {
            pull.offset = 0;
        }
        state.segment = gst::FormattedSegment::<gst::ClockTime>::new();
        state.need_segment = true;
        state.need_caps = true;
        state.pending_events.clear();
        state.last_position = None;
        state.last_raw_line = [].to_vec();
        state.format = None;
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
                let mut state = self.state.lock().unwrap();
                self.flush(&mut state);
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

        self.sinkpad.pause_task().unwrap();

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

        self.flush(&mut state);

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
impl ObjectSubclass for JsonGstParse {
    const NAME: &'static str = "GstJsonGstParse";
    type Type = super::JsonGstParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .activate_function(|pad, parent| {
                JsonGstParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::loggable_error!(CAT, "Panic activating sink pad")),
                    |parse| parse.sink_activate(pad),
                )
            })
            .activatemode_function(|pad, parent, mode, active| {
                JsonGstParse::catch_panic_pad_function(
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
                JsonGstParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                JsonGstParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                JsonGstParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                JsonGstParse::catch_panic_pad_function(
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

impl ObjectImpl for JsonGstParse {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for JsonGstParse {}

impl ElementImpl for JsonGstParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "JSON GStreamer parser",
                "Parser/JSON",
                "Parses ndjson as output by jsongstenc",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-json").build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::new_any();
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
