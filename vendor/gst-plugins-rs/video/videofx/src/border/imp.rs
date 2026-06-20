// Copyright (C) 2021, Daily
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, subclass::prelude::*};
use gst_base::{
    prelude::*,
    subclass::base_transform::{InputBuffer, PrepareOutputBufferSuccess},
};
use gst_video::{subclass::prelude::*, VideoFormat};

use std::sync::LazyLock;
use std::sync::Mutex;

const DEFAULT_BORDER_RADIUS: u32 = 0;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "roundedcorners",
        gst::DebugColorFlags::empty(),
        Some("Rounded corners"),
    )
});

#[derive(Debug, Clone, Copy)]
struct Settings {
    border_radius_px: u32,
    changed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            border_radius_px: DEFAULT_BORDER_RADIUS,
            changed: false,
        }
    }
}

struct State {
    alpha_mem: gst::Memory,
    out_info: Option<gst_video::VideoInfo>,
}

#[derive(Default)]
pub struct RoundedCorners {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

impl RoundedCorners {
    fn draw_rounded_corners(
        &self,
        cairo_ctx: &cairo::Context,
        border_radius_px: u32,
        width: f64,
        height: f64,
    ) -> Result<(), cairo::Error> {
        let border_radius = border_radius_px as f64;
        let degrees = std::f64::consts::PI / 180.0;

        // Taken from https://www.cairographics.org/samples/rounded_rectangle/
        cairo_ctx.new_sub_path();
        cairo_ctx.arc(
            width - border_radius,
            border_radius,
            border_radius,
            -90_f64 * degrees,
            0 as f64 * degrees,
        );
        cairo_ctx.arc(
            width - border_radius,
            height - border_radius,
            border_radius,
            0 as f64 * degrees,
            90_f64 * degrees,
        );
        cairo_ctx.arc(
            border_radius,
            height - border_radius,
            border_radius,
            90_f64 * degrees,
            180_f64 * degrees,
        );
        cairo_ctx.arc(
            border_radius,
            border_radius,
            border_radius,
            180_f64 * degrees,
            270_f64 * degrees,
        );
        cairo_ctx.close_path();

        cairo_ctx.set_source_rgb(0.0, 0.0, 0.0);
        cairo_ctx.fill_preserve()?;
        cairo_ctx.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cairo_ctx.set_line_width(1.0);
        cairo_ctx.stroke()?;

        Ok(())
    }

    fn generate_alpha_mask(&self, border_radius_px: u32) -> Result<(), gst::LoggableError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().unwrap();

        let out_info = state.out_info.as_ref().unwrap();
        let width = out_info.width() as i32;
        let height = out_info.height() as i32;
        let alpha_stride = out_info.stride()[3];

        let mem = &mut state.alpha_mem;
        let mut_mem = mem.make_mut();

        let mut alpha_mem = mut_mem
            .map_writable()
            .map_err(|_| gst::loggable_error!(CAT, "Failed to map alpha memory as writable"))?;
        if border_radius_px == 0 {
            // Border radius is 0 but output needs to have an alpha plane. Attach an opaque alpha
            // plane here and just return.
            alpha_mem.fill(0xff);
            return Ok(());
        }

