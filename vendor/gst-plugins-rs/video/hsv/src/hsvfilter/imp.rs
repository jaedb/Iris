// Copyright (C) 2020 Julien Bardagi <julien.bardagi@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;

use std::sync::Mutex;

use std::sync::LazyLock;

use super::super::hsvutils;

// Default values of properties
const DEFAULT_HUE_SHIFT: f32 = 0.0;
const DEFAULT_SATURATION_MUL: f32 = 1.0;
const DEFAULT_SATURATION_OFF: f32 = 0.0;
const DEFAULT_VALUE_MUL: f32 = 1.0;
const DEFAULT_VALUE_OFF: f32 = 0.0;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    hue_shift: f32,
    saturation_mul: f32,
    saturation_off: f32,
    value_mul: f32,
    value_off: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hue_shift: DEFAULT_HUE_SHIFT,
            saturation_mul: DEFAULT_SATURATION_MUL,
            saturation_off: DEFAULT_SATURATION_OFF,
            value_mul: DEFAULT_VALUE_MUL,
            value_off: DEFAULT_VALUE_OFF,
        }
    }
}

// Struct containing all the element data
#[derive(Default)]
pub struct HsvFilter {
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hsvfilter",
        gst::DebugColorFlags::empty(),
        Some("Rust HSV transformation filter"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for HsvFilter {
    const NAME: &'static str = "GstHsvFilter";
    type Type = super::HsvFilter;
    type ParentType = gst_video::VideoFilter;
}

impl HsvFilter {
    #[inline]
    fn hsv_filter<CF, FF>(
        &self,
        frame: &mut gst_video::video_frame::VideoFrameRef<&mut gst::buffer::BufferRef>,
        to_hsv: CF,
        apply_filter: FF,
    ) where
        CF: Fn(&[u8]) -> [f32; 3],
        FF: Fn(&[f32; 3], &mut [u8]),
    {
        let settings = *self.settings.lock().unwrap();

        let width = frame.width() as usize;
        let stride = frame.plane_stride()[0] as usize;
        let nb_channels = frame.format_info().pixel_stride()[0] as usize;
        let data = frame.plane_data_mut(0).unwrap();

        assert_eq!(data.len() % nb_channels, 0);

        let line_bytes = width * nb_channels;

        for line in data.chunks_exact_mut(stride) {
            for p in line[..line_bytes].chunks_exact_mut(nb_channels) {
                assert_eq!(p.len(), nb_channels);

                let mut hsv = to_hsv(p);

                hsv[0] = (hsv[0] + settings.hue_shift) % 360.0;
                if hsv[0] < 0.0 {
                    hsv[0] += 360.0;
                }
                hsv[1] = hsvutils::Clamp::clamp(
                    settings.saturation_mul * hsv[1] + settings.saturation_off,
                    0.0,
                    1.0,
                );
                hsv[2] = hsvutils::Clamp::clamp(
                    settings.value_mul * hsv[2] + settings.value_off,
                    0.0,
                    1.0,
                );

                apply_filter(&hsv, p);
            }
        }
    }
}

impl ObjectImpl for HsvFilter {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecFloat::builder("hue-shift")
                    .nick("Hue shift")
                    .blurb("Hue shifting in degrees")
                    .default_value(DEFAULT_HUE_SHIFT)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("saturation-mul")
                    .nick("Saturation multiplier")
                    .blurb("Saturation multiplier to apply to the saturation value (before offset)")
                    .default_value(DEFAULT_SATURATION_MUL)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("saturation-off")
                    .nick("Saturation offset")
                    .blurb("Saturation offset to add to the saturation value (after multiplier)")
                    .default_value(DEFAULT_SATURATION_OFF)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("value-mul")
                    .nick("Value multiplier")
                    .blurb("Value multiplier to apply to the value (before offset)")
                    .default_value(DEFAULT_VALUE_MUL)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("value-off")
                    .nick("Value offset")
                    .blurb("Value offset to add to the value (after multiplier)")
                    .default_value(DEFAULT_VALUE_OFF)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "hue-shift" => {
                let mut settings = self.settings.lock().unwrap();
                let hue_shift = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing hue-shift from {} to {}",
                    settings.hue_shift,
                    hue_shift
                );
                settings.hue_shift = hue_shift;
            }
            "saturation-mul" => {
                let mut settings = self.settings.lock().unwrap();
                let saturation_mul = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing saturation-mul from {} to {}",
                    settings.saturation_mul,
                    saturation_mul
                );
                settings.saturation_mul = saturation_mul;
            }
            "saturation-off" => {
                let mut settings = self.settings.lock().unwrap();
                let saturation_off = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing saturation-off from {} to {}",
                    settings.saturation_off,
                    saturation_off
                );
                settings.saturation_off = saturation_off;
            }
            "value-mul" => {
                let mut settings = self.settings.lock().unwrap();
                let value_mul = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing value-mul from {} to {}",
                    settings.value_mul,
                    value_mul
                );
                settings.value_mul = value_mul;
            }
            "value-off" => {
                let mut settings = self.settings.lock().unwrap();
                let value_off = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing value-off from {} to {}",
                    settings.value_off,
                    value_off
                );
                settings.value_off = value_off;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "hue-shift" => {
                let settings = self.settings.lock().unwrap();
                settings.hue_shift.to_value()
            }
            "saturation-mul" => {
                let settings = self.settings.lock().unwrap();
                settings.saturation_mul.to_value()
            }
            "saturation-off" => {
                let settings = self.settings.lock().unwrap();
                settings.saturation_off.to_value()
            }
            "value-mul" => {
                let settings = self.settings.lock().unwrap();
                settings.value_mul.to_value()
            }
            "value-off" => {
                let settings = self.settings.lock().unwrap();
                settings.value_off.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for HsvFilter {}

impl ElementImpl for HsvFilter {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HSV filter",
                "Filter/Effect/Converter/Video",
                "Works within the HSV colorspace to apply transformations to incoming frames",
                "Julien Bardagi <julien.bardagi@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // src pad capabilities
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list([
                    gst_video::VideoFormat::Rgbx,
                    gst_video::VideoFormat::Xrgb,
                    gst_video::VideoFormat::Bgrx,
                    gst_video::VideoFormat::Xbgr,
                    gst_video::VideoFormat::Rgba,
                    gst_video::VideoFormat::Argb,
                    gst_video::VideoFormat::Bgra,
                    gst_video::VideoFormat::Abgr,
                    gst_video::VideoFormat::Rgb,
                    gst_video::VideoFormat::Bgr,
                ])
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

impl BaseTransformImpl for HsvFilter {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;
}

impl VideoFilterImpl for HsvFilter {
    fn transform_frame_ip(
        &self,
        frame: &mut gst_video::VideoFrameRef<&mut gst::BufferRef>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        match frame.format() {
            gst_video::VideoFormat::Rgbx
            | gst_video::VideoFormat::Rgba
            | gst_video::VideoFormat::Rgb => {
                self.hsv_filter(
                    frame,
                    |p| hsvutils::from_rgb(p[..3].try_into().expect("slice with incorrect length")),
                    |hsv, p| {
                        p[..3].copy_from_slice(&hsvutils::to_rgb(hsv));
                    },
                );
            }
            gst_video::VideoFormat::Xrgb | gst_video::VideoFormat::Argb => {
                self.hsv_filter(
                    frame,
                    |p| {
                        hsvutils::from_rgb(p[1..4].try_into().expect("slice with incorrect length"))
                    },
                    |hsv, p| {
                        p[1..4].copy_from_slice(&hsvutils::to_rgb(hsv));
                    },
                );
            }
            gst_video::VideoFormat::Bgrx
            | gst_video::VideoFormat::Bgra
            | gst_video::VideoFormat::Bgr => {
                self.hsv_filter(
                    frame,
                    |p| hsvutils::from_bgr(p[..3].try_into().expect("slice with incorrect length")),
                    |hsv, p| {
                        p[..3].copy_from_slice(&hsvutils::to_bgr(hsv));
                    },
                );
            }
            gst_video::VideoFormat::Xbgr | gst_video::VideoFormat::Abgr => {
                self.hsv_filter(
                    frame,
                    |p| {
                        hsvutils::from_bgr(p[1..4].try_into().expect("slice with incorrect length"))
                    },
                    |hsv, p| {
                        p[1..4].copy_from_slice(&hsvutils::to_bgr(hsv));
                    },
                );
            }
            _ => unreachable!(),
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
