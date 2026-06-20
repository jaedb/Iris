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
const DEFAULT_HUE_REF: f32 = 0.0;
const DEFAULT_HUE_VAR: f32 = 10.0;
const DEFAULT_SATURATION_REF: f32 = 0.0;
const DEFAULT_SATURATION_VAR: f32 = 0.15;
const DEFAULT_VALUE_REF: f32 = 0.0;
const DEFAULT_VALUE_VAR: f32 = 0.3;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    hue_ref: f32,
    hue_var: f32,
    saturation_ref: f32,
    saturation_var: f32,
    value_ref: f32,
    value_var: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hue_ref: DEFAULT_HUE_REF,
            hue_var: DEFAULT_HUE_VAR,
            saturation_ref: DEFAULT_SATURATION_REF,
            saturation_var: DEFAULT_SATURATION_VAR,
            value_ref: DEFAULT_VALUE_REF,
            value_var: DEFAULT_VALUE_VAR,
        }
    }
}

// Struct containing all the element data
#[derive(Default)]
pub struct HsvDetector {
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hsvdetector",
        gst::DebugColorFlags::empty(),
        Some("Rust HSV-based detection filter"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for HsvDetector {
    const NAME: &'static str = "GstHsvDetector";
    type Type = super::HsvDetector;
    type ParentType = gst_video::VideoFilter;
}

fn video_input_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    [
        gst_video::VideoFormat::Rgbx,
        gst_video::VideoFormat::Xrgb,
        gst_video::VideoFormat::Bgrx,
        gst_video::VideoFormat::Xbgr,
        gst_video::VideoFormat::Rgb,
        gst_video::VideoFormat::Bgr,
    ]
}

fn video_output_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    [
        gst_video::VideoFormat::Rgba,
        gst_video::VideoFormat::Argb,
        gst_video::VideoFormat::Bgra,
        gst_video::VideoFormat::Abgr,
    ]
}

impl HsvDetector {
    #[inline]
    fn hsv_detect<CF, DF>(
        &self,
        in_frame: &gst_video::video_frame::VideoFrameRef<&gst::buffer::BufferRef>,
        out_frame: &mut gst_video::video_frame::VideoFrameRef<&mut gst::buffer::BufferRef>,
        to_hsv: CF,
        apply_alpha: DF,
    ) where
        CF: Fn(&[u8]) -> [f32; 3],
        DF: Fn(&[u8], &mut [u8], u8),
    {
        let settings = self.settings.lock().unwrap();

        // Keep the various metadata we need for working with the video frames in
        // local variables. This saves some typing below.
        let width = in_frame.width() as usize;
        let in_stride = in_frame.plane_stride()[0] as usize;
        let in_data = in_frame.plane_data(0).unwrap();
        let out_stride = out_frame.plane_stride()[0] as usize;
        let out_data = out_frame.plane_data_mut(0).unwrap();
        let nb_input_channels = in_frame.format_info().pixel_stride()[0] as usize;

        assert_eq!(out_data.len() / out_stride, in_data.len() / in_stride);
        assert_eq!(in_data.len() % nb_input_channels, 0);

        let in_line_bytes = width * nb_input_channels;
        let out_line_bytes = width * 4;

        assert!(in_line_bytes <= in_stride);
        assert!(out_line_bytes <= out_stride);

        for (in_line, out_line) in in_data
            .chunks_exact(in_stride)
            .zip(out_data.chunks_exact_mut(out_stride))
        {
            for (in_p, out_p) in in_line[..in_line_bytes]
                .chunks_exact(nb_input_channels)
                .zip(out_line[..out_line_bytes].chunks_exact_mut(4))
            {
                let hsv = to_hsv(in_p);

                // We handle hue being circular here
                let ref_hue_offset = 180.0 - settings.hue_ref;
                let mut shifted_hue = hsv[0] + ref_hue_offset;

                if shifted_hue < 0.0 {
                    shifted_hue += 360.0;
                }

                shifted_hue %= 360.0;

                if (shifted_hue - 180.0).abs() <= settings.hue_var
                    && (hsv[1] - settings.saturation_ref).abs() <= settings.saturation_var
                    && (hsv[2] - settings.value_ref).abs() <= settings.value_var
                {
                    apply_alpha(in_p, out_p, 255);
                } else {
                    apply_alpha(in_p, out_p, 0);
                };
            }
        }
    }
}

