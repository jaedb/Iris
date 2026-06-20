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

use std::sync::Mutex;

use crate::cea608utils::Cea608Mode;
use crate::cea608utils::TextStyle;
use crate::ttutils::Chunk;
use crate::ttutils::Line;
use crate::ttutils::Lines;

use super::translate::{TextToCea608, DEFAULT_FPS_D, DEFAULT_FPS_N};

const DEFAULT_MODE: Cea608Mode = Cea608Mode::RollUp2;
const DEFAULT_ORIGIN_ROW: i32 = -1;
const DEFAULT_ORIGIN_COLUMN: u32 = 0;
const DEFAULT_SPEAKER_PREFIX: Option<&str> = None;

#[derive(Debug, Clone)]
struct Settings {
    mode: Cea608Mode,
    origin_row: i32,
    origin_column: u32,
    roll_up_timeout: Option<gst::ClockTime>,
    speaker_prefix: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: DEFAULT_MODE,
            origin_row: DEFAULT_ORIGIN_ROW,
            origin_column: DEFAULT_ORIGIN_COLUMN,
            roll_up_timeout: gst::ClockTime::NONE,
            speaker_prefix: DEFAULT_SPEAKER_PREFIX.map(String::from),
        }
    }
}

#[derive(Debug)]
struct State {
    translator: TextToCea608,
    framerate: gst::Fraction,
    json_input: bool,
    force_clear: bool,
    force_carriage_return: bool,
    prefix_newline: bool,
    max_frame_no: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            translator: TextToCea608::default(),
            framerate: gst::Fraction::new(DEFAULT_FPS_N, DEFAULT_FPS_D),
            json_input: false,
            force_clear: false,
            force_carriage_return: false,
            prefix_newline: false,
            max_frame_no: 0,
        }
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "tttocea608",
        gst::DebugColorFlags::empty(),
        Some("TT CEA 608 Element"),
    )
});

fn cc_data_buffer(
    imp: &TtToCea608,
    cc_data: [u8; 2],
    pts: gst::ClockTime,
    duration: gst::ClockTime,
) -> gst::Buffer {
    let mut ret = gst::Buffer::with_size(2).unwrap();
    let buf_mut = ret.get_mut().unwrap();

    if cc_data != [0x80, 0x80] {
        let code = cea608_types::tables::Code::from_data(cc_data);
        gst::log!(CAT, imp = imp, "{} -> {}: {:?}", pts, pts + duration, code);
    } else {
        gst::trace!(CAT, imp = imp, "{} -> {}: padding", pts, pts + duration);
    }

    buf_mut.copy_from_slice(0, &cc_data).unwrap();
    buf_mut.set_pts(pts);
    buf_mut.set_duration(duration);

    ret
}

pub struct TtToCea608 {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,

