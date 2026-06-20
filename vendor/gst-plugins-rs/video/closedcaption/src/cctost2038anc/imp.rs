// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-cctost2038anc
 *
 * Takes closed captions (CEA-608 and/or CEA-708) as produced by other GStreamer closed caption
 * processing elements and converts them into SMPTE ST-2038 ancillary data that can be fed to
 * `st2038ancmux` and then to `mpegtsmux` for splicing/muxing into an MPEG-TS container. The
 * `line-number` and `horizontal-offset` properties should be set to the desired line number
 * and horizontal offset.
 *
 * Since: plugins-rs-0.14.0
 */
use std::sync::{LazyLock, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use atomic_refcell::AtomicRefCell;

use crate::st2038anc_utils::convert_to_st2038_buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Cea608,
    Cea708,
}

#[derive(Default)]
struct State {
    format: Option<Format>,
}

#[derive(Clone)]
struct Settings {
    c_not_y_channel: bool,
    line_number: u16,
    horizontal_offset: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            c_not_y_channel: false,
            line_number: 9,
            horizontal_offset: 0,
        }
    }
}

pub struct CcToSt2038Anc {
    sinkpad: gst::Pad,
    srcpad: gst::Pad,

    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cctost2038anc",
        gst::DebugColorFlags::empty(),
        Some("Closed Caption to ST-2038 ANC Element"),
    )
});

impl CcToSt2038Anc {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let state = self.state.borrow_mut();
        let settings = self.settings.lock().unwrap().clone();

        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let (did, sdid) = match state.format {
            Some(Format::Cea608) => (0x61, 0x02),
            Some(Format::Cea708) => (0x61, 0x01),
            None => {
                gst::error!(CAT, imp = self, "No caps set");
                return Err(gst::FlowError::NotNegotiated);
            }
        };

        let mut outbuf = match convert_to_st2038_buffer(
            settings.c_not_y_channel,
            settings.line_number,
            settings.horizontal_offset,
            did,
            sdid,
            &map,
        ) {
            Ok(outbuf) => outbuf,
            Err(err) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Can't convert Closed Caption buffer: {err}"
                );
                return Err(gst::FlowError::Error);
            }
        };
        drop(map);

        {
            let outbuf = outbuf.get_mut().unwrap();
            let _ = buffer.copy_into(outbuf, gst::BUFFER_COPY_METADATA, ..);
        }
        drop(state);

        self.srcpad.push(outbuf)
    }

    #[allow(clippy::single_match)]
    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(ev) => {
                let caps = ev.caps();
                let s = caps.structure(0).unwrap();

                let format = match s.name().as_str() {
                    "closedcaption/x-cea-608" => Format::Cea608,
                    "closedcaption/x-cea-708" => Format::Cea708,
                    _ => {
                        gst::error!(CAT, imp = self, "Unsupported caps {caps:?}");
                        return false;
                    }
                };

                gst::debug!(CAT, imp = self, "Configuring format {format:?}");

                let mut state = self.state.borrow_mut();
                state.format = Some(format);
                drop(state);

                let templ_caps = self.srcpad.pad_template_caps();
                let mut peer_caps = self.srcpad.peer_query_caps(Some(&templ_caps));
                gst::debug!(CAT, obj = pad, "Downstream caps {peer_caps:?}");

                let alignment = if peer_caps.is_empty() {
                    "frame"
                } else {
                    peer_caps.fixate();
                    peer_caps
                        .structure(0)
                        .and_then(|s| s.get::<&str>("alignment").ok())
                        .unwrap_or("frame")
                };

                let src_caps = {
                    let mut caps = self.srcpad.pad_template_caps();
                    let caps_ref = caps.make_mut();
                    if let Ok(framerate) = s.get::<gst::Fraction>("framerate") {
                        caps_ref.set("framerate", framerate);
                    }
                    caps_ref.set("alignment", alignment);
                    caps
                };

                return self.srcpad.push_event(
                    gst::event::Caps::builder(&src_caps)
                        .seqnum(ev.seqnum())
                        .build(),
                );
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for CcToSt2038Anc {
    const NAME: &'static str = "GstCcToSt2038Anc";
    type Type = super::CcToSt2038Anc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                CcToSt2038Anc::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                CcToSt2038Anc::catch_panic_pad_function(
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
            sinkpad,
            srcpad,
            state: AtomicRefCell::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for CcToSt2038Anc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("c-not-y-channel")
                    .nick("Y Not C Channel")
                    .blurb("Set the y_not_c_channel flag in the output")
                    .default_value(Settings::default().c_not_y_channel)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("line-number")
                    .nick("Line Number")
                    .blurb("Line Number of the output")
                    .default_value(Settings::default().line_number as u32)
                    .maximum(2047)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("horizontal-offset")
                    .nick("Horizontal Offset")
                    .blurb("Horizontal offset of the output")
                    .default_value(Settings::default().horizontal_offset as u32)
                    .maximum(4095)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "c-not-y-channel" => {
                let mut settings = self.settings.lock().unwrap();

                settings.c_not_y_channel = value.get().expect("type checked upstream");
            }
            "line-number" => {
                let mut settings = self.settings.lock().unwrap();

                settings.line_number = value.get::<u32>().expect("type checked upstream") as u16;
            }
            "horizontal-offset" => {
                let mut settings = self.settings.lock().unwrap();

                settings.horizontal_offset =
                    value.get::<u32>().expect("type checked upstream") as u16;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "c-not-y-channel" => {
                let settings = self.settings.lock().unwrap();
                settings.c_not_y_channel.to_value()
            }
            "line-number" => {
                let settings = self.settings.lock().unwrap();
                (settings.line_number as u32).to_value()
            }
            "horizontal-offset" => {
                let settings = self.settings.lock().unwrap();
                (settings.horizontal_offset as u32).to_value()
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

impl GstObjectImpl for CcToSt2038Anc {}

impl ElementImpl for CcToSt2038Anc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CC to ST-2038 ANC",
                "Generic",
                "Converts Closed Captions to ST-2038 ANC",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps_aligned = gst::Caps::builder("meta/x-st-2038")
                .field("alignment", gst::List::new(["frame", "line", "packet"]))
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &caps_aligned,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &[
                    gst::Structure::builder("closedcaption/x-cea-608")
                        .field("format", "s334-1a")
                        .build(),
                    gst::Structure::builder("closedcaption/x-cea-708")
                        .field("format", "cdp")
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
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
                *self.state.borrow_mut() = State::default();
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                *self.state.borrow_mut() = State::default();
            }
            _ => (),
        }

        Ok(ret)
    }
}
