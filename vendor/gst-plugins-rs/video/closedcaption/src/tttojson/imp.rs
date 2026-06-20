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

use std::cmp::min;
use std::sync::Mutex;

use crate::cea608utils::*;
use crate::ttutils::{Chunk, Line, Lines};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "tttojson",
        gst::DebugColorFlags::empty(),
        Some("Timed Text to JSON"),
    )
});

const DEFAULT_MODE: Cea608Mode = Cea608Mode::RollUp2;

#[derive(Debug, Clone)]
struct Settings {
    mode: Cea608Mode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { mode: DEFAULT_MODE }
    }
}

pub struct TtToJson {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
}

impl TtToJson {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let pts = buffer.pts();
        let duration = buffer.duration();

        let buffer = buffer.into_mapped_buffer_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        let text = std::str::from_utf8(buffer.as_slice()).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map decode as utf8: {}", err]
            );

            gst::FlowError::Error
        })?;

        let mode = self.settings.lock().unwrap().mode;

        let mut lines = Lines {
            lines: Vec::new(),
            mode: Some(mode),
            clear: Some(false),
        };

        let mut row = min(15usize.saturating_sub(text.lines().count()), 13usize) as u32;

        for phrase in text.lines() {
            lines.lines.push(Line {
                carriage_return: Some(true),
                column: Some(0),
                row: Some(row),
                chunks: vec![Chunk {
                    // Default CEA 608 styling
                    style: TextStyle::White,
                    underline: false,
                    text: phrase.to_string(),
                }],
            });

            row = min(14u32, row + 1);
        }

        let json = serde_json::to_string(&lines).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ["Failed to serialize as json {}", err]
            );

            gst::FlowError::Error
        })?;

        let mut buf = gst::Buffer::from_mut_slice(json.into_bytes());
        {
            let buf_mut = buf.get_mut().unwrap();
            buf_mut.set_pts(pts);
            buf_mut.set_duration(duration);
        }

        self.srcpad.push(buf)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(_) => {
                // We send our own caps downstream
                let caps = gst::Caps::builder("application/x-json")
                    .field("format", "cea608")
                    .build();
                self.srcpad.push_event(gst::event::Caps::new(&caps))
            }
            EventView::Eos(_) => gst::Pad::event_default(pad, Some(&*self.obj()), event),
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

impl GstObjectImpl for TtToJson {}

impl ElementImpl for TtToJson {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Timed text to JSON encoder",
                "Encoder/ClosedCaption",
                "Encodes Timed Text to JSON",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-json").build();
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
}

#[glib::object_subclass]
impl ObjectSubclass for TtToJson {
    const NAME: &'static str = "GstTtToJson";
    type Type = super::TtToJson;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                TtToJson::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |enc| enc.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TtToJson::catch_panic_pad_function(
                    parent,
                    || false,
                    |enc| enc.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::from_template(&templ);

        Self {
            srcpad,
            sinkpad,
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for TtToJson {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default("mode", DEFAULT_MODE)
                    .nick("Mode")
                    .blurb("Which mode to operate in")
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
                let mut settings = self.settings.lock().unwrap();
                settings.mode = value.get::<Cea608Mode>().expect("type checked upstream");
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
            _ => unimplemented!(),
        }
    }
}