    // Ordered by locking order
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl TtToCea608 {
    fn generate(
        &self,
        state: &mut State,
        pts: gst::ClockTime,
        duration: gst::ClockTime,
        lines: Lines,
    ) {
        let (fps_n, fps_d) = {
            let f = state.translator.framerate();
            (f.numer() as u64, f.denom() as u64)
        };

        let frame_no = pts.mul_div_round(fps_n, fps_d).unwrap().seconds();

        let max_frame_no = (pts + duration)
            .mul_div_round(fps_n, fps_d)
            .unwrap()
            .seconds();

        state.translator.generate(frame_no, max_frame_no, lines);
        state.max_frame_no = max_frame_no;
    }

    fn pop_bufferlist(&self, state: &mut State) -> gst::BufferList {
        let (fps_n, fps_d) = {
            let f = state.translator.framerate();
            (f.numer() as u64, f.denom() as u64)
        };
        let mut bufferlist = gst::BufferList::new();
        let mut_list = bufferlist.get_mut().unwrap();
        while let Some(cea608) = state.translator.pop_output() {
            if cea608.frame_no > state.max_frame_no {
                gst::warning!(CAT, imp = self, "Too much text for bandwidth");
            }
            let frame_no = cea608.frame_no.min(state.max_frame_no);
            let pts = frame_no
                .mul_div_round(fps_d * gst::ClockTime::SECOND.nseconds(), fps_n)
                .unwrap()
                .nseconds();
            let next_pts = (frame_no + 1)
                .min(state.max_frame_no)
                .mul_div_round(fps_d * gst::ClockTime::SECOND.nseconds(), fps_n)
                .unwrap()
                .nseconds();

            let duration = next_pts - pts;
            mut_list.add(cc_data_buffer(self, cea608.cea608, pts, duration));
        }
        bufferlist
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, imp = self, "Handling {:?}", buffer);

        let pts = buffer.pts().ok_or_else(|| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Stream with timestamped buffers required"]
            );
            gst::FlowError::Error
        })?;

        let duration = buffer.duration().ok_or_else(|| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Buffers of stream need to have a duration"]
            );
            gst::FlowError::Error
        })?;

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        if state.force_clear {
            state.force_carriage_return = false;
        }

        let mut lines = Lines {
            lines: Vec::new(),
            mode: Some(settings.mode),
            clear: Some(state.force_clear),
        };
        state.force_clear = false;
        match state.json_input {
            false => {
                let data = std::str::from_utf8(&data).map_err(|err| {
                    gst::error!(CAT, obj = pad, "Can't decode utf8: {}", err);

                    gst::FlowError::Error
                })?;

                let phrases: Vec<&str> = data.split('\n').collect();
                let mut row = match settings.origin_row {
                    -1 => match settings.mode {
                        Cea608Mode::PopOn | Cea608Mode::PaintOn => {
                            15u32.saturating_sub(phrases.len() as u32)
                        }
                        Cea608Mode::RollUp2 | Cea608Mode::RollUp3 | Cea608Mode::RollUp4 => 14,
                    },
                    _ => settings.origin_row as u32,
                };

                for phrase in &phrases {
                    let mut text = phrase.to_string();

                    let carriage_return = if state.force_carriage_return {
                        Some(true)
                    } else {
                        None
                    };

                    state.force_carriage_return = false;

                    if state.prefix_newline {
                        if let Some(prefix) = settings.speaker_prefix.as_ref() {
                            text.insert_str(0, prefix);
                        }
                    }

                    state.prefix_newline = false;

                    lines.lines.push(Line {
                        carriage_return,
                        column: None,
                        row: Some(row),
                        chunks: vec![Chunk {
                            style: TextStyle::White,
                            underline: false,
                            text,
                        }],
                    });

                    if settings.mode == Cea608Mode::PopOn || settings.mode == Cea608Mode::PaintOn {
                        row += 1;
                    }
                }
            }
            true => {
                lines = serde_json::from_slice(&data).map_err(|err| {
                    gst::error!(CAT, obj = pad, "Failed to parse input as json: {}", err);

                    gst::FlowError::Error
                })?;
            }
        }
        drop(settings);

        self.generate(&mut state, pts, duration, lines);
        let bufferlist = self.pop_bufferlist(&mut state);

        drop(state);

        self.srcpad.push_list(bufferlist)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        use gst::EventView;

        match event.view() {
            EventView::Caps(e) => {
                let mut downstream_caps = match self.srcpad.allowed_caps() {
                    None => self.srcpad.pad_template_caps(),
                    Some(caps) => caps,
                };

                if downstream_caps.is_empty() {
                    gst::error!(CAT, obj = pad, "Empty downstream caps");
                    return false;
                }

                let caps = downstream_caps.make_mut();
                let s = caps.structure_mut(0).unwrap();

                s.fixate_field_nearest_fraction(
                    "framerate",
                    gst::Fraction::new(DEFAULT_FPS_N, DEFAULT_FPS_D),
                );
                s.fixate();

                let caps = gst::Caps::builder_full().structure(s.to_owned()).build();

                let mut state = self.state.lock().unwrap();
                let framerate = s.get::<gst::Fraction>("framerate").unwrap();
                state.framerate = framerate;
                state.translator.set_framerate(framerate);
                state.translator.set_caption_id(cea608_types::Id::CC1);

                let upstream_caps = e.caps();
                let s = upstream_caps.structure(0).unwrap();
                state.json_input = s.name() == "application/x-json";

                gst::debug!(CAT, obj = pad, "Pushing caps {}", caps);

                let new_event = gst::event::Caps::new(&caps);

                drop(state);

                self.srcpad.push_event(new_event)
            }
            EventView::Gap(e) => {
                let mut state = self.state.lock().unwrap();

                let (timestamp, duration) = e.get();

                self.generate(
                    &mut state,
                    timestamp,
                    duration.unwrap_or(gst::ClockTime::ZERO),
                    Lines::new_empty(),
                );
                let bufferlist = self.pop_bufferlist(&mut state);

                drop(state);

                let _ = self.srcpad.push_list(bufferlist);

                true
            }
            EventView::Eos(_) => {
                let mut state = self.state.lock().unwrap();
                if let Some(erase_display_frame_no) = state.translator.erase_display_frame_no() {
                    state.max_frame_no = erase_display_frame_no;
                    let last_frame_no = state.translator.last_frame_no();
                    state.translator.generate(
                        last_frame_no,
                        erase_display_frame_no,
                        Lines::new_empty(),
                    );
                    let bufferlist = self.pop_bufferlist(&mut state);

                    drop(state);

                    let _ = self.srcpad.push_list(bufferlist);
                } else {
                    drop(state);
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::FlushStop(_) => {
                let mut state = self.state.lock().unwrap();
                let settings = self.settings.lock().unwrap();
                let framerate = state.framerate;

                *state = State::default();
                state.translator.set_mode(settings.mode);
                state.translator.set_framerate(framerate);
                state
                    .translator
                    .set_origin_column(settings.origin_column as u8);
                state
                    .translator
                    .set_roll_up_timeout(settings.roll_up_timeout);
                state.translator.set_column(settings.origin_column as u8);
                state.translator.flush();

                drop(settings);
                drop(state);

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::CustomDownstream(c) => {
                let Some(s) = c.structure() else {
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                let (insert_newline, prefix_newline) = match s.name().as_str() {
                    "rstranscribe/final-transcript" => {
                        gst::debug!(CAT, imp = self, "transcript is final, inserting new line");
                        (true, false)
                    }
                    "rstranscribe/speaker-change" => {
                        gst::debug!(CAT, imp = self, "speaker change, inserting new line");
                        (true, true)
                    }
                    _ => (false, false),
                };

                if insert_newline {
                    let mut state = self.state.lock().unwrap();
                    let settings = self.settings.lock().unwrap();

                    if !state.json_input && settings.mode.is_rollup() {
                        state.force_carriage_return = true;
                        state.prefix_newline = prefix_newline;
                    }
                }

                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TtToCea608 {
    const NAME: &'static str = "GstTtToCea608";
    type Type = super::TtToCea608;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                TtToCea608::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TtToCea608::catch_panic_pad_function(
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
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for TtToCea608 {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default("mode", DEFAULT_MODE)
                    .nick("Mode")
                    .blurb("Which mode to operate in")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecInt::builder("origin-row")
                    .nick("Origin row")
                    .blurb("Origin row, (-1=automatic)")
                    .minimum(-1)
                    .maximum(14)
                    .default_value(DEFAULT_ORIGIN_ROW)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("origin-column")
                    .nick("Origin column")
                    .blurb("Origin column")
                    .maximum(31)
                    .default_value(DEFAULT_ORIGIN_COLUMN)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("roll-up-timeout")
                    .nick("Roll-Up Timeout")
                    .blurb("Duration after which to erase display memory in roll-up mode")
                    .default_value(u64::MAX)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecString::builder("speaker-prefix")
                    .nick("Speaker Prefix")
                    .blurb("The prefix to add when outputting a carriage return on speaker change")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "mode" => {
                let mut state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.mode = value.get::<Cea608Mode>().expect("type checked upstream");
                state.force_clear = true;
            }
            "origin-row" => {
                let mut state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.origin_row = value.get().expect("type checked upstream");
                state.force_clear = true;
            }
            "origin-column" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();
                settings.origin_column = value.get().expect("type checked upstream");
                state.force_clear = true;
                state.translator.set_column(settings.origin_column as u8);
            }
            "roll-up-timeout" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();

                let timeout = match value.get().expect("type checked upstream") {
                    u64::MAX => gst::ClockTime::NONE,
                    timeout => Some(timeout.nseconds()),
                };
                settings.roll_up_timeout = timeout;
                state.translator.set_roll_up_timeout(timeout);
            }
            "speaker-prefix" => {
                let mut settings = self.settings.lock().unwrap();
                settings.speaker_prefix = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "mode" => {
                let settings = self.settings.lock().unwrap();
                settings.mode.to_value()
            }
            "origin-row" => {
                let settings = self.settings.lock().unwrap();
                settings.origin_row.to_value()
            }
            "origin-column" => {
                let settings = self.settings.lock().unwrap();
                settings.origin_column.to_value()
            }
            "roll-up-timeout" => {
                let settings = self.settings.lock().unwrap();

                if let Some(timeout) = settings.roll_up_timeout {
                    timeout.nseconds().to_value()
                } else {
                    u64::MAX.to_value()
                }
            }
            "speaker-prefix" => {
                let settings = self.settings.lock().unwrap();
                settings.speaker_prefix.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TtToCea608 {}

impl ElementImpl for TtToCea608 {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "TT to CEA-608",
                "Generic",
                "Converts timed text to CEA-608 Closed Captions",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let mut caps = gst::Caps::new_empty();
            {
                let caps = caps.get_mut().unwrap();

                let s = gst::Structure::builder("text/x-raw")
                    .field("format", "utf8")
                    .build();
                caps.append_structure(s);

                let s = gst::Structure::builder("application/x-json")
                    .field("format", "cea608")
                    .build();
                caps.append_structure(s);
            }

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let framerate = gst::FractionRange::new(
                gst::Fraction::new(1, i32::MAX),
                gst::Fraction::new(i32::MAX, 1),
            );

            let caps = gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .field("framerate", framerate)
                .field("field", 0)
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

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.lock().unwrap();
                let settings = self.settings.lock().unwrap();
                let framerate = state.framerate;
                *state = State::default();
                state.force_clear = false;
                state.force_carriage_return = false;
                state.prefix_newline = false;
                state.translator.set_mode(settings.mode);
                state
                    .translator
                    .set_origin_column(settings.origin_column as u8);
                state.translator.set_framerate(framerate);
                state
                    .translator
                    .set_roll_up_timeout(settings.roll_up_timeout);
                state.translator.set_column(settings.origin_column as u8);
                state.translator.flush();
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let mut state = self.state.lock().unwrap();
                *state = State::default();
            }
            _ => (),
        }

        Ok(ret)
    }
}
