// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
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
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;

use parking_lot::Mutex;
use std::sync::LazyLock;

use super::CompressionLevel;
use super::Filter;

const DEFAULT_COMPRESSION_LEVEL: CompressionLevel = CompressionLevel::Default;
const DEFAULT_FILTER_TYPE: Filter = Filter::Adaptive;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rspngenc",
        gst::DebugColorFlags::empty(),
        Some("PNG encoder"),
    )
});

#[derive(Debug, Clone, Copy)]
struct Settings {
    compression: CompressionLevel,
    filter: Filter,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            compression: DEFAULT_COMPRESSION_LEVEL,
            filter: DEFAULT_FILTER_TYPE,
        }
    }
}

struct State {
    video_info: gst_video::VideoInfo,
}

#[derive(Default)]
pub struct PngEncoder {
    state: Mutex<Option<State>>,
    settings: Mutex<Settings>,
}

#[glib::object_subclass]
impl ObjectSubclass for PngEncoder {
    const NAME: &'static str = "GstRsPngEnc";
    type Type = super::PngEncoder;
    type ParentType = gst_video::VideoEncoder;
}

impl ObjectImpl for PngEncoder {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default(
                    "compression-level",
                    DEFAULT_COMPRESSION_LEVEL,
                )
                .nick("Compression level")
                .blurb("Selects the compression algorithm to use")
                .mutable_ready()
                .build(),
                glib::ParamSpecEnum::builder_with_default("filter", DEFAULT_FILTER_TYPE)
                    .nick("Filter")
                    .blurb("Selects the filter type to applied")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "compression-level" => {
                let mut settings = self.settings.lock();
                settings.compression = value
                    .get::<CompressionLevel>()
                    .expect("type checked upstream");
            }
            "filter" => {
                let mut settings = self.settings.lock();
                settings.filter = value.get::<Filter>().expect("type checked upstream");
            }
            _ => unreachable!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "compression-level" => {
                let settings = self.settings.lock();
                settings.compression.to_value()
            }
            "filter" => {
                let settings = self.settings.lock();
                settings.filter.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for PngEncoder {}

impl ElementImpl for PngEncoder {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "PNG encoder",
                "Encoder/Video",
                "PNG encoder",
                "Natanael Mojica <neithanmo@gmail>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst_video::VideoCapsBuilder::new()
                .format_list([
                    gst_video::VideoFormat::Gray8,
                    gst_video::VideoFormat::Gray16Be,
                    gst_video::VideoFormat::Rgb,
                    gst_video::VideoFormat::Rgba,
                ])
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("image/png").build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl VideoEncoderImpl for PngEncoder {
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.lock() = None;
        Ok(())
    }

    fn set_format(
        &self,
        state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let video_info = state.info().clone();
        gst::debug!(CAT, imp = self, "Setting format {:?}", video_info);

        *self.state.lock() = Some(State { video_info });

        let instance = self.obj();
        let output_state = instance
            .set_output_state(gst::Caps::builder("image/png").build(), Some(state))
            .map_err(|_| gst::loggable_error!(CAT, "Failed to set output state"))?;
        instance
            .negotiate(output_state)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to negotiate"))
    }

    fn handle_frame(
        &self,
        mut frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = *self.settings.lock();
        let mut state_guard = self.state.lock();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        gst::debug!(
            CAT,
            imp = self,
            "Sending frame {}",
            frame.system_frame_number()
        );

        let mut buffer = Vec::with_capacity(4096);
        let mut encoder = png::Encoder::new(
            &mut buffer,
            state.video_info.width(),
            state.video_info.height(),
        );
        let color = match state.video_info.format() {
            gst_video::VideoFormat::Gray8 | gst_video::VideoFormat::Gray16Be => {
                png::ColorType::Grayscale
            }
            gst_video::VideoFormat::Rgb => png::ColorType::Rgb,
            gst_video::VideoFormat::Rgba => png::ColorType::Rgba,
            _ => unreachable!(),
        };
        let depth = if state.video_info.format() == gst_video::VideoFormat::Gray16Be {
            png::BitDepth::Sixteen
        } else {
            png::BitDepth::Eight
        };

        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder.set_compression(png::Compression::from(settings.compression));
        encoder.set_filter(png::Filter::from(settings.filter));

        let mut writer = encoder.write_header().map_err(|e| {
            gst::error!(CAT, imp = self, "Failed to create encoder: {e}");
            gst::element_imp_error!(self, gst::CoreError::Failed, ["{e}"]);
            gst::FlowError::Error
        })?;

        {
            let input_buffer = frame.input_buffer().expect("frame without input buffer");
            let input_map = input_buffer.map_readable().unwrap();
            writer.write_image_data(&input_map).map_err(|e| {
                gst::error!(CAT, imp = self, "Failed to write image data: {e}");
                gst::element_imp_error!(self, gst::CoreError::Failed, ["{e}"]);
                gst::FlowError::Error
            })?;
        }

        writer.finish().map_err(|e| {
            gst::error!(CAT, imp = self, "Failed to finish encoder: {e}");
            gst::element_imp_error!(self, gst::CoreError::Failed, ["{e}"]);
            gst::FlowError::Error
        })?;

        drop(state_guard);

        let output_buffer = gst::Buffer::from_mut_slice(buffer);
        // There are no such incremental frames in the png format
        frame.set_flags(gst_video::VideoCodecFrameFlags::SYNC_POINT);
        frame.set_output_buffer(output_buffer);
        self.obj().finish_frame(frame)
    }
}
