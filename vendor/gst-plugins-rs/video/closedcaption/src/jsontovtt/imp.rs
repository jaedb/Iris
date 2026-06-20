// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2021 Jan Schmidt <jan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use crate::jsontovtt::fku::ForceKeyUnitRequest;
use crate::ttutils::Lines;

use std::sync::LazyLock;

use std::collections::{BinaryHeap, VecDeque};
use std::sync::Mutex;

#[derive(Clone, Debug)]
struct TimestampedLines {
    lines: Lines,
    pts: gst::ClockTime,
    duration: gst::ClockTime,
    line_running_time: Option<gst::ClockTime>,
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    timeout: Option<gst::ClockTime>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            timeout: gst::ClockTime::NONE,
        }
    }
}

struct State {
    pending: VecDeque<TimestampedLines>,
    need_initial_header: bool,
    last_pts: Option<gst::ClockTime>,

    keyunit_requests: BinaryHeap<ForceKeyUnitRequest>,
    segment: gst::FormattedSegment<gst::ClockTime>,
    settings: Settings,
}

impl Default for State {
    fn default() -> Self {
        State {
            pending: VecDeque::new(),
            need_initial_header: true,
            last_pts: None,
            keyunit_requests: BinaryHeap::new(),
            segment: gst::FormattedSegment::new(),
            settings: Settings::default(),
        }
    }
}

pub struct JsonToVtt {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,

    state: Mutex<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "jsontovtt",
        gst::DebugColorFlags::empty(),
        Some("JSON to WebVTT"),
    )
});

fn clamp(
    segment: &gst::FormattedSegment<gst::ClockTime>,
    mut pts: gst::ClockTime,
    mut duration: Option<gst::ClockTime>,
) -> Option<(gst::ClockTime, Option<gst::ClockTime>)> {
    let end_pts = pts.opt_add(duration).unwrap_or(pts);

    if let Some(segment_start) = segment.start() {
        if end_pts < segment_start {
            return None;
        }

        if pts < segment_start {
            duration.opt_sub_assign(segment_start - pts);
            pts = segment_start;
        }
    }

    if let Some(segment_stop) = segment.stop() {
        if pts > segment_stop {
            return None;
        }

        if end_pts > segment_stop {
            duration.opt_sub_assign(end_pts - segment_stop);
        }
    }

    Some((pts, duration))
}

impl State {
    fn create_vtt_header(timestamp: gst::ClockTime) -> gst::Buffer {
        let mut buffer = gst::Buffer::from_slice(String::from("WEBVTT\n\n").into_bytes());
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(timestamp);
        }

