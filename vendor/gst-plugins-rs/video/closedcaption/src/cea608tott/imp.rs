// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use cea608_types::Cea608State;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use atomic_refcell::AtomicRefCell;

use std::sync::LazyLock;

use crate::cea608utils::Cea608Frame;

#[derive(Copy, Clone, Debug)]
enum Format {
    Srt,
    Vtt,
    Raw,
}

struct State {
    format: Option<Format>,
    wrote_header: bool,
    state: Cea608State,
    frame: Cea608Frame,
    previous_text: Option<(gst::ClockTime, String)>,
    index: u64,
}

impl Default for State {
    fn default() -> Self {
        State {
            format: None,
            wrote_header: false,
            state: Cea608State::default(),
            frame: Cea608Frame::new(),
            previous_text: None,
            index: 1,
        }
    }
}

pub struct Cea608ToTt {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,

    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea608tott",
        gst::DebugColorFlags::empty(),
        Some("CEA-608 to TT Element"),
    )
});

impl Cea608ToTt {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.borrow_mut();
        let format = match state.format {
            Some(format) => format,
            None => {
                gst::error!(CAT, obj = pad, "Not negotiated yet");
                return Err(gst::FlowError::NotNegotiated);
            }
        };

        let buffer_pts = buffer.pts().ok_or_else(|| {
            gst::error!(CAT, obj = pad, "Require timestamped buffers");
            gst::FlowError::Error
        })?;

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        if data.len() < 2 {
            gst::error!(CAT, obj = pad, "Invalid closed caption packet size");

            return Ok(gst::FlowSuccess::Ok);
        }

        let previous_text = {
            match state.state.decode([data[0], data[1]]) {
                Err(e) => {
                    gst::error!(
                        CAT,
                        obj = pad,
                        "Failed to decode closed caption packet: {e:?}"
                    );
                    return Ok(gst::FlowSuccess::Ok);
                }
                Ok(Some(cea608)) => {
                    gst::trace!(
                        CAT,
                        obj = pad,
                        "received {:x?} cea608: {cea608:?}",
                        [data[0], data[1]]
                    );
                    if state.frame.push_code(cea608) {
                        let text = state.frame.get_text();
                        gst::trace!(CAT, obj = pad, "generated text: {text}");
                        if text.is_empty() {
                            state.previous_text.take()
                        } else if state.frame.mode() == Some(cea608_types::Mode::PaintOn)
                            || matches!(
                                cea608,
                                cea608_types::Cea608::EraseDisplay(_)
                                    | cea608_types::Cea608::Backspace(_)
                                    | cea608_types::Cea608::EndOfCaption(_)
                                    | cea608_types::Cea608::DeleteToEndOfRow(_)
                                    | cea608_types::Cea608::CarriageReturn(_)
                            )
                        {
                            // only in some specific circumstances do we want to actually change
                            // our generated text
                            state.previous_text.replace((buffer_pts, text))
                        } else {
                            return Ok(gst::FlowSuccess::Ok);
                        }
                    } else {
                        // no change, nothing to do
                        return Ok(gst::FlowSuccess::Ok);
                    }
                }
                Ok(None) => {
                    return Ok(gst::FlowSuccess::Ok);
                }
            }
        };

        let Some(previous_text) = previous_text else {
            gst::debug!(CAT, obj = pad, "Have no previous text");
            return Ok(gst::FlowSuccess::Ok);
        };

        let duration = buffer_pts.saturating_sub(previous_text.0);

        let (timestamp, text) = previous_text;

        let header_buffer = if !state.wrote_header {
            state.wrote_header = true;

            match format {
                Format::Vtt => Some(Self::create_vtt_header(timestamp)),
                Format::Srt | Format::Raw => None,
            }
        } else {
            None
        };

        let buffer = match format {
            Format::Vtt => Self::create_vtt_buffer(timestamp, duration, text),
            Format::Srt => Self::create_srt_buffer(timestamp, duration, state.index, text),
            Format::Raw => Self::create_raw_buffer(timestamp, duration, text),
        };
        state.index += 1;
        drop(state);

        if let Some(header_buffer) = header_buffer {
            self.srcpad.push(header_buffer)?;
        }