        alpha_mem.fill(0);
        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                alpha_mem.as_mut_slice().as_mut_ptr(),
                cairo::Format::A8,
                width,
                height,
                alpha_stride,
            )
        };

        if let Err(e) = surface {
            return Err(gst::loggable_error!(
                CAT,
                "Failed to create cairo image surface: {}",
                e
            ));
        }

        match cairo::Context::new(surface.as_ref().unwrap()) {
            Ok(cr) => {
                if let Err(e) =
                    self.draw_rounded_corners(&cr, border_radius_px, width as f64, height as f64)
                {
                    return Err(gst::loggable_error!(
                        CAT,
                        "Failed to draw rounded corners: {}",
                        e
                    ));
                };

                drop(cr);

                unsafe {
                    assert_eq!(
                        cairo::ffi::cairo_surface_get_reference_count(
                            surface.unwrap().to_raw_none()
                        ),
                        1
                    );
                }

                Ok(())
            }
            Err(e) => Err(gst::loggable_error!(
                CAT,
                "Failed to create cairo context: {}",
                e
            )),
        }
    }

    fn add_video_meta(
        &self,
        buf: &mut gst::BufferRef,
        out_info: &gst_video::VideoInfo,
        alpha_plane_offset: usize,
        input_buffer_writable: bool,
    ) -> Result<PrepareOutputBufferSuccess, gst::FlowError> {
        let mut strides: [i32; 4] = [0; 4];
        let mut offsets: [usize; 4] = [0; 4];

        match buf.meta_mut::<gst_video::VideoMeta>() {
            Some(meta) => {
                let video_frame_flags = meta.video_frame_flags();
                let n_planes = meta.n_planes() as usize;
                offsets[..n_planes].clone_from_slice(&meta.offset()[..n_planes]);
                strides[..n_planes].clone_from_slice(&meta.stride()[..n_planes]);

                offsets[3] = alpha_plane_offset;
                strides[3] = out_info.stride()[3];

                match meta.remove() {
                    Err(_) => {
                        // We could not remove the meta, probably because it was locked. We need to
                        // create a new buffer and copy over all memories, metadata (timestamps, etc)
                        // and metas (except for videometa) of the original buffer into a newly allocated buffer.
                        let copy_flags = gst::BufferCopyFlags::FLAGS
                            | gst::BufferCopyFlags::TIMESTAMPS
                            | gst::BufferCopyFlags::MEMORY;
                        let mut buf = buf.copy_region(copy_flags, ..).unwrap();
                        let mut_buf = buf.make_mut();
                        gst_video::VideoMeta::add_full(
                            mut_buf,
                            video_frame_flags,
                            out_info.format(),
                            out_info.width(),
                            out_info.height(),
                            &offsets,
                            &strides,
                        )
                        .unwrap();

                        Ok(PrepareOutputBufferSuccess::Buffer(mut_buf.to_owned()))
                    }
                    Ok(_) => {
                        gst_video::VideoMeta::add_full(
                            buf,
                            video_frame_flags,
                            out_info.format(),
                            out_info.width(),
                            out_info.height(),
                            &offsets,
                            &strides,
                        )
                        .unwrap();

                        if input_buffer_writable {
                            Ok(PrepareOutputBufferSuccess::InputBuffer)
                        } else {
                            Ok(PrepareOutputBufferSuccess::Buffer(buf.to_owned()))
                        }
                    }
                }
            }
            None => {
                let n_planes = out_info.n_planes() as usize;
                offsets[..n_planes].clone_from_slice(&out_info.offset()[..n_planes]);
                strides[..n_planes].clone_from_slice(&out_info.stride()[..n_planes]);

                gst_video::VideoMeta::add_full(
                    buf,
                    gst_video::VideoFrameFlags::empty(),
                    out_info.format(),
                    out_info.width(),
                    out_info.height(),
                    &offsets,
                    &strides,
                )
                .unwrap();

                if input_buffer_writable {
                    Ok(PrepareOutputBufferSuccess::InputBuffer)
                } else {
                    Ok(PrepareOutputBufferSuccess::Buffer(buf.to_owned()))
                }
            }
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RoundedCorners {
    const NAME: &'static str = "GstRoundedCorners";
    type Type = super::RoundedCorners;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for RoundedCorners {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecUInt::builder("border-radius-px")
                .nick("Border radius in pixels")
                .blurb("Draw rounded corners with given border radius")
                .default_value(DEFAULT_BORDER_RADIUS)
                .mutable_playing()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "border-radius-px" => {
                let mut settings = self.settings.lock().unwrap();
                let border_radius = value.get().expect("type checked upstream");
                if settings.border_radius_px != border_radius {
                    settings.changed = true;
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing border radius from {} to {}",
                        settings.border_radius_px,
                        border_radius
                    );
                    settings.border_radius_px = border_radius;
                    self.obj().reconfigure_src();
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "border-radius-px" => {
                let settings = self.settings.lock().unwrap();
                settings.border_radius_px.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RoundedCorners {}

impl ElementImpl for RoundedCorners {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Rounded Corners",
                "Filter/Effect/Converter/Video",
                "Adds rounded corners to video",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst_video::VideoCapsBuilder::new()
                .format(VideoFormat::I420)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_video::VideoCapsBuilder::new()
                .format_list([VideoFormat::I420, VideoFormat::A420])
                .build();
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

impl BaseTransformImpl for RoundedCorners {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let _ = self.state.lock().unwrap().take();

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn transform_caps(
        &self,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let other_caps = if direction == gst::PadDirection::Src {
            let mut caps = caps.clone();

            for s in caps.make_mut().iter_mut() {
                s.set("format", VideoFormat::I420.to_str());
            }

            caps
        } else {
            let mut output_caps = gst::Caps::new_empty();
            {
                let output_caps = output_caps.get_mut().unwrap();
                let border_radius = self.settings.lock().unwrap().border_radius_px;

                for s in caps.iter() {
                    let mut s_output = s.to_owned();
                    if border_radius == 0 {
                        s_output.set(
                            "format",
                            gst::List::new([
                                VideoFormat::I420.to_str(),
                                VideoFormat::A420.to_str(),
                            ]),
                        );
                    } else {
                        s_output.set("format", VideoFormat::A420.to_str());
                    }
                    output_caps.append_structure(s_output);
                }
            }

            output_caps
        };

        gst::debug!(
            CAT,
            imp = self,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        if let Some(filter) = filter {
            Some(filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First))
        } else {
            Some(other_caps)
        }
    }

    fn set_caps(&self, incaps: &gst::Caps, outcaps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let mut settings = self.settings.lock().unwrap();

        let out_info = match gst_video::VideoInfo::from_caps(outcaps) {
            Err(_) => return Err(gst::loggable_error!(CAT, "Failed to parse output caps")),
            Ok(info) => info,
        };

        gst::debug!(
            CAT,
            imp = self,
            "Configured for caps {} to {}",
            incaps,
            outcaps
        );

        if out_info.format() == VideoFormat::I420 {
            self.obj().set_passthrough(true);
            return Ok(());
        } else {
            self.obj().set_passthrough(false);
        }

        // See "A420" planar 4:4:2:0 AYUV section
        // https://gstreamer.freedesktop.org/documentation/additional/design/mediatype-video-raw.html?gi-language=c
        let ru2_height = (out_info.height() + 1) & !1;
        let alpha_mem_size = (out_info.stride()[3] as u32 * ru2_height) as usize;

        *self.state.lock().unwrap() = Some(State {
            alpha_mem: gst::Memory::with_size(alpha_mem_size),
            out_info: Some(out_info),
        });

        settings.changed = true;

        Ok(())
    }

    fn prepare_output_buffer(
        &self,
        inbuf: InputBuffer,
    ) -> Result<PrepareOutputBufferSuccess, gst::FlowError> {
        if self.obj().is_passthrough() {
            return Ok(PrepareOutputBufferSuccess::InputBuffer);
        }

        let mut settings = self.settings.lock().unwrap();
        if settings.changed {
            settings.changed = false;
            gst::debug!(
                CAT,
                imp = self,
                "Caps or border radius changed, generating alpha mask"
            );
            let state_guard = self.state.lock().unwrap();
            let state = state_guard.as_ref().ok_or_else(|| {
                gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no state yet"]);
                gst::FlowError::NotNegotiated
            })?;

            match state.out_info.as_ref().unwrap().format() {
                VideoFormat::I420 => return Ok(PrepareOutputBufferSuccess::InputBuffer),
                VideoFormat::A420 => {
                    drop(state_guard);
                    if self.generate_alpha_mask(settings.border_radius_px).is_err() {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Negotiation,
                            ["Failed to generate alpha mask"]
                        );
                        return Err(gst::FlowError::NotNegotiated);
                    }
                }
                _ => unimplemented!(),
            }
        }

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or_else(|| {
            gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no state yet"]);
            gst::FlowError::NotNegotiated
        })?;

        let out_info = state.out_info.as_ref().unwrap();
        let mem = &state.alpha_mem;
        let alpha_mem = mem.clone();

        match inbuf {
            InputBuffer::Writable(outbuf) => {
                gst::log!(
                    CAT,
                    imp = self,
                    "Received writable input buffer of size: {}",
                    outbuf.size()
                );
                let alpha_plane_offset = outbuf.size();
                outbuf.append_memory(alpha_mem);

                self.add_video_meta(outbuf, out_info, alpha_plane_offset, true)
            }
            InputBuffer::Readable(buf) => {
                gst::log!(
                    CAT,
                    imp = self,
                    "Received readable input buffer of size: {}",
                    buf.size()
                );
                let alpha_plane_offset = buf.size();
                let mut outbuf = buf.copy();
                let mut_outbuf = outbuf.make_mut();
                mut_outbuf.append_memory(alpha_mem);

                self.add_video_meta(mut_outbuf, out_info, alpha_plane_offset, false)
            }
        }
    }

    fn transform_ip(&self, _buf: &mut gst::BufferRef) -> Result<gst::FlowSuccess, gst::FlowError> {
        Ok(gst::FlowSuccess::Ok)
    }

    fn propose_allocation(
        &self,
        decide_query: Option<&gst::query::Allocation>,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        query.add_allocation_meta::<gst_video::VideoMeta>(None);
        self.parent_propose_allocation(decide_query, query)
    }
}