        buffer
    }

    fn split_time(time: gst::ClockTime) -> (u64, u8, u8, u16) {
        let time = time.nseconds();

        let mut s = time / 1_000_000_000;
        let mut m = s / 60;
        let h = m / 60;
        s %= 60;
        m %= 60;
        let ns = time % 1_000_000_000;

        (h, m as u8, s as u8, (ns / 1_000_000) as u16)
    }

    fn create_vtt_buffer(
        timestamp: gst::ClockTime,
        duration: gst::ClockTime,
        text: &str,
    ) -> Option<gst::Buffer> {
        use std::fmt::Write;

        let mut data = String::new();

        let (h1, m1, s1, ms1) = Self::split_time(timestamp);
        let (h2, m2, s2, ms2) = Self::split_time(timestamp + duration);

        // Rounding up to the millisecond and clamping to fragment duration
        // might result in zero-duration cues, which we skip as some players
        // interpret those in a special way
        if h1 == h2 && m1 == m2 && s1 == s2 && ms1 == ms2 {
            return None;
        }

        writeln!(
            &mut data,
            "{h1:02}:{m1:02}:{s1:02}.{ms1:03} --> {h2:02}:{m2:02}:{s2:02}.{ms2:03}"
        )
        .unwrap();
        writeln!(&mut data, "{text}").unwrap();

        let mut buffer = gst::Buffer::from_mut_slice(data.into_bytes());
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(timestamp);
            buffer.set_duration(duration);
            buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
        }

        Some(buffer)
    }

    fn check_initial_header(&mut self, pts: gst::ClockTime) -> Option<gst::Buffer> {
        if self.need_initial_header {
            let ret = Self::create_vtt_header(pts);
            self.need_initial_header = false;
            Some(ret)
        } else {
            None
        }
    }

    fn drain(
        &mut self,
        imp: &JsonToVtt,
        buffers: &mut Vec<gst::Buffer>,
        running_time: Option<gst::ClockTime>,
    ) {
        /* We don't output anything until we've received the first request, we trigger
         * that first request by sending a first header buffer for each new fragment.
         *
         * In practice, we will never hold more than one request at a time, but handling
         * queuing gracefully doesn't hurt.
         */
        while let Some(fku) = self
            .keyunit_requests
            .peek()
            .filter(|fku| match running_time {
                None => true,
                Some(running_time) => fku.running_time <= running_time,
            })
        {
            let mut drained_lines: VecDeque<TimestampedLines> = VecDeque::new();

            /* Collect cues, fixing up their duration based on the next cue */
            while let Some(lines) = self.pending.front() {
                if let Some(drained_line) = drained_lines.back_mut() {
                    drained_line.duration = lines.pts - drained_line.pts;
                }

                if running_time.is_none()
                    || self.segment.to_running_time(lines.pts) <= Some(fku.running_time)
                {
                    drained_lines.push_back(self.pending.pop_front().unwrap());
                } else {
                    break;
                }
            }

            /* cues that end a fragment must be clipped and cloned for the next fragment */
            if let Some(drained_line) = drained_lines.back_mut() {
                /* Clip to either the requested PTS, or segment stop if specified */
                let end_pts = if running_time.is_none() {
                    self.last_pts.unwrap()
                } else {
                    self.segment
                        .position_from_running_time(fku.running_time)
                        .unwrap_or_else(|| self.segment.stop().unwrap())
                };

                if let (Some(line_running_time), Some(running_time)) =
                    (drained_line.line_running_time, running_time)
                {
                    if self
                        .settings
                        .timeout
                        .is_none_or(|timeout| running_time < line_running_time + timeout)
                    {
                        let mut cloned = drained_line.clone();
                        cloned.pts = end_pts;
                        self.pending.push_front(cloned);
                    } else {
                        gst::debug!(
                            CAT,
                            imp = imp,
                            "Reached timeout, clearing line running time {}, cur running time {}",
                            line_running_time,
                            running_time
                        );
                    }
                }
                drained_line.duration = end_pts - drained_line.pts;
            }

            /* We have gathered, clipped and timestamped all cues, output them now */
            for lines in drained_lines {
                let mut output_text = String::new();

                for line in &lines.lines.lines {
                    for chunk in &line.chunks {
                        output_text += &chunk.text;
                    }
                    output_text += "\n";
                }

                // No need to output an explicit cue for eg clear buffers
                if !output_text.is_empty() {
                    let mut buf = Self::create_vtt_buffer(lines.pts, lines.duration, &output_text);

                    if let Some(buf) = buf.take() {
                        buffers.push(buf);
                    } else {
                        gst::debug!(
                            CAT,
                            imp = imp,
                            "Dropping empty duration cue, pts: {}, text: {}",
                            lines.pts,
                            output_text
                        );
                    }
                }
            }

            // Now open the next fragment, if it lies inside the segment
            if let Some(pts) = self.segment.position_from_running_time(fku.running_time) {
                buffers.push(Self::create_vtt_header(pts));
            }

            self.keyunit_requests.pop();
        }
    }

    fn handle_buffer(
        &mut self,
        imp: &JsonToVtt,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<Vec<gst::Buffer>, gst::FlowError> {
        let mut ret = vec![];

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let lines: Lines = serde_json::from_slice(&data).map_err(|err| {
            gst::error!(CAT, obj = pad, "Failed to parse input as json: {}", err);

            gst::FlowError::Error
        })?;

        let pts = buffer.pts().ok_or_else(|| {
            gst::error!(CAT, obj = pad, "Require timestamped buffers");
            gst::FlowError::Error
        })?;

        let duration = buffer.duration().ok_or_else(|| {
            gst::error!(CAT, obj = pad, "Require buffers with duration");
            gst::FlowError::Error
        })?;

        let (pts, duration) = match clamp(&self.segment, pts, Some(duration)) {
            Some((pts, duration)) => (pts, duration.unwrap()),
            None => {
                gst::warning!(
                    CAT,
                    obj = pad,
                    "Dropping buffer outside segment: {:?}",
                    buffer
                );
                return Ok(ret);
            }
        };

        if let Some(buffer) = self.check_initial_header(pts) {
            ret.push(buffer);
        }

        let line_running_time = self.segment.to_running_time(pts);

        self.pending.push_back(TimestampedLines {
            lines,
            pts,
            duration,
            line_running_time,
        });

        self.drain(imp, &mut ret, line_running_time);

        self.last_pts = Some(pts + duration);

        Ok(ret)
    }

    fn handle_gap(&mut self, imp: &JsonToVtt, gap: &gst::event::Gap) -> Vec<gst::Buffer> {
        let mut ret = vec![];

        let (pts, duration) = gap.get();

        let (pts, duration) = match clamp(&self.segment, pts, duration) {
            Some((pts, duration)) => (pts, duration),
            None => {
                gst::warning!(CAT, imp = imp, "Ignoring gap outside segment");
                return ret;
            }
        };

        if let Some(buffer) = self.check_initial_header(pts) {
            ret.push(buffer);
        }

        self.drain(imp, &mut ret, self.segment.to_running_time(pts));

        self.last_pts = Some(pts).opt_add(duration).or(Some(pts));

        ret
    }

    fn handle_eos(&mut self, imp: &JsonToVtt) -> Vec<gst::Buffer> {
        let mut ret = vec![];

        gst::log!(CAT, imp = imp, "handling EOS, {}", self.pending.len());
        self.drain(imp, &mut ret, None);

        ret
    }
}