        self.srcpad.push(buffer)
    }

    fn create_vtt_header(timestamp: gst::ClockTime) -> gst::Buffer {
        use std::fmt::Write;

        let mut headers = String::new();
        writeln!(&mut headers, "WEBVTT\r").unwrap();
        writeln!(&mut headers, "\r").unwrap();

        let mut buffer = gst::Buffer::from_mut_slice(headers.into_bytes());
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
        text: String,
    ) -> gst::Buffer {
        use std::fmt::Write;

        let mut data = String::new();

        let (h1, m1, s1, ms1) = Self::split_time(timestamp);
        let (h2, m2, s2, ms2) = Self::split_time(timestamp + duration);

        writeln!(
            &mut data,
            "{h1:02}:{m1:02}:{s1:02}.{ms1:03} --> {h2:02}:{m2:02}:{s2:02}.{ms2:03}\r"
        )
        .unwrap();
        writeln!(&mut data, "{text}\r").unwrap();
        writeln!(&mut data, "\r").unwrap();

        let mut buffer = gst::Buffer::from_mut_slice(data.into_bytes());
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(timestamp);
            buffer.set_duration(duration);
        }

        buffer
    }

    fn create_srt_buffer(
        timestamp: gst::ClockTime,
        duration: gst::ClockTime,
        index: u64,
        text: String,
    ) -> gst::Buffer {
        use std::fmt::Write;

        let mut data = String::new();

        let (h1, m1, s1, ms1) = Self::split_time(timestamp);
        let (h2, m2, s2, ms2) = Self::split_time(timestamp + duration);

        writeln!(&mut data, "{index}\r").unwrap();
        writeln!(
            &mut data,
            "{h1:02}:{m1:02}:{s1:02},{ms1:03} --> {h2:02}:{m2:02}:{s2:02},{ms2:03}\r"
        )
        .unwrap();
        writeln!(&mut data, "{text}\r").unwrap();
        writeln!(&mut data, "\r").unwrap();

        let mut buffer = gst::Buffer::from_mut_slice(data.into_bytes());
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(timestamp);
            buffer.set_duration(duration);
        }

        buffer
    }

    fn create_raw_buffer(
        timestamp: gst::ClockTime,
        duration: gst::ClockTime,
        text: String,
    ) -> gst::Buffer {
        let mut buffer = gst::Buffer::from_mut_slice(text.into_bytes());
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(timestamp);
            buffer.set_duration(duration);
        }

        buffer
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(..) => {
                let mut state = self.state.borrow_mut();

                if state.format.is_some() {
                    return true;
                }

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
                let new_caps = if s.name() == "application/x-subtitle-vtt" {
                    state.format = Some(Format::Vtt);
                    gst::Caps::builder("application/x-subtitle-vtt").build()
                } else if s.name() == "application/x-subtitle" {
                    state.format = Some(Format::Srt);
                    gst::Caps::builder("application/x-subtitle").build()
                } else if s.name() == "text/x-raw" {
                    state.format = Some(Format::Raw);
                    gst::Caps::builder("text/x-raw")
                        .field("format", "utf8")
                        .build()
                } else {
                    unreachable!();
                };

                let new_event = gst::event::Caps::new(&new_caps);

                return self.srcpad.push_event(new_event);
            }
            EventView::FlushStop(..) => {
                let mut state = self.state.borrow_mut();
                state.frame = Cea608Frame::new();
                state.state = Cea608State::default();
                state.previous_text = None;
            }
            EventView::Eos(..) => {
                let mut state = self.state.borrow_mut();
                if let Some((timestamp, text)) = state.previous_text.take() {
                    gst::debug!(CAT, obj = pad, "Outputting final text on EOS");

                    let format = state.format.unwrap();

                    let header_buffer = if !state.wrote_header {
                        state.wrote_header = true;

                        match format {
                            Format::Vtt => Some(Self::create_vtt_header(timestamp)),
                            Format::Srt | Format::Raw => None,
                        }
                    } else {
                        None
                    };

                    let buffer = match format {
                        Format::Vtt => {
                            Self::create_vtt_buffer(timestamp, gst::ClockTime::ZERO, text)
                        }
                        Format::Srt => Self::create_srt_buffer(
                            timestamp,
                            gst::ClockTime::ZERO,
                            state.index,
                            text,
                        ),
                        Format::Raw => {
                            Self::create_raw_buffer(timestamp, gst::ClockTime::ZERO, text)
                        }
                    };
                    state.index += 1;
                    drop(state);

                    if let Some(header_buffer) = header_buffer {
                        let _ = self.srcpad.push(header_buffer);
                    }

                    let _ = self.srcpad.push(buffer);
                }
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cea608ToTt {
    const NAME: &'static str = "GstCea608ToTt";
    type Type = super::Cea608ToTt;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cea608ToTt::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cea608ToTt::catch_panic_pad_function(
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
            state: AtomicRefCell::new(State::default()),
        }
    }
}

impl ObjectImpl for Cea608ToTt {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for Cea608ToTt {}

impl ElementImpl for Cea608ToTt {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CEA-608 to TT",
                "Generic",
                "Converts CEA-608 Closed Captions to SRT/VTT timed text",
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

                // WebVTT
                let s = gst::Structure::builder("application/x-subtitle-vtt").build();
                caps.append_structure(s);

                // SRT
                let s = gst::Structure::builder("application/x-subtitle").build();
                caps.append_structure(s);

                // Raw timed text
                let s = gst::Structure::builder("text/x-raw")
                    .field("format", "utf8")
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

            let caps = gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
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

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.borrow_mut();
                *state = State::default();
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let mut state = self.state.borrow_mut();
                *state = State::default();
            }
            _ => (),
        }

        Ok(ret)
    }
}
