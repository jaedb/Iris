// Copyright (C) 2022, Igalia S.L
//      Author: Philippe Normand <philn@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;
use color_name;
use color_thief::{get_palette, Color, ColorFormat};
use gst::{glib, subclass::prelude::*};
use gst_base::prelude::*;
use gst_video::{subclass::prelude::*, VideoFormat};
use std::sync::LazyLock;
use std::sync::Mutex;

const DEFAULT_QUALITY: u32 = 10;
const DEFAULT_MAX_COLORS: u32 = 2;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "colordetect",
        gst::DebugColorFlags::empty(),
        Some("Dominant color detection"),
    )
});

#[derive(Debug, Clone, Copy)]
struct Settings {
    quality: u32,
    max_colors: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quality: DEFAULT_QUALITY,
            max_colors: DEFAULT_MAX_COLORS,
        }
    }
}

struct State {
    color_format: ColorFormat,
    current_color: Option<String>,
}

#[derive(Default)]
pub struct ColorDetect {
    settings: Mutex<Settings>,
    state: AtomicRefCell<Option<State>>,
}

impl ColorDetect {
    fn detect_color(
        &self,
        frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
    ) -> Result<Option<(String, Vec<Color>)>, gst::FlowError> {
        let mut state_guard = self.state.borrow_mut();
        let state = state_guard.as_mut().ok_or_else(|| {
            gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no state yet"]);
            gst::FlowError::NotNegotiated
        })?;

        let settings = *self.settings.lock().unwrap();
        let palette = get_palette(
            frame.plane_data(0).unwrap(),
            state.color_format,
            settings.quality as u8,
            settings.max_colors as u8,
        )
        .map_err(|_| gst::FlowError::Error)?;

        let dominant_color = palette[0];
        let dominant_color_name =
            color_name::css::Color::similar([dominant_color.r, dominant_color.g, dominant_color.b])
                .to_lowercase();
        if state.current_color.as_ref() != Some(&dominant_color_name) {
            let name = dominant_color_name.clone();
            state.current_color = Some(dominant_color_name);
            return Ok(Some((name, palette)));
        }
        Ok(None)
    }

    fn color_changed(&self, dominant_color_name: &str, palette: Vec<Color>) {
        gst::debug!(
            CAT,
            imp = self,
            "Dominant color changed to {}",
            dominant_color_name
        );
        let palette_colors = gst::List::new(
            palette
                .iter()
                .map(|c| ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)),
        );

        self.obj()
            .post_message(
                gst::message::Element::builder(
                    gst::structure::Structure::builder("colordetect")
                        .field("dominant-color", dominant_color_name)
                        .field("palette", palette_colors)
                        .build(),
                )
                .build(),
            )
            .expect("Element without bus. Should not happen!");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ColorDetect {
    const NAME: &'static str = "GstColorDetect";
    type Type = super::ColorDetect;
    type ParentType = gst_video::VideoFilter;
}

impl ObjectImpl for ColorDetect {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("quality")
                    .nick("Quality of an output colors")
                    .blurb("A step in pixels to improve performance")
                    .maximum(10)
                    .default_value(DEFAULT_QUALITY)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("max-colors")
                    .nick("Number of colors in the output palette")
                    .blurb("Actual colors count can be lower depending on the image")
                    .minimum(2)
                    .maximum(255)
                    .default_value(DEFAULT_MAX_COLORS)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "quality" => {
                let mut settings = self.settings.lock().unwrap();
                let quality = value.get().expect("type checked upstream");
                if settings.quality != quality {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing quality from {} to {}",
                        settings.quality,
                        quality
                    );
                    settings.quality = quality;
                }
            }
            "max-colors" => {
                let mut settings = self.settings.lock().unwrap();
                let max_colors = value.get().expect("type checked upstream");
                if settings.max_colors != max_colors {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing max_colors from {} to {}",
                        settings.max_colors,
                        max_colors
                    );
                    settings.max_colors = max_colors;
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "quality" => {
                let settings = self.settings.lock().unwrap();
                settings.quality.to_value()
            }
            "max-colors" => {
                let settings = self.settings.lock().unwrap();
                settings.max_colors.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for ColorDetect {}

impl ElementImpl for ColorDetect {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Dominant color detection",
                "Filter/Video",
                "Detects the dominant color of a video",
                "Philippe Normand <philn@igalia.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list([
                    VideoFormat::Rgb,
                    VideoFormat::Rgba,
                    VideoFormat::Argb,
                    VideoFormat::Bgr,
                    VideoFormat::Bgra,
                ])
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseTransformImpl for ColorDetect {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = true;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = None;
        gst::info!(CAT, imp = self, "Stopped");
        Ok(())
    }
}

impl VideoFilterImpl for ColorDetect {
    fn set_info(
        &self,
        incaps: &gst::Caps,
        in_info: &gst_video::VideoInfo,
        outcaps: &gst::Caps,
        _out_info: &gst_video::VideoInfo,
    ) -> Result<(), gst::LoggableError> {
        gst::debug!(
            CAT,
            imp = self,
            "Configured for caps {} to {}",
            incaps,
            outcaps
        );

        let color_format = match in_info.format() {
            VideoFormat::Rgb => ColorFormat::Rgb,
            VideoFormat::Rgba => ColorFormat::Rgba,
            VideoFormat::Argb => ColorFormat::Argb,
            VideoFormat::Bgr => ColorFormat::Bgr,
            VideoFormat::Bgra => ColorFormat::Bgra,
            _ => unimplemented!(),
        };

        let previous_color = match self.state.borrow().as_ref() {
            Some(state) => state.current_color.clone(),
            None => None,
        };
        *self.state.borrow_mut() = Some(State {
            color_format,
            current_color: previous_color,
        });

        Ok(())
    }

    fn transform_frame_ip_passthrough(
        &self,
        frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if let Some((dominant_color_name, palette)) = self.detect_color(frame)? {
            self.color_changed(&dominant_color_name, palette);
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