impl JsonToVtt {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);
        let mut state = self.state.lock().unwrap();

        let buffers = state.handle_buffer(self, pad, buffer)?;
        drop(state);
        self.output(buffers)?;

        Ok(gst::FlowSuccess::Ok)
    }

    fn handle_fku(&self, fku: ForceKeyUnitRequest) {
        let mut state = self.state.lock().unwrap();

        state.keyunit_requests.push(fku);
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::CustomUpstream(ev) => {
                if gst_video::ForceKeyUnitEvent::is(ev) {
                    match gst_video::UpstreamForceKeyUnitEvent::parse(ev) {
                        Ok(fku_event) => {
                            gst::log!(CAT, obj = pad, "Handling fku {:?}", fku_event);

                            if fku_event.running_time.is_some() {
                                self.handle_fku(ForceKeyUnitRequest::new_from_event(&fku_event));
                            }
                        }
                        Err(_) => gst::warning!(
                            CAT,
                            imp = self,
                            "Invalid force-key-unit event received from downstream: {:?}",
                            &ev
                        ),
                    }
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event);
                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Eos(..) => {
                gst::log!(CAT, obj = pad, "Handling EOS");
                let mut state = self.state.lock().unwrap();
                let buffers = state.handle_eos(self);
                drop(state);
                let _ = self.output(buffers);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Caps(..) => {
                let mut downstream_caps = match self.srcpad.allowed_caps() {
                    None => self.srcpad.pad_template_caps(),
                    Some(caps) => caps,
                };

                if downstream_caps.is_empty() {
                    gst::error!(CAT, obj = pad, "Empty downstream caps");
                    return false;
                }

                downstream_caps.fixate();

                gst::debug!(
                    CAT,
                    obj = pad,
                    "Negotiating for downstream caps {}",
                    downstream_caps
                );

                let s = downstream_caps.structure(0).unwrap();
                let new_caps = if s.name() == "application/x-subtitle-vtt-fragmented" {
                    gst::Caps::builder("application/x-subtitle-vtt-fragmented")
                        .field("inline-headers", true)
                        .build()
                } else {
                    unreachable!();
                };

                let new_event = gst::event::Caps::new(&new_caps);

                self.srcpad.push_event(new_event)
            }
            EventView::Segment(ev) => {
                let mut state = self.state.lock().unwrap();

                match ev.segment().clone().downcast::<gst::format::Time>() {
                    Ok(s) => {
                        state.segment = s;
                    }
                    Err(err) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Failed,
                            ["Time segment needed: {:?}", err]
                        );
                        return false;
                    }
                };

                /* FIXME: Handle segment updates by draining? */
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Gap(ev) => {
                gst::log!(CAT, obj = pad, "Handling gap {:?}", ev);
                let mut state = self.state.lock().unwrap();
                let buffers = state.handle_gap(self, ev);
                drop(state);
                let _ = self.output(buffers);
                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn output(&self, mut buffers: Vec<gst::Buffer>) -> Result<gst::FlowSuccess, gst::FlowError> {
        for buf in buffers.drain(..) {
            self.srcpad.push(buf)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for JsonToVtt {
    const NAME: &'static str = "GstJsonToVtt";
    type Type = super::JsonToVtt;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                JsonToVtt::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                JsonToVtt::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::FIXED_CAPS)
            .event_function(|pad, parent, event| {
                JsonToVtt::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.src_event(pad, event),
                )
            })
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for JsonToVtt {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecUInt64::builder("timeout")
                .nick("Timeout")
                .blurb("Duration after which to erase text when no data has arrived")
                .minimum(16.seconds().nseconds())
                .default_value(u64::MAX)
                .mutable_playing()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();

                let timeout = value.get().expect("type checked upstream");

                settings.timeout = match timeout {
                    u64::MAX => gst::ClockTime::NONE,
                    _ => Some(timeout.nseconds()),
                };
                state.settings.timeout = settings.timeout;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                if let Some(timeout) = settings.timeout {
                    timeout.nseconds().to_value()
                } else {
                    u64::MAX.to_value()
                }
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for JsonToVtt {}

impl ElementImpl for JsonToVtt {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "JSON to WebVTT",
                "Generic",
                "Converts JSON to WebVTT",
                "Jan Schmidt <jan@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-json")
                .field("format", "cea608")
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-subtitle-vtt-fragmented")
                .field("inline-headers", true)
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
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

        if transition == gst::StateChange::ReadyToPaused {
            let settings = self.settings.lock().unwrap();
            let mut state = self.state.lock().unwrap();
            *state = State::default();
            state.settings = *settings;
        }

        self.parent_change_state(transition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_clamp() {
        gst::init().unwrap();

        let segment = gst::FormattedSegment::<gst::ClockTime>::new();

        let pts = gst::ClockTime::ZERO;
        let duration = Some(10.nseconds());

        assert_eq!(
            clamp(&segment, pts, duration),
            Some((gst::ClockTime::ZERO, Some(10.nseconds())))
        );
    }

    #[test]
    fn test_clamp_start() {
        gst::init().unwrap();

        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_start(2.nseconds());

        let pts = gst::ClockTime::ZERO;
        let duration = Some(10.nseconds());

        assert_eq!(
            clamp(&segment, pts, duration),
            Some((2.nseconds(), Some(8.nseconds())))
        );
    }

    #[test]
    fn test_clamp_stop() {
        gst::init().unwrap();

        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_stop(7.nseconds());

        let pts = gst::ClockTime::ZERO;
        let duration = Some(10.nseconds());

        assert_eq!(
            clamp(&segment, pts, duration),
            Some((gst::ClockTime::ZERO, Some(7.nseconds())))
        );
    }

    #[test]
    fn test_clamp_start_stop() {
        gst::init().unwrap();

        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_start(2.nseconds());
        segment.set_stop(7.nseconds());

        let pts = gst::ClockTime::ZERO;
        let duration = Some(10.nseconds());

        assert_eq!(
            clamp(&segment, pts, duration),
            Some((2.nseconds(), Some(5.nseconds())))
        );
    }

    #[test]
    fn test_clamp_before() {
        gst::init().unwrap();

        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_start(15.nseconds());

        let pts = gst::ClockTime::ZERO;
        let duration = Some(10.nseconds());

        assert_eq!(clamp(&segment, pts, duration), None);
    }

    #[test]
    fn test_clamp_after() {
        gst::init().unwrap();

        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_stop(10.nseconds());

        let pts = 15.nseconds();
        let duration = Some(10.nseconds());

        assert_eq!(clamp(&segment, pts, duration), None);
    }
}
