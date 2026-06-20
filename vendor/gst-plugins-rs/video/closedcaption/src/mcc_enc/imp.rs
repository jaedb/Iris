// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::structure;
use gst::subclass::prelude::*;

use chrono::prelude::*;
use uuid::Uuid;

use std::sync::LazyLock;

use std::io::Write;
use std::sync::Mutex;

use super::headers::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Cea708Cdp,
    Cea608,
}

#[derive(Debug)]
struct State {
    format: Option<Format>,
    need_headers: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            format: None,
            need_headers: true,
        }
    }
}

#[derive(Default, Debug, Clone)]
struct Settings {
    uuid: Option<String>,
    creation_date: Option<glib::DateTime>,
}

pub struct MccEnc {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "mccenc",
        gst::DebugColorFlags::empty(),
        Some("Mcc Encoder Element"),
    )
});

impl MccEnc {
    #[allow(clippy::write_with_newline)]
    fn generate_headers(&self, _state: &State, buffer: &mut Vec<u8>) -> Result<(), gst::FlowError> {
        let settings = self.settings.lock().unwrap();

        let caps = self
            .sinkpad
            .current_caps()
            .ok_or(gst::FlowError::NotNegotiated)?;
        let framerate = match caps.structure(0).unwrap().get::<gst::Fraction>("framerate") {
            Ok(framerate) => framerate,
            Err(structure::GetError::FieldNotFound { .. }) => {
                return Err(gst::FlowError::NotNegotiated);
            }
            err => panic!("MccEnc::generate_headers caps: {err:?}"),
        };

        if framerate == gst::Fraction::new(60000, 1001) {
            buffer.extend_from_slice(PREAMBLE_V2);
        } else {
            buffer.extend_from_slice(PREAMBLE_V1);
        }

        if let Some(ref uuid) = settings.uuid {
            let _ = write!(buffer, "UUID={uuid}\r\n");
        } else {
            let _ = write!(buffer, "UUID={:X}\r\n", Uuid::new_v4().as_hyphenated());
        }

        let _ = write!(
            buffer,
            "Creation Program=GStreamer MCC Encoder {}\r\n",
            env!("CARGO_PKG_VERSION")
        );

        if let Some(ref creation_date) = settings.creation_date {
            let creation_date =
                FixedOffset::east_opt(creation_date.utc_offset().as_seconds() as i32)
                    .and_then(|tz| {
                        tz.with_ymd_and_hms(
                            creation_date.year(),
                            creation_date.month() as u32,
                            creation_date.day_of_month() as u32,
                            creation_date.hour() as u32,
                            creation_date.minute() as u32,
                            creation_date.seconds() as u32,
                        )
                        .latest()
                    })
                    .ok_or_else(|| {
                        gst::error!(CAT, imp = self, "Invalid creation datetime");
                        gst::FlowError::Error
                    })?;

            let _ = write!(
                buffer,
                "Creation Date={}\r\n",
                creation_date.format("%A, %B %d, %Y")
            );
            let _ = write!(
                buffer,
                "Creation Time={}\r\n",
                creation_date.format("%H:%M:%S")
            );
        } else {
            let creation_date = Local::now();
            let _ = write!(
                buffer,
                "Creation Date={}\r\n",
                creation_date.format("%A, %B %d, %Y")
            );
            let _ = write!(
                buffer,
                "Creation Time={}\r\n",
                creation_date.format("%H:%M:%S")
            );
        }

        if framerate.denom() == 1 {
            let _ = write!(buffer, "Time Code Rate={}\r\n", framerate.numer());
        } else {
            assert_eq!(framerate.denom(), 1001);
            let _ = write!(buffer, "Time Code Rate={}DF\r\n", framerate.numer() / 1000);
        }
        let _ = write!(buffer, "\r\n");

        Ok(())
    }

