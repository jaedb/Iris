// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use regex::{Regex, RegexBuilder};
use std::default::Default;
use std::sync::Mutex;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "regex",
        gst::DebugColorFlags::empty(),
        Some("Regular Expression element"),
    )
});

enum Operation {
    ReplaceAll(String),
}

struct Command {
    pattern: String,
    regex: Regex,
    operation: Operation,
}

#[derive(Default)]
struct State {
    commands: Vec<Command>,
}

pub struct RegEx {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

impl RegEx {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        let mut data = std::str::from_utf8(&data)
            .map_err(|err| {
                gst::error!(CAT, imp = self, "Can't decode utf8: {}", err);
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Decode,
                    ["Failed to decode utf8: {}", err]
                );

                gst::FlowError::Error
            })?
            .to_string();

        let state = self.state.lock().unwrap();

        for command in &state.commands {
            match &command.operation {
                Operation::ReplaceAll(replacement) => {
                    data = command
                        .regex
                        .replace_all(&data, replacement.as_str())
                        .to_string();
                }
            }
        }

        let mut outbuf = gst::Buffer::from_mut_slice(data.into_bytes());

        {
            let outbuf_mut = outbuf.get_mut().unwrap();
            let _ = buffer.copy_into(
                outbuf_mut,
                gst::BufferCopyFlags::FLAGS
                    | gst::BufferCopyFlags::TIMESTAMPS
                    | gst::BufferCopyFlags::META,
                ..,
            );
        }

        drop(state);

        self.srcpad.push(outbuf)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RegEx {
    const NAME: &'static str = "GstRegEx";
    type Type = super::RegEx;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                RegEx::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |regex| regex.sink_chain(pad, buffer),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::FIXED_CAPS)
            .build();

        let state = Mutex::new(State::default());

        Self {
            srcpad,
            sinkpad,
            state,
        }
    }
}

impl ObjectImpl for RegEx {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![gst::ParamSpecArray::builder("commands")
                .nick("Commands")
                .blurb("A set of commands to apply on input text")
                .element_spec(
                    &glib::ParamSpecBoxed::builder::<gst::Structure>("command")
                        .nick("Command")
                        .blurb("A command to apply on input text")
                        .build(),
                )
                .mutable_playing()
                .build()]
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
            "commands" => {
                let mut state = self.state.lock().unwrap();
                state.commands = vec![];
                let commands = value.get::<gst::ArrayRef>().expect("type checked upstream");
                for command in commands.as_slice() {
                    let s = match command
                        .get::<Option<gst::Structure>>()
                        .expect("type checked upstream")
                    {
                        Some(s) => s,
                        None => {
                            continue;
                        }
                    };
                    let operation = s.name();

                    let pattern = match s.get::<Option<String>>("pattern") {
                        Ok(Some(pattern)) => pattern,
                        Ok(None) | Err(_) => {
                            gst::error!(
                                CAT,
                                imp = self,
                                "All commands require a pattern field as a string"
                            );
                            continue;
                        }
                    };

                    let mut builder = RegexBuilder::new(&pattern);
                    builder
                        .unicode(s.get::<bool>("unicode").unwrap_or(true))
                        .case_insensitive(s.get::<bool>("case-insensitive").unwrap_or(false))
                        .multi_line(s.get::<bool>("multi-line").unwrap_or(false))
                        .dot_matches_new_line(
                            s.get::<bool>("dot-matches-new-line").unwrap_or(false),
                        )
                        .crlf(s.get::<bool>("crlf").unwrap_or(false))
                        .line_terminator(s.get::<u8>("line-terminator").unwrap_or(b'\n'))
                        .swap_greed(s.get::<bool>("swap-greed").unwrap_or(false))
                        .ignore_whitespace(s.get::<bool>("ignore-whitespace").unwrap_or(false))
                        .octal(s.get::<bool>("octal").unwrap_or(false));

                    if let Ok(limit) = s.get::<u64>("size-limit") {
                        builder.size_limit(limit as usize);
                    }

                    if let Ok(limit) = s.get::<u64>("dfa-size-limit") {
                        builder.dfa_size_limit(limit as usize);
                    }

                    if let Ok(limit) = s.get::<u32>("nest-limit") {
                        builder.nest_limit(limit);
                    }

                    let regex = match builder.build() {
                        Ok(regex) => regex,
                        Err(err) => {
                            gst::error!(CAT, imp = self, "Failed to compile regex: {:?}", err);
                            continue;
                        }
                    };

                    match operation.as_str() {
                        "replace-all" | "replace_all" => {
                            let replacement = match s.get::<Option<String>>("replacement") {
                                Ok(Some(pattern)) => pattern,
                                Ok(None) | Err(_) => {
                                    gst::error!(
                                        CAT,
                                        imp = self,
                                        "Replace operations require a replacement field as a string"
                                    );
                                    continue;
                                }
                            };
                            state.commands.push(Command {
                                pattern,
                                regex,
                                operation: Operation::ReplaceAll(replacement),
                            });
                        }
                        val => {
                            gst::error!(CAT, imp = self, "Unknown operation {}", val);
                        }
                    }
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "commands" => {
                let state = self.state.lock().unwrap();
                let mut commands = gst::Array::default();
                for command in &state.commands {
                    match command.operation {
                        Operation::ReplaceAll(ref replacement) => {
                            commands.append(
                                gst::Structure::builder("replace-all")
                                    .field("pattern", &command.pattern)
                                    .field("replacement", replacement)
                                    .build(),
                            );
                        }
                    }
                }
                commands.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RegEx {}

impl ElementImpl for RegEx {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Regular Expression processor",
                "Text/Filter",
                "Applies operations according to regular expressions",
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

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}
