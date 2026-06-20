// Copyright (C) 2020 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;

use crate::ccutils::{extract_cdp, ParseError, ParseErrorCode};
use std::sync::LazyLock;

use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ccdetect",
        gst::DebugColorFlags::empty(),
        Some("Closed Caption Detection"),
    )
});

const DEFAULT_WINDOW: gst::ClockTime = gst::ClockTime::from_seconds(10);
const DEFAULT_CC608: bool = false;
const DEFAULT_CC708: bool = false;

#[derive(Debug, Clone, Copy)]
struct Settings {
    pub window: gst::ClockTime,
    pub cc608: bool,
    pub cc708: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            window: DEFAULT_WINDOW,
            cc608: DEFAULT_CC608,
            cc708: DEFAULT_CC708,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CCFormat {
    Cc708Cdp,
    Cc708CcData,
}

#[derive(Debug, Clone, Copy)]
struct State {
    format: CCFormat,
    last_cc608_change: Option<gst::ClockTime>,
    last_cc708_change: Option<gst::ClockTime>,
}

#[derive(Default)]
pub struct CCDetect {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

#[derive(Debug, Clone, Copy)]
struct CCPacketContents {
    cc608: bool,
    cc708: bool,
}

impl CCDetect {
    fn detect_cc_data(&self, data: &[u8]) -> Result<CCPacketContents, ParseError> {
        if data.len() % 3 != 0 {
            gst::warning!(
                CAT,
                imp = self,
                "cc_data length is not a multiple of 3, truncating"
            );
        }

        /* logic from ccconverter */
        let mut started_ccp = false;
        let mut have_cc608 = false;
        let mut have_cc708 = false;
        for (i, triple) in data.chunks_exact(3).enumerate() {
            let cc_valid = (triple[0] & 0x04) == 0x04;
            let cc_type = triple[0] & 0x03;
            gst::trace!(
                CAT,
                imp = self,
                "triple:{} have ccp:{} 608:{} 708:{} data:{:02x},{:02x},{:02x} cc_valid:{} cc_type:{:02b}",
                i * 3,
                started_ccp,
                have_cc608,
                have_cc708,
                triple[0],
                triple[1],
                triple[2],
                cc_valid,
                cc_type
            );

            if !started_ccp && cc_valid && (cc_type == 0x00 || cc_type == 0x01) {
                if triple[1] != 0x80 || triple[2] != 0x80 {
                    have_cc608 = true;
                }
                continue;
            }

            if cc_type & 0b10 == 0b10 {
                started_ccp = true;
            }

            if !cc_valid {
                continue;
            }

            if cc_type == 0x00 || cc_type == 0x01 {
                return Err(ParseError {
                    code: ParseErrorCode::WrongLayout,
                    byte: data.len() - i * 3,
                    msg: String::from("Invalid cc_data. cea608 bytes after cea708"),
                });
            }

            have_cc708 = true;
        }

        Ok(CCPacketContents {
            cc608: have_cc608,
            cc708: have_cc708,
        })
    }

    fn detect_cdp(&self, data: &[u8]) -> Result<CCPacketContents, ParseError> {
        let data = extract_cdp(data)?;

        self.detect_cc_data(data)
    }

    fn detect(&self, format: CCFormat, data: &[u8]) -> Result<CCPacketContents, ParseError> {
        match format {
            CCFormat::Cc708CcData => self.detect_cc_data(data),
            CCFormat::Cc708Cdp => self.detect_cdp(data),
        }
    }

