// Copyright (C) 2021 Guillaume Desmottes <guillaume@desmottes.be>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use std::sync::LazyLock;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "textahead",
        gst::DebugColorFlags::empty(),
        Some("textahead debug category"),
    )
});

struct Settings {
    n_ahead: u32,
    separator: String,
    current_attributes: String,
    ahead_attributes: String,
    buffer_start_segment: bool,
    n_previous: u32,
    previous_attributes: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            n_ahead: 1,
            separator: "\n".to_string(),
            current_attributes: "size=\"larger\"".to_string(),
            ahead_attributes: "size=\"smaller\"".to_string(),
            buffer_start_segment: false,
            n_previous: 0,
            previous_attributes: "size=\"smaller\"".to_string(),
        }
    }
}

struct Input {
    text: String,
    pts: Option<gst::ClockTime>,
    duration: Option<gst::ClockTime>,
}

#[derive(Default)]
struct State {
    previous: VecDeque<Input>,
    pending: Vec<Input>,
    done: bool,
    /// Segment for which we should send a buffer with ahead text. Only set if `Settings.buffer_start_segment` is set.
    pending_segment: Option<gst::FormattedSegment<gst::format::Time>>,
}

pub struct TextAhead {
    sink_pad: gst::Pad,
    src_pad: gst::Pad,

    state: Mutex<State>,
    settings: Mutex<Settings>,
}

