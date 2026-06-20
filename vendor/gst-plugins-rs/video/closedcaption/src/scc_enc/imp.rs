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
use gst::structure;
use gst::subclass::prelude::*;
use gst_video::{self, ValidVideoTimeCode};

use std::sync::LazyLock;

use std::io::Write;
use std::sync::Mutex;

const DEFAULT_OUTPUT_PADDING: bool = true;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "sccenc",
        gst::DebugColorFlags::empty(),
        Some("Scc Encoder Element"),
    )
});

#[derive(Clone, Debug)]
struct Settings {
    output_padding: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_padding: DEFAULT_OUTPUT_PADDING,
        }
    }
}

#[derive(Debug)]
struct State {
    need_headers: bool,
    expected_timecode: Option<ValidVideoTimeCode>,
    internal_buffer: Vec<gst::Buffer>,
    framerate: Option<gst::Fraction>,
    settings: Settings,
}

impl Default for State {
    fn default() -> Self {
        Self {
            need_headers: true,
            expected_timecode: None,
            internal_buffer: Vec::with_capacity(64),
            framerate: None,
            settings: Settings::default(),
        }
    }
}

impl State {
    // Write the header to the buffer and set need_headers to false
    fn generate_headers(&mut self, buffer: &mut Vec<u8>) {
        assert!(buffer.is_empty());
        self.need_headers = false;
        buffer.extend_from_slice(b"Scenarist_SCC V1.0\r\n\r\n");
    }

    fn encode_payload(outbuf: &mut Vec<u8>, slice: &[u8]) {
        write!(outbuf, "{:02x}{:02x}", slice[0], slice[1]).unwrap();
    }

    fn generate_caption(
        &mut self,
        imp: &SccEnc,
        buffer: gst::Buffer,
    ) -> Result<Option<gst::Buffer>, gst::FlowError> {
        // Arbitrary number that was chosen to keep in order
        // to batch pushes of smaller buffers
        const MAXIMUM_PACKETES_PER_LINE: usize = 16;

        assert!(self.internal_buffer.len() < MAXIMUM_PACKETES_PER_LINE);

        if buffer.size() != 2 {
            gst::element_imp_error!(
                imp,
                gst::StreamError::Format,
                ["Wrongly sized CEA608 packet: {}", buffer.size()]
            );

            return Err(gst::FlowError::Error);
        };

        if !self.settings.output_padding {
            let map = buffer.map_readable().map_err(|_| {
                gst::element_imp_error!(
                    imp,
                    gst::StreamError::Format,
                    ["Failed to map buffer readable"]
                );

                gst::FlowError::Error
            })?;

            if map[0] == 0x80 && map[1] == 0x80 {
                return Ok(None);
            }

            drop(map);
        }

        let mut timecode = buffer
            .meta::<gst_video::VideoTimeCodeMeta>()
            .ok_or_else(|| {
                gst::element_imp_error!(
                    imp,
                    gst::StreamError::Format,
                    ["Stream with timecodes on each buffer required"]
                );

                // If we need to skip a buffer, increment the frame if it exists
                // to avoid getting out of sync
                if let Some(ref mut timecode) = self.expected_timecode {
                    timecode.increment_frame();
                }

                gst::FlowError::Error
            })?
            .tc();

        if self.expected_timecode.is_none() {
            self.expected_timecode = Some(timecode.clone());
        }

        // if the timecode is different from the expected one,
        // flush the previous line into the buffer, and push
        // the new packet to the, now empty, internal buffer
        if Some(&timecode) != self.expected_timecode.as_ref() {
            let outbuf = self.write_line(imp)?;

            assert!(self.internal_buffer.is_empty());
            self.internal_buffer.push(buffer);

            timecode.increment_frame();
            self.expected_timecode = Some(timecode);

            return Ok(outbuf);
        } else if let Some(ref mut timecode) = self.expected_timecode {
            timecode.increment_frame();
        }

        self.internal_buffer.push(buffer);

        if self.internal_buffer.len() == MAXIMUM_PACKETES_PER_LINE {
            return self.write_line(imp);
        }

        Ok(None)
    }