impl ObjectImpl for HsvDetector {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecFloat::builder("hue-ref")
                    .nick("Hue reference")
                    .blurb("Hue reference in degrees")
                    .default_value(DEFAULT_HUE_REF)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("hue-var")
                    .nick("Hue variation")
                    .blurb("Allowed hue variation from the reference hue angle, in degrees")
                    .minimum(0.0)
                    .maximum(180.0)
                    .default_value(DEFAULT_HUE_VAR)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("saturation-ref")
                    .nick("Saturation reference")
                    .blurb("Reference saturation value")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_SATURATION_REF)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("saturation-var")
                    .nick("Saturation variation")
                    .blurb("Allowed saturation variation from the reference value")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_SATURATION_VAR)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("value-ref")
                    .nick("Value reference")
                    .blurb("Reference value value")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_VALUE_REF)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("value-var")
                    .nick("Value variation")
                    .blurb("Allowed value variation from the reference value")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_VALUE_VAR)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "hue-ref" => {
                let mut settings = self.settings.lock().unwrap();
                let hue_ref = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing hue-ref from {} to {}",
                    settings.hue_ref,
                    hue_ref
                );
                settings.hue_ref = hue_ref;
            }
            "hue-var" => {
                let mut settings = self.settings.lock().unwrap();
                let hue_var = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing hue-var from {} to {}",
                    settings.hue_var,
                    hue_var
                );
                settings.hue_var = hue_var;
            }
            "saturation-ref" => {
                let mut settings = self.settings.lock().unwrap();
                let saturation_ref = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing saturation-ref from {} to {}",
                    settings.saturation_ref,
                    saturation_ref
                );
                settings.saturation_ref = saturation_ref;
            }
            "saturation-var" => {
                let mut settings = self.settings.lock().unwrap();
                let saturation_var = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing saturation-var from {} to {}",
                    settings.saturation_var,
                    saturation_var
                );
                settings.saturation_var = saturation_var;
            }
            "value-ref" => {
                let mut settings = self.settings.lock().unwrap();
                let value_ref = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing value-ref from {} to {}",
                    settings.value_ref,
                    value_ref
                );
                settings.value_ref = value_ref;
            }
            "value-var" => {
                let mut settings = self.settings.lock().unwrap();
                let value_var = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing value-var from {} to {}",
                    settings.value_var,
                    value_var
                );
                settings.value_var = value_var;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "hue-ref" => {
                let settings = self.settings.lock().unwrap();
                settings.hue_ref.to_value()
            }
            "hue-var" => {
                let settings = self.settings.lock().unwrap();
                settings.hue_var.to_value()
            }
            "saturation-ref" => {
                let settings = self.settings.lock().unwrap();
                settings.saturation_ref.to_value()
            }
            "saturation-var" => {
                let settings = self.settings.lock().unwrap();
                settings.saturation_var.to_value()
            }
            "value-ref" => {
                let settings = self.settings.lock().unwrap();
                settings.value_ref.to_value()
            }
            "value-var" => {
                let settings = self.settings.lock().unwrap();
                settings.value_var.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for HsvDetector {}

impl ElementImpl for HsvDetector {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HSV detector",
                "Filter/Effect/Converter/Video",
                "Works within the HSV colorspace to mark positive pixels",
                "Julien Bardagi <julien.bardagi@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list(video_output_formats())
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            // sink pad capabilities
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list(video_input_formats())
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
}

impl BaseTransformImpl for HsvDetector {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn transform_caps(
        &self,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let mut other_caps = caps.clone();
        if direction == gst::PadDirection::Src {
            for s in other_caps.make_mut().iter_mut() {
                s.set("format", gst::List::new(video_input_formats()));
            }
        } else {
            for s in other_caps.make_mut().iter_mut() {
                s.set("format", gst::List::new(video_output_formats()));
            }
        };

        gst::debug!(
            CAT,
            imp = self,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        // In the end we need to filter the caps through an optional filter caps to get rid of any
        // unwanted caps.
        if let Some(filter) = filter {
            Some(filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First))
        } else {
            Some(other_caps)
        }
    }
}

impl VideoFilterImpl for HsvDetector {
    fn transform_frame(
        &self,
        in_frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
        out_frame: &mut gst_video::VideoFrameRef<&mut gst::BufferRef>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        match in_frame.format() {
            gst_video::VideoFormat::Rgbx | gst_video::VideoFormat::Rgb => {
                match out_frame.format() {
                    gst_video::VideoFormat::Rgba => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[..3].copy_from_slice(&in_p[..3]);
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Argb => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1..4].copy_from_slice(&in_p[..3]);
                                out_p[0] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Bgra => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[0] = in_p[2];
                                out_p[1] = in_p[1];
                                out_p[2] = in_p[0];
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Abgr => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1] = in_p[2];
                                out_p[2] = in_p[1];
                                out_p[3] = in_p[0];
                                out_p[0] = val;
                            },
                        );
                    }
                    _ => unreachable!(),
                }
            }
            gst_video::VideoFormat::Xrgb => {
                match out_frame.format() {
                    gst_video::VideoFormat::Rgba => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[1..4].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[..3].copy_from_slice(&in_p[1..4]);
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Argb => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[1..4].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1..4].copy_from_slice(&in_p[1..4]);
                                out_p[0] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Bgra => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[1..4].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[0] = in_p[3];
                                out_p[1] = in_p[2];
                                out_p[2] = in_p[1];
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Abgr => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_rgb(
                                    in_p[1..4].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1] = in_p[3];
                                out_p[2] = in_p[2];
                                out_p[3] = in_p[1];
                                out_p[0] = val;
                            },
                        );
                    }
                    _ => unreachable!(),
                };
            }
            gst_video::VideoFormat::Bgrx | gst_video::VideoFormat::Bgr => {
                match out_frame.format() {
                    gst_video::VideoFormat::Rgba => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_bgr(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[0] = in_p[2];
                                out_p[1] = in_p[1];
                                out_p[2] = in_p[0];
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Argb => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_bgr(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1] = in_p[2];
                                out_p[2] = in_p[1];
                                out_p[3] = in_p[0];
                                out_p[0] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Bgra => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_bgr(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[..3].copy_from_slice(&in_p[..3]);
                                out_p[3] = val;
                            },
                        );
                    }
                    gst_video::VideoFormat::Abgr => {
                        self.hsv_detect(
                            in_frame,
                            out_frame,
                            |in_p| {
                                hsvutils::from_bgr(
                                    in_p[..3].try_into().expect("slice with incorrect length"),
                                )
                            },
                            |in_p, out_p, val| {
                                out_p[1..4].copy_from_slice(&in_p[..3]);
                                out_p[0] = val;
                            },
                        );
                    }
                    _ => unreachable!(),
                }
            }
            gst_video::VideoFormat::Xbgr => match out_frame.format() {
                gst_video::VideoFormat::Rgba => {
                    self.hsv_detect(
                        in_frame,
                        out_frame,
                        |in_p| {
                            hsvutils::from_bgr(
                                in_p[1..4].try_into().expect("slice with incorrect length"),
                            )
                        },
                        |in_p, out_p, val| {
                            out_p[0] = in_p[3];
                            out_p[1] = in_p[2];
                            out_p[2] = in_p[1];
                            out_p[3] = val;
                        },
                    );
                }
                gst_video::VideoFormat::Argb => {
                    self.hsv_detect(
                        in_frame,
                        out_frame,
                        |in_p| {
                            hsvutils::from_bgr(
                                in_p[1..4].try_into().expect("slice with incorrect length"),
                            )
                        },
                        |in_p, out_p, val| {
                            out_p[1] = in_p[3];
                            out_p[2] = in_p[2];
                            out_p[3] = in_p[1];
                            out_p[0] = val;
                        },
                    );
                }
                gst_video::VideoFormat::Bgra => {
                    self.hsv_detect(
                        in_frame,
                        out_frame,
                        |in_p| {
                            hsvutils::from_bgr(
                                in_p[1..4].try_into().expect("slice with incorrect length"),
                            )
                        },
                        |in_p, out_p, val| {
                            out_p[..3].copy_from_slice(&in_p[1..4]);
                            out_p[3] = val;
                        },
                    );
                }
                gst_video::VideoFormat::Abgr => {
                    self.hsv_detect(
                        in_frame,
                        out_frame,
                        |in_p| {
                            hsvutils::from_bgr(
                                in_p[1..4].try_into().expect("slice with incorrect length"),
                            )
                        },
                        |in_p, out_p, val| {
                            out_p[1..4].copy_from_slice(&in_p[1..4]);
                            out_p[0] = val;
                        },
                    );
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        Ok(gst::FlowSuccess::Ok)
    }
}