    fn maybe_update_properties(
        &self,
        ts: gst::ClockTime,
        cc_packet: CCPacketContents,
    ) -> Result<(), gst::FlowError> {
        let mut notify_cc608 = false;
        let mut notify_cc708 = false;

        {
            let mut settings = self.settings.lock().unwrap();

            let mut state_guard = self.state.lock().unwrap();
            let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

            gst::trace!(
                CAT,
                imp = self,
                "packet contains {:?} current settings {:?} and state {:?}",
                cc_packet,
                settings,
                state
            );

            if cc_packet.cc608 != settings.cc608 {
                let changed = state
                    .last_cc608_change
                    .is_none_or(|last_cc608_change| ts > last_cc608_change + settings.window);

                if changed {
                    settings.cc608 = cc_packet.cc608;
                    state.last_cc608_change = Some(ts);
                    notify_cc608 = true;
                }
            } else {
                state.last_cc608_change = Some(ts);
            }

            if cc_packet.cc708 != settings.cc708 {
                let changed = state
                    .last_cc708_change
                    .is_none_or(|last_cc708_change| ts > last_cc708_change + settings.window);
                if changed {
                    settings.cc708 = cc_packet.cc708;
                    state.last_cc708_change = Some(ts);
                    notify_cc708 = true;
                }
            } else {
                state.last_cc708_change = Some(ts);
            }

            gst::trace!(
                CAT,
                imp = self,
                "changed to settings {:?} state {:?}",
                settings,
                state
            );
        }

        if notify_cc608 {
            self.obj().notify("cc608");
        }
        if notify_cc708 {
            self.obj().notify("cc708");
        }

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for CCDetect {
    const NAME: &'static str = "GstCCDetect";
    type Type = super::CCDetect;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for CCDetect {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("window")
                    .nick("Window")
                    .blurb("Window of time (in ns) to determine if captions exist in the stream")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_WINDOW.nseconds())
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("cc608")
                    .nick("cc608")
                    .blurb("Whether CEA608 captions (CC1/CC3) have been detected")
                    .default_value(DEFAULT_CC608)
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("cc708")
                    .nick("cc608")
                    .blurb("Whether CEA708 captions (cc_data) have been detected")
                    .default_value(DEFAULT_CC708)
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "window" => {
                let mut settings = self.settings.lock().unwrap();
                settings.window = value.get::<u64>().unwrap().nseconds();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "window" => {
                let settings = self.settings.lock().unwrap();
                settings.window.nseconds().to_value()
            }
            "cc608" => {
                let settings = self.settings.lock().unwrap();
                settings.cc608.to_value()
            }
            "cc708" => {
                let settings = self.settings.lock().unwrap();
                settings.cc708.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for CCDetect {}

impl ElementImpl for CCDetect {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Closed Caption Detect",
                "Filter/Video/ClosedCaption/Detect",
                "Detect if valid closed captions are present in a stream",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let mut caps = gst::Caps::new_empty();
            {
                let caps = caps.get_mut().unwrap();
                let s = gst::Structure::builder("closedcaption/x-cea-708")
                    .field("format", gst::List::new(["cc_data", "cdp"]))
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

impl BaseTransformImpl for CCDetect {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;
    const PASSTHROUGH_ON_SAME_CAPS: bool = true;

    fn transform_ip_passthrough(
        &self,
        buf: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let map = buf.map_readable().map_err(|_| gst::FlowError::Error)?;

        let pts = buf.pts().ok_or_else(|| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Input buffers must have valid timestamps"]
            );
            gst::FlowError::Error
        })?;

        let format = {
            let mut state_guard = self.state.lock().unwrap();
            let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;
            state.format
        };

        let cc_packet = match self.detect(format, map.as_slice()) {
            Ok(v) => v,
            Err(e) => {
                gst::warning!(CAT, imp = self, "{e}");
                gst::element_imp_warning!(self, gst::StreamError::Decode, ["{e}"]);
                CCPacketContents {
                    cc608: false,
                    cc708: false,
                }
            }
        };

        self.maybe_update_properties(pts, cc_packet)
            .map_err(|_| gst::FlowError::Error)?;

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, event: gst::Event) -> bool {
        match event.view() {
            gst::event::EventView::Gap(gap) => {
                let _ = self.maybe_update_properties(
                    gap.get().0,
                    CCPacketContents {
                        cc608: false,
                        cc708: false,
                    },
                );
                self.parent_sink_event(event)
            }
            _ => self.parent_sink_event(event),
        }
    }

    fn set_caps(&self, incaps: &gst::Caps, outcaps: &gst::Caps) -> Result<(), gst::LoggableError> {
        if incaps != outcaps {
            return Err(gst::loggable_error!(
                CAT,
                "Input and output caps are not the same"
            ));
        }

        let s = incaps
            .structure(0)
            .ok_or_else(|| gst::loggable_error!(CAT, "Failed to parse input caps"))?;
        let format_str = s
            .get::<Option<&str>>("format")
            .map_err(|_| gst::loggable_error!(CAT, "Failed to parse input caps"))?
            .ok_or_else(|| gst::loggable_error!(CAT, "Failed to parse input caps"))?;
        let cc_format = match format_str {
            "cdp" => CCFormat::Cc708Cdp,
            "cc_data" => CCFormat::Cc708CcData,
            _ => return Err(gst::loggable_error!(CAT, "Failed to parse input caps")),
        };

        *self.state.lock().unwrap() = Some(State {
            format: cc_format,
            last_cc608_change: gst::ClockTime::NONE,
            last_cc708_change: gst::ClockTime::NONE,
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        // Drop state
        let _ = self.state.lock().unwrap().take();

        Ok(())
    }
}