    // Flush the internal buffers into a line
    fn write_line(&mut self, imp: &SccEnc) -> Result<Option<gst::Buffer>, gst::FlowError> {
        let mut outbuf = Vec::new();
        let mut line_start = true;

        if self.internal_buffer.is_empty() {
            return Ok(None);
        }

        if self.need_headers {
            self.generate_headers(&mut outbuf);
        }

        let first_buf = self.internal_buffer.first().unwrap();
        for buffer in self.internal_buffer.iter() {
            let map = buffer.map_readable().map_err(|_| {
                gst::element_imp_error!(
                    imp,
                    gst::StreamError::Format,
                    ["Failed to map buffer readable"]
                );

                gst::FlowError::Error
            })?;

            // If its the first packet in the line, write the timecode first
            // else, separate the packets with a space
            if line_start {
                let timecode = buffer
                    .meta::<gst_video::VideoTimeCodeMeta>()
                    // Checked already before the buffer has been pushed to the
                    // internal_buffer
                    .expect("Buffer without timecode")
                    .tc();

                let _ = write!(outbuf, "{timecode}\t");
                line_start = false;
            } else {
                outbuf.push(b' ');
            }

            Self::encode_payload(&mut outbuf, &map);
        }

        outbuf.extend_from_slice(b"\r\n\r\n".as_ref());

        let buffer = {
            let mut buffer = gst::Buffer::from_mut_slice(outbuf);
            let buf_mut = buffer.get_mut().unwrap();

            // Something is seriously wrong else
            assert!(self.framerate.is_some());
            let framerate = self.framerate.unwrap();

            let dur = gst::ClockTime::SECOND.mul_div_floor(
                self.internal_buffer.len() as u64 * framerate.denom() as u64,
                framerate.numer() as u64,
            );
            buf_mut.set_duration(dur);

            // Copy the metadata of the first buffer
            first_buf
                .copy_into(buf_mut, gst::BUFFER_COPY_METADATA, ..)
                .expect("Failed to copy buffer metadata");
            buf_mut.set_pts(first_buf.pts());
            buffer
        };

        // Clear the internal buffer
        self.internal_buffer.clear();

        Ok(Some(buffer))
    }
}

pub struct SccEnc {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl SccEnc {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();
        let res = state.generate_caption(self, buffer)?;

        if let Some(outbuf) = res {
            gst::trace!(CAT, obj = pad, "Pushing buffer {:?} to the pad", &outbuf);

            drop(state);
            self.srcpad.push(outbuf)?;
        };

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(ev) => {
                let caps = ev.caps();
                let s = caps.structure(0).unwrap();
                let framerate = match s.get::<gst::Fraction>("framerate") {
                    Ok(framerate) => Some(framerate),
                    Err(structure::GetError::FieldNotFound { .. }) => {
                        gst::error!(CAT, obj = pad, "Caps without framerate");
                        return false;
                    }
                    err => panic!("SccEnc::sink_event caps: {err:?}"),
                };

                let mut state = self.state.lock().unwrap();
                state.framerate = framerate;

                // We send our own caps downstream
                let caps = gst::Caps::builder("application/x-scc").build();
                self.srcpad.push_event(gst::event::Caps::new(&caps))
            }
            EventView::Eos(_) => {
                let mut state = self.state.lock().unwrap();

                let outbuf = state.write_line(self);

                if let Ok(Some(buffer)) = outbuf {
                    gst::trace!(CAT, obj = pad, "Pushing buffer {:?} to the pad", &buffer);

                    drop(state);
                    if self.srcpad.push(buffer).is_err() {
                        gst::error!(CAT, obj = pad, "Failed to push buffer to the pad");
                        return false;
                    }
                } else if let Err(err) = outbuf {
                    gst::error!(
                        CAT,
                        obj = pad,
                        "Failed to write a line after EOS: {:?}",
                        err
                    );
                    return false;
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
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
impl ObjectSubclass for SccEnc {
    const NAME: &'static str = "GstSccEnc";
    type Type = super::SccEnc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                SccEnc::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |enc| enc.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                SccEnc::catch_panic_pad_function(parent, || false, |enc| enc.sink_event(pad, event))
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                SccEnc::catch_panic_pad_function(parent, || false, |enc| enc.src_event(pad, event))
            })
            .query_function(|pad, parent, query| {
                SccEnc::catch_panic_pad_function(parent, || false, |enc| enc.src_query(pad, query))
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

impl ObjectImpl for SccEnc {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("output-padding")
                .nick("Output padding")
                .blurb(
                    "Whether the encoder should output padding captions. \
                The element will never add padding, but will encode padding \
                buffers it receives if this property is set to true.",
                )
                .default_value(DEFAULT_OUTPUT_PADDING)
                .mutable_ready()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "output-padding" => {
                self.settings.lock().unwrap().output_padding =
                    value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "output-padding" => {
                let settings = self.settings.lock().unwrap();
                settings.output_padding.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for SccEnc {}

impl ElementImpl for SccEnc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
            "Scc Encoder",
            "Encoder/ClosedCaption",
            "Encodes SCC Closed Caption Files",
            "Sebastian Dröge <sebastian@centricular.com>, Jordan Petridis <jordan@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let framerates =
                gst::List::new([gst::Fraction::new(30000, 1001), gst::Fraction::new(30, 1)]);
            let caps = gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .field("framerate", framerates)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-scc").build();
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
            gst::StateChange::ReadyToPaused => {
                // Reset the whole state
                let mut state = self.state.lock().unwrap();
                *state = State::default();
                state.settings = self.settings.lock().unwrap().clone();
            }
            gst::StateChange::PausedToReady => {
                // Reset the whole state
                let mut state = self.state.lock().unwrap();
                *state = State::default();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