#[glib::object_subclass]
impl ObjectSubclass for TextAhead {
    const NAME: &'static str = "GstTextAhead";
    type Type = super::TextAhead;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sink_pad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                TextAhead::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TextAhead::catch_panic_pad_function(
                    parent,
                    || false,
                    |imp| imp.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let src_pad = gst::Pad::from_template(&templ);

        Self {
            sink_pad,
            src_pad,
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for TextAhead {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            let default = Settings::default();

            vec![
                glib::ParamSpecUInt::builder("n-ahead")
                    .nick("n-ahead")
                    .blurb("The number of ahead text buffers to display along with the current one")
                    .default_value(default.n_ahead)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecString::builder("separator")
                    .nick("Separator")
                    .blurb("Text inserted between each text buffers")
                    .default_value(&*default.separator)
                    .mutable_playing()
                    .build(),
                // See https://docs.gtk.org/Pango/pango_markup.html for pango attributes
                glib::ParamSpecString::builder("current-attributes")
                    .nick("Current attributes")
                    .blurb("Pango span attributes to set on the text from the current buffer")
                    .default_value(&*default.current_attributes)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecString::builder("ahead-attributes")
                    .nick("Ahead attributes")
                    .blurb("Pango span attributes to set on the ahead text")
                    .default_value(&*default.ahead_attributes)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("buffer-start-segment")
                    .nick("Buffer start segment")
                    .blurb("Generate a buffer at the start of the segment with ahead text")
                    .default_value(default.buffer_start_segment)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("n-previous")
                    .nick("n-previous")
                    .blurb("The number of previous text buffers to display before the current one")
                    .default_value(default.n_previous)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecString::builder("previous-attributes")
                    .nick("Previous attributes")
                    .blurb("Pango span attributes to set on the previous text")
                    .default_value(&*default.previous_attributes)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "n-ahead" => {
                settings.n_ahead = value.get().expect("type checked upstream");
            }
            "separator" => {
                settings.separator = value.get().expect("type checked upstream");
            }
            "current-attributes" => {
                settings.current_attributes = value.get().expect("type checked upstream");
            }
            "ahead-attributes" => {
                settings.ahead_attributes = value.get().expect("type checked upstream");
            }
            "buffer-start-segment" => {
                settings.buffer_start_segment = value.get().expect("type checked upstream");
            }
            "n-previous" => {
                settings.n_previous = value.get().expect("type checked upstream");
            }
            "previous-attributes" => {
                settings.previous_attributes = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "n-ahead" => settings.n_ahead.to_value(),
            "separator" => settings.separator.to_value(),
            "current-attributes" => settings.current_attributes.to_value(),
            "ahead-attributes" => settings.ahead_attributes.to_value(),
            "buffer-start-segment" => settings.buffer_start_segment.to_value(),
            "n-previous" => settings.n_previous.to_value(),
            "previous-attributes" => settings.previous_attributes.to_value(),
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

impl GstObjectImpl for TextAhead {}

impl ElementImpl for TextAhead {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Text Ahead",
                "Text/Filter",
                "Display upcoming text buffers ahead",
                "Guillaume Desmottes <guillaume@desmottes.be>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("text/x-raw")
                .field("format", gst::List::new(["utf8", "pango-markup"]))
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("text/x-raw")
                .field("format", "pango-markup")
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let res = self.parent_change_state(transition);

        match transition {
            gst::StateChange::ReadyToPaused => *self.state.lock().unwrap() = State::default(),
            gst::StateChange::PausedToReady => {
                let mut state = self.state.lock().unwrap();
                state.done = true;
            }
            _ => {}
        }

        res
    }
}

impl TextAhead {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let pts = buffer.pts();
        let duration = buffer.duration();

        let buffer = buffer
            .into_mapped_buffer_readable()
            .map_err(|_| gst::FlowError::Error)?;
        let text =
            String::from_utf8(Vec::from(buffer.as_slice())).map_err(|_| gst::FlowError::Error)?;

        // queue buffer
        let mut state = self.state.lock().unwrap();

        gst::log!(CAT, imp = self, "input {:?}: {}", pts, text);

        state.pending.push(Input {
            text,
            pts,
            duration,
        });

        let n_ahead = {
            let settings = self.settings.lock().unwrap();
            settings.n_ahead as usize
        };

        // then check if we can output
        // FIXME: this won't work on live pipelines as we can't really report latency
        if state.pending.len() > n_ahead {
            self.push_pending(&mut state)
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::Eos(_) => {
                let mut state = self.state.lock().unwrap();

                gst::debug!(CAT, imp = self, "eos");

                while !state.pending.is_empty() {
                    let _ = self.push_pending(&mut state);
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::Caps(_caps) => {
                // set caps on src pad
                let element = self.obj();
                let templ = element.class().pad_template("src").unwrap();
                let _ = self.src_pad.push_event(gst::event::Caps::new(templ.caps()));
                true
            }
            gst::EventView::Segment(segment) => {
                if let Ok(segment) = segment.segment().clone().downcast::<gst::format::Time>() {
                    let buffer_start_segment = {
                        let settings = self.settings.lock().unwrap();
                        settings.buffer_start_segment
                    };

                    if buffer_start_segment {
                        let mut state = self.state.lock().unwrap();
                        state.pending_segment = Some(segment);
                    }
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    /// push first pending buffer as current and all the other ones as ahead text
    fn push_pending(
        &self,
        state: &mut MutexGuard<State>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if state.done {
            return Err(gst::FlowError::Flushing);
        }
        let settings = self.settings.lock().unwrap();

        let (mut text, pts, duration) = if let Some(pending_segment) = state.pending_segment.take()
        {
            let duration = match (pending_segment.start(), state.pending[0].pts) {
                (Some(start), Some(first_pts)) => Some(first_pts - start),
                _ => None,
            };

            ("".to_string(), pending_segment.start(), duration)
        } else {
            let mut text = String::new();
            let mut first_buffer = true;

            // previous buffers
            for previous in state.previous.iter() {
                if !first_buffer && !settings.separator.is_empty() {
                    text.push_str(&settings.separator);
                }

                if settings.ahead_attributes.is_empty() {
                    text.push_str(&previous.text);
                } else {
                    use std::fmt::Write;

                    write!(
                        &mut text,
                        "<span {}>{}</span>",
                        settings.previous_attributes, previous.text,
                    )
                    .unwrap();
                }

                first_buffer = false;
            }

            // current buffer
            let current = state.pending.remove(0);

            if !first_buffer && !settings.separator.is_empty() {
                text.push_str(&settings.separator);
            }

            if settings.current_attributes.is_empty() {
                text.push_str(&current.text);
            } else {
                use std::fmt::Write;

                write!(
                    &mut text,
                    "<span {}>{}</span>",
                    settings.current_attributes, current.text
                )
                .unwrap();
            }

            let pts = current.pts;
            let duration = current.duration;

            if settings.n_previous > 0 {
                state.previous.push_back(current);

                if state.previous.len() > settings.n_previous as usize {
                    state.previous.pop_front();
                }
            }

            (text, pts, duration)
        };

        // ahead buffers
        for input in state.pending.iter() {
            if !settings.separator.is_empty() {
                text.push_str(&settings.separator);
            }

            if settings.ahead_attributes.is_empty() {
                text.push_str(&input.text);
            } else {
                use std::fmt::Write;

                write!(
                    &mut text,
                    "<span {}>{}</span>",
                    settings.ahead_attributes, input.text,
                )
                .unwrap();
            }
        }

        gst::log!(CAT, imp = self, "output {:?}: {}", pts, text);

        let mut output = gst::Buffer::from_mut_slice(text.into_bytes());
        {
            let output = output.get_mut().unwrap();

            output.set_pts(pts);
            output.set_duration(duration);
        }

        self.src_pad.push(output)
    }
}