    fn encode_payload(outbuf: &mut Vec<u8>, mut slice: &[u8]) {
        while !slice.is_empty() {
            if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'O');
                slice = &slice[27..];
            } else if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'N');
                slice = &slice[24..];
            } else if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'M');
                slice = &slice[21..];
            } else if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'L');
                slice = &slice[18..];
            } else if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'K');
                slice = &slice[15..];
            } else if slice.starts_with(
                [
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                ]
                .as_ref(),
            ) {
                outbuf.push(b'J');
                slice = &slice[12..];
            } else if slice
                .starts_with([0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00].as_ref())
            {
                outbuf.push(b'I');
                slice = &slice[9..];
            } else if slice.starts_with([0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00].as_ref()) {
                outbuf.push(b'H');
                slice = &slice[6..];
            } else if slice.starts_with([0xFA, 0x00, 0x00].as_ref()) {
                outbuf.push(b'G');
                slice = &slice[3..];
            } else if slice.starts_with([0xFB, 0x80, 0x80].as_ref()) {
                outbuf.push(b'P');
                slice = &slice[3..];
            } else if slice.starts_with([0xFC, 0x80, 0x80].as_ref()) {
                outbuf.push(b'Q');
                slice = &slice[3..];
            } else if slice.starts_with([0xFD, 0x80, 0x80].as_ref()) {
                outbuf.push(b'R');
                slice = &slice[3..];
            } else if slice.starts_with([0x96, 0x69].as_ref()) {
                outbuf.push(b'S');
                slice = &slice[2..];
            } else if slice.starts_with([0x61, 0x01].as_ref()) {
                outbuf.push(b'T');
                slice = &slice[2..];
            } else if slice.starts_with([0xE1, 0x00, 0x00, 0x00].as_ref()) {
                outbuf.push(b'U');
                slice = &slice[4..];
            } else if slice[0] == 0x00 {
                outbuf.push(b'Z');
                slice = &slice[1..];
            } else {
                let _ = write!(outbuf, "{:02X}", slice[0]);
                slice = &slice[1..];
            }
        }
    }

    fn generate_caption(
        &self,
        state: &State,
        buffer: &gst::Buffer,
        outbuf: &mut Vec<u8>,
    ) -> Result<(), gst::FlowError> {
        let meta = buffer
            .meta::<gst_video::VideoTimeCodeMeta>()
            .ok_or_else(|| {
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Format,
                    ["Stream with timecodes on each buffer required"]
                );

                gst::FlowError::Error
            })?;

        let _ = write!(outbuf, "{}\t", meta.tc());

        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        let len = map.len();
        if len >= 256 {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Too big buffer: {}", map.len()]
            );

            return Err(gst::FlowError::Error);
        }
        let len = len as u8;

        match state.format {
            Some(Format::Cea608) => {
                let _ = write!(outbuf, "6102{len:02X}");
            }
            Some(Format::Cea708Cdp) => {
                let _ = write!(outbuf, "T{len:02X}");
            }
            _ => return Err(gst::FlowError::NotNegotiated),
        };

        let checksum = map.iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
        Self::encode_payload(outbuf, &map);

        if checksum == 0 {
            outbuf.push(b'Z');
        } else {
            let _ = write!(outbuf, "{checksum:02X}");
        }

        outbuf.extend_from_slice(b"\r\n".as_ref());

        Ok(())
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();

        let mut outbuf = Vec::new();
        if state.need_headers {
            state.need_headers = false;
            self.generate_headers(&state, &mut outbuf)?;
        }

        self.generate_caption(&state, &buffer, &mut outbuf)?;

        let mut buf = gst::Buffer::from_mut_slice(outbuf);
        buffer
            .copy_into(buf.get_mut().unwrap(), gst::BUFFER_COPY_METADATA, ..)
            .expect("Failed to copy buffer metadata");

        drop(state);
        self.srcpad.push(buf)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(ev) => {
                let caps = ev.caps();
                let s = caps.structure(0).unwrap();
                let framerate = match s.get::<gst::Fraction>("framerate") {
                    Ok(framerate) => framerate,
                    Err(structure::GetError::FieldNotFound { .. }) => {
                        gst::error!(CAT, obj = pad, "Caps without framerate");
                        return false;
                    }
                    err => panic!("MccEnc::sink_event caps: {err:?}"),
                };

                let mut state = self.state.lock().unwrap();
                if s.name() == "closedcaption/x-cea-608" {
                    state.format = Some(Format::Cea608);
                } else {
                    state.format = Some(Format::Cea708Cdp);
                }
                drop(state);

                // We send our own caps downstream
                let caps = gst::Caps::builder("application/x-mcc")
                    .field(
                        "version",
                        if framerate == gst::Fraction::new(60000, 1001) {
                            2i32
                        } else {
                            1i32
                        },
                    )
                    .build();
                self.srcpad.push_event(gst::event::Caps::new(&caps))
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Seek(_) => {
                gst::log!(CAT, obj = pad, "Dropping seek event");
                false
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Seeking(q) => {
                // We don't support any seeking at all
                let format = q.format();
                q.set(
                    false,
                    gst::GenericFormattedValue::none_for_format(format),
                    gst::GenericFormattedValue::none_for_format(format),
                );
                true
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for MccEnc {
    const NAME: &'static str = "GstMccEnc";
    type Type = super::MccEnc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                MccEnc::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |enc| enc.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                MccEnc::catch_panic_pad_function(parent, || false, |enc| enc.sink_event(pad, event))
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                MccEnc::catch_panic_pad_function(parent, || false, |enc| enc.src_event(pad, event))
            })
            .query_function(|pad, parent, query| {
                MccEnc::catch_panic_pad_function(parent, || false, |enc| enc.src_query(pad, query))
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

impl ObjectImpl for MccEnc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("uuid")
                    .nick("UUID")
                    .blurb("UUID for the output file")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<glib::DateTime>("creation-date")
                    .nick("Creation Date")
                    .blurb("Creation date for the output file")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "uuid" => {
                let mut settings = self.settings.lock().unwrap();
                settings.uuid = value.get().expect("type checked upstream");
            }
            "creation-date" => {
                let mut settings = self.settings.lock().unwrap();
                settings.creation_date = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "uuid" => {
                let settings = self.settings.lock().unwrap();
                settings.uuid.to_value()
            }
            "creation-date" => {
                let settings = self.settings.lock().unwrap();
                settings.creation_date.to_value()
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

impl GstObjectImpl for MccEnc {}

impl ElementImpl for MccEnc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Mcc Encoder",
                "Encoder/ClosedCaption",
                "Encodes MCC Closed Caption Files",
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

                let framerates = gst::List::new([
                    gst::Fraction::new(24, 1),
                    gst::Fraction::new(25, 1),
                    gst::Fraction::new(30000, 1001),
                    gst::Fraction::new(30, 1),
                    gst::Fraction::new(50, 1),
                    gst::Fraction::new(60000, 1001),
                    gst::Fraction::new(60, 1),
                ]);

                let s = gst::Structure::builder("closedcaption/x-cea-708")
                    .field("format", "cdp")
                    .field("framerate", &framerates)
                    .build();
                caps.append_structure(s);

                let s = gst::Structure::builder("closedcaption/x-cea-608")
                    .field("format", "s334-1a")
                    .field("framerate", &framerates)
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

            let caps = gst::Caps::builder("application/x-mcc").build();
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
