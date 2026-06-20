//
// Copyright (C) 2021 Bilal Elmoussaoui <bil.elmoussaoui@gmail.com>
// Copyright (C) 2021 Jordan Petridis <jordan@centricular.com>
// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::sink::imp::PaintableSink;
use crate::sink::imp::CAT;
use gst_gl::prelude::*;
use gst_video::subclass::prelude::*;
use gtk::{gdk, glib};
use std::{
    collections::{HashMap, HashSet},
    ops,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VideoInfo {
    VideoInfo(gst_video::VideoInfo),
    #[cfg(all(target_os = "linux", feature = "dmabuf"))]
    DmaDrm(gst_video::VideoInfoDmaDrm),
}

impl From<gst_video::VideoInfo> for VideoInfo {
    fn from(v: gst_video::VideoInfo) -> Self {
        VideoInfo::VideoInfo(v)
    }
}

#[cfg(all(target_os = "linux", feature = "dmabuf"))]
impl From<gst_video::VideoInfoDmaDrm> for VideoInfo {
    fn from(v: gst_video::VideoInfoDmaDrm) -> Self {
        VideoInfo::DmaDrm(v)
    }
}

impl ops::Deref for VideoInfo {
    type Target = gst_video::VideoInfo;

    fn deref(&self) -> &Self::Target {
        match self {
            VideoInfo::VideoInfo(info) => info,
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            VideoInfo::DmaDrm(info) => info,
        }
    }
}

impl VideoInfo {
    #[cfg(all(target_os = "linux", feature = "dmabuf"))]
    fn dma_drm(&self) -> Option<&gst_video::VideoInfoDmaDrm> {
        match self {
            VideoInfo::VideoInfo(..) => None,
            VideoInfo::DmaDrm(info) => Some(info),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum TextureCacheId {
    Memory(usize),
    GL(usize),
    #[cfg(all(target_os = "linux", feature = "dmabuf"))]
    DmaBuf([i32; 4]),
}

#[derive(Debug)]
enum MappedFrame {
    SysMem {
        frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
        orientation: Orientation,
    },
    GL {
        frame: gst_gl::GLVideoFrame<gst_gl::gl_video_frame::Readable>,
        wrapped_context: gst_gl::GLContext,
        orientation: Orientation,
    },
    #[cfg(all(target_os = "linux", feature = "dmabuf"))]
    DmaBuf {
        buffer: gst::Buffer,
        info: gst_video::VideoInfoDmaDrm,
        n_planes: u32,
        fds: [i32; 4],
        offsets: [usize; 4],
        strides: [usize; 4],
        orientation: Orientation,
    },
}

impl MappedFrame {
    fn buffer(&self) -> &gst::BufferRef {
        match self {
            MappedFrame::SysMem { frame, .. } => frame.buffer(),
            MappedFrame::GL { frame, .. } => frame.buffer(),
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf { buffer, .. } => buffer,
        }
    }

    fn width(&self) -> u32 {
        match self {
            MappedFrame::SysMem { frame, .. } => frame.width(),
            MappedFrame::GL { frame, .. } => frame.width(),
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf { info, .. } => info.width(),
        }
    }

    fn height(&self) -> u32 {
        match self {
            MappedFrame::SysMem { frame, .. } => frame.height(),
            MappedFrame::GL { frame, .. } => frame.height(),
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf { info, .. } => info.height(),
        }
    }

    fn format_info(&self) -> gst_video::VideoFormatInfo {
        match self {
            MappedFrame::SysMem { frame, .. } => frame.format_info(),
            MappedFrame::GL { frame, .. } => frame.format_info(),
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf { info, .. } => info.format_info(),
        }
    }

    fn orientation(&self) -> Orientation {
        match self {
            MappedFrame::SysMem { orientation, .. } => *orientation,
            MappedFrame::GL { orientation, .. } => *orientation,
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf { orientation, .. } => *orientation,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Frame {
    frame: MappedFrame,
    overlays: Vec<Overlay>,
}

#[derive(Debug, Default, glib::Enum, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
#[enum_type(name = "GstGtk4PaintableSinkOrientation")]
pub enum Orientation {
    #[default]
    Auto,
    Rotate0,
    Rotate90,
    Rotate180,
    Rotate270,
    FlipRotate0,
    FlipRotate90,
    FlipRotate180,
    FlipRotate270,
}

impl Orientation {
    pub fn from_tags(tags: &gst::TagListRef) -> Option<Orientation> {
        let orientation = tags
            .generic("image-orientation")
            .and_then(|v| v.get::<String>().ok())?;

        Some(match orientation.as_str() {
            "rotate-0" => Orientation::Rotate0,
            "rotate-90" => Orientation::Rotate90,
            "rotate-180" => Orientation::Rotate180,
            "rotate-270" => Orientation::Rotate270,
            "flip-rotate-0" => Orientation::FlipRotate0,
            "flip-rotate-90" => Orientation::FlipRotate90,
            "flip-rotate-180" => Orientation::FlipRotate180,
            "flip-rotate-270" => Orientation::FlipRotate270,
            _ => return None,
        })
    }

    pub fn is_flip_width_height(self) -> bool {
        matches!(
            self,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::FlipRotate90
                | Orientation::FlipRotate270
        )
    }
}

#[derive(Debug)]
struct Overlay {
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    global_alpha: f32,
}

#[derive(Debug)]
pub(crate) struct Texture {
    pub texture: gdk::Texture,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub global_alpha: f32,
    pub has_alpha: bool,
    pub orientation: Orientation,
}

struct FrameWrapper(gst_video::VideoFrame<gst_video::video_frame::Readable>);
impl AsRef<[u8]> for FrameWrapper {
    fn as_ref(&self) -> &[u8] {
        let raw = self.0.as_ptr();
        let data = unsafe {
            std::slice::from_raw_parts((*raw).map[0].data as *const u8, (*raw).map[0].size)
        };
        data
    }
}

fn video_format_to_memory_format(f: gst_video::VideoFormat) -> gdk::MemoryFormat {
    match f {
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Yuy2 => gdk::MemoryFormat::G8b8g8r8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Yvyu => gdk::MemoryFormat::G8r8g8b8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Uyvy => gdk::MemoryFormat::B8g8r8g8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Vyuy => gdk::MemoryFormat::R8g8b8g8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Nv12 => gdk::MemoryFormat::G8B8r8420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Nv21 => gdk::MemoryFormat::G8R8b8420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Nv16 => gdk::MemoryFormat::G8B8r8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Nv61 => gdk::MemoryFormat::G8R8b8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Nv24 => gdk::MemoryFormat::G8B8r8444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P01010le => gdk::MemoryFormat::G10x6B10x6r10x6420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P01010be => gdk::MemoryFormat::G10x6B10x6r10x6420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P012Le => gdk::MemoryFormat::G12x4B12x4r12x4420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P012Be => gdk::MemoryFormat::G12x4B12x4r12x4420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P016Le => gdk::MemoryFormat::G16B16r16420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::P016Be => gdk::MemoryFormat::G16B16r16420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Yuv9 => gdk::MemoryFormat::G8B8R8410,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Yvu9 => gdk::MemoryFormat::G8R8B8410,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y41b => gdk::MemoryFormat::G8B8R8411,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I420 => gdk::MemoryFormat::G8B8R8420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Yv12 => gdk::MemoryFormat::G8R8B8420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y42b => gdk::MemoryFormat::G8B8R8422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y444 => gdk::MemoryFormat::G8B8R8444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42010le => gdk::MemoryFormat::X6g10X6b10X6r10420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42010be => gdk::MemoryFormat::X6g10X6b10X6r10420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42210le => gdk::MemoryFormat::X6g10X6b10X6r10422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42210be => gdk::MemoryFormat::X6g10X6b10X6r10422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44410le => gdk::MemoryFormat::X6g10X6b10X6r10444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44410be => gdk::MemoryFormat::X6g10X6b10X6r10444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42012le => gdk::MemoryFormat::X4g12X4b12X4r12420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42012be => gdk::MemoryFormat::X4g12X4b12X4r12420,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42212le => gdk::MemoryFormat::X4g12X4b12X4r12422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::I42212be => gdk::MemoryFormat::X4g12X4b12X4r12422,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44412le => gdk::MemoryFormat::X4g12X4b12X4r12444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44412be => gdk::MemoryFormat::X4g12X4b12X4r12444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44416le => gdk::MemoryFormat::G16B16R16444,
        #[cfg(feature = "gtk_v4_20")]
        gst_video::VideoFormat::Y44416be => gdk::MemoryFormat::G16B16R16444,
        #[cfg(feature = "gtk_v4_14")]
        gst_video::VideoFormat::Bgrx => gdk::MemoryFormat::B8g8r8x8,
        #[cfg(feature = "gtk_v4_14")]
        gst_video::VideoFormat::Xrgb => gdk::MemoryFormat::X8r8g8b8,
        #[cfg(feature = "gtk_v4_14")]
        gst_video::VideoFormat::Rgbx => gdk::MemoryFormat::R8g8b8x8,
        #[cfg(feature = "gtk_v4_14")]
        gst_video::VideoFormat::Xbgr => gdk::MemoryFormat::X8b8g8r8,
        gst_video::VideoFormat::Bgra => gdk::MemoryFormat::B8g8r8a8,
        gst_video::VideoFormat::Argb => gdk::MemoryFormat::A8r8g8b8,
        gst_video::VideoFormat::Rgba => gdk::MemoryFormat::R8g8b8a8,
        gst_video::VideoFormat::Abgr => gdk::MemoryFormat::A8b8g8r8,
        gst_video::VideoFormat::Rgb => gdk::MemoryFormat::R8g8b8,
        gst_video::VideoFormat::Bgr => gdk::MemoryFormat::B8g8r8,
        _ => unreachable!(),
    }
}

#[cfg(feature = "gtk_v4_20")]
fn videoinfo_to_color_state(info: &gst_video::VideoInfo) -> Option<gdk::ColorState> {
    let colorimetry = info.colorimetry();
    let color_params = gdk::CicpParams::new();

    let range = match (colorimetry.range(), info.is_rgb()) {
        (gst_video::VideoColorRange::Range0_255, ..) => gdk::CicpRange::Full,
        (gst_video::VideoColorRange::Range16_235, ..) => gdk::CicpRange::Narrow,
        (.., true) => gdk::CicpRange::Full,
        (.., false) => gdk::CicpRange::Narrow,
    };
    color_params.set_range(range);

    let matrix = match (colorimetry.matrix(), info.is_rgb()) {
        (gst_video::VideoColorMatrix::Unknown, true) => gst_video::VideoColorMatrix::Rgb.to_iso(),
        (gst_video::VideoColorMatrix::Unknown, false) => {
            gst_video::VideoColorMatrix::Bt709.to_iso()
        }
        _ => colorimetry.matrix().to_iso(),
    };
    color_params.set_matrix_coefficients(matrix);

    let tf = match (colorimetry.transfer(), info.is_rgb()) {
        (gst_video::VideoTransferFunction::Unknown, true) => {
            gst_video::VideoTransferFunction::Srgb.to_iso()
        }
        (gst_video::VideoTransferFunction::Unknown, false) => {
            gst_video::VideoTransferFunction::Bt709.to_iso()
        }
        _ => colorimetry.transfer().to_iso(),
    };
    color_params.set_transfer_function(tf);

    let primaries = match (colorimetry.primaries(), info.is_rgb()) {
        (gst_video::VideoColorPrimaries::Unknown, true) => {
            gst_video::VideoColorPrimaries::Bt709.to_iso()
        }
        (gst_video::VideoColorPrimaries::Unknown, false) => {
            gst_video::VideoColorPrimaries::Bt709.to_iso()
        }
        _ => colorimetry.primaries().to_iso(),
    };
    color_params.set_color_primaries(primaries);

    match color_params.build_color_state() {
        Ok(color_state) => Some(color_state),
        Err(error) => {
            println!("Could not build color state: {}", error);
            None
        }
    }
}

fn cropped_size_from_info(info: &gst_video::VideoInfo) -> (u32, u32) {
    let w_sub = 1 << info.format_info().w_sub()[(info.n_components() as usize) - 1];
    let h_sub = 1 << info.format_info().h_sub()[(info.n_components() as usize) - 1];
    let cropped_width = info.width() - (info.width() % w_sub);
    let cropped_height = info.height() - (info.width() % h_sub);
    (cropped_width, cropped_height)
}

fn video_frame_to_memory_texture(
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
    cached_textures: &mut HashMap<TextureCacheId, gdk::Texture>,
    used_textures: &mut HashSet<TextureCacheId>,
) -> (gdk::Texture, f64) {
    let ptr = frame.plane_data(0).unwrap().as_ptr() as usize;

    let pixel_aspect_ratio =
        (frame.info().par().numer() as f64) / (frame.info().par().denom() as f64);

    if let Some(texture) = cached_textures.get(&TextureCacheId::Memory(ptr)) {
        used_textures.insert(TextureCacheId::Memory(ptr));
        return (texture.clone(), pixel_aspect_ratio);
    }

    let format = video_format_to_memory_format(frame.format());
    let (width, height) = cropped_size_from_info(frame.info());

    let texture = {
        #[cfg(feature = "gtk_v4_20")]
        {
            let mut builder = gdk::MemoryTextureBuilder::new()
                .set_width(width as i32)
                .set_height(height as i32)
                .set_format(format);

            if let Some(color_state) = videoinfo_to_color_state(frame.info()) {
                builder = builder.set_color_state(&color_state);
            }

            for plane in 0..(frame.n_planes() as usize) {
                builder = builder
                    .set_offset(plane as u32, frame.plane_offset()[plane])
                    .set_stride_for_plane(plane as u32, frame.plane_stride()[plane] as usize);
            }

            builder = builder.set_bytes(Some(&glib::Bytes::from_owned(FrameWrapper(frame))));

            builder.build()
        }
        #[cfg(not(feature = "gtk_v4_20"))]
        {
            let stride = frame.plane_stride()[0] as usize;

            gdk::MemoryTexture::new(
                width as i32,
                height as i32,
                format,
                &glib::Bytes::from_owned(FrameWrapper(frame)),
                stride,
            )
        }
        .upcast::<gdk::Texture>()
    };

    cached_textures.insert(TextureCacheId::Memory(ptr), texture.clone());
    used_textures.insert(TextureCacheId::Memory(ptr));

    (texture, pixel_aspect_ratio)
}

fn video_frame_to_gl_texture(
    frame: gst_gl::GLVideoFrame<gst_gl::gl_video_frame::Readable>,
    cached_textures: &mut HashMap<TextureCacheId, gdk::Texture>,
    used_textures: &mut HashSet<TextureCacheId>,
    gdk_context: &gdk::GLContext,
    #[allow(unused)] wrapped_context: &gst_gl::GLContext,
) -> (gdk::Texture, f64) {
    let texture_id = frame.texture_id(0).expect("Invalid texture id") as usize;

    let pixel_aspect_ratio =
        (frame.info().par().numer() as f64) / (frame.info().par().denom() as f64);

    if let Some(texture) = cached_textures.get(&TextureCacheId::GL(texture_id)) {
        used_textures.insert(TextureCacheId::GL(texture_id));
        return (texture.clone(), pixel_aspect_ratio);
    }

    let (width, height) = cropped_size_from_info(frame.info());

    let sync_meta = frame.buffer().meta::<gst_gl::GLSyncMeta>().unwrap();

    let texture = unsafe {
        #[cfg(feature = "gtk_v4_12")]
        {
            let format = {
                let format = video_format_to_memory_format(frame.format());
                #[cfg(feature = "gtk_v4_14")]
                {
                    use gtk::prelude::*;

                    if gdk_context.api() != gdk::GLAPI::GLES || gdk_context.version().0 < 3 {
                        // Map alpha formats to the pre-multiplied versions because we pre-multiply
                        // ourselves if not GLES3 with the new GL renderer is used as the GTK GL
                        // backend does not natively support non-premultiplied formats.
                        match format {
                            gdk::MemoryFormat::B8g8r8a8 => gdk::MemoryFormat::B8g8r8a8Premultiplied,
                            gdk::MemoryFormat::A8r8g8b8 => gdk::MemoryFormat::A8r8g8b8Premultiplied,
                            gdk::MemoryFormat::R8g8b8a8 => gdk::MemoryFormat::R8g8b8a8Premultiplied,
                            gdk::MemoryFormat::A8b8g8r8 => gdk::MemoryFormat::A8r8g8b8Premultiplied,
                            gdk::MemoryFormat::R8g8b8 | gdk::MemoryFormat::B8g8r8 => format,
                            gdk::MemoryFormat::B8g8r8x8
                            | gdk::MemoryFormat::X8r8g8b8
                            | gdk::MemoryFormat::R8g8b8x8
                            | gdk::MemoryFormat::X8b8g8r8 => format,
                            _ => unreachable!(),
                        }
                    } else {
                        format
                    }
                }

                #[cfg(not(feature = "gtk_v4_14"))]
                {
                    // Map alpha formats to the pre-multiplied versions because we pre-multiply
                    // ourselves in pre-4.14 versions as the GTK GL backend does not natively
                    // support non-premultiplied formats
                    match format {
                        gdk::MemoryFormat::B8g8r8a8 => gdk::MemoryFormat::B8g8r8a8Premultiplied,
                        gdk::MemoryFormat::A8r8g8b8 => gdk::MemoryFormat::A8r8g8b8Premultiplied,
                        gdk::MemoryFormat::R8g8b8a8 => gdk::MemoryFormat::R8g8b8a8Premultiplied,
                        gdk::MemoryFormat::A8b8g8r8 => gdk::MemoryFormat::A8r8g8b8Premultiplied,
                        gdk::MemoryFormat::R8g8b8 | gdk::MemoryFormat::B8g8r8 => format,
                        _ => unreachable!(),
                    }
                }
            };
            let sync_point = (*sync_meta.as_ptr()).data;

            let builder = {
                #[cfg(feature = "gtk_v4_20")]
                {
                    let mut mut_builder = gdk::GLTextureBuilder::new()
                        .set_context(Some(gdk_context))
                        .set_id(texture_id as u32)
                        .set_width(width as i32)
                        .set_height(height as i32)
                        .set_format(format)
                        .set_sync(Some(sync_point));

                    if let Some(color_state) = videoinfo_to_color_state(frame.info()) {
                        mut_builder = mut_builder.set_color_state(&color_state);
                    }
                    mut_builder
                }
                #[cfg(not(feature = "gtk_v4_20"))]
                {
                    gdk::GLTextureBuilder::new()
                        .set_context(Some(gdk_context))
                        .set_id(texture_id as u32)
                        .set_width(width as i32)
                        .set_height(height as i32)
                        .set_format(format)
                        .set_sync(Some(sync_point))
                }
            };

            builder.build_with_release_func(move || {
                // Unmap and drop the GStreamer GL texture once GTK is done with it and not earlier
                drop(frame);
            })
        }
        #[cfg(not(feature = "gtk_v4_12"))]
        {
            sync_meta.wait(wrapped_context);

            gdk::GLTexture::with_release_func(
                gdk_context,
                texture_id as u32,
                width as i32,
                height as i32,
                move || {
                    // Unmap and drop the GStreamer GL texture once GTK is done with it and not earlier
                    drop(frame);
                },
            )
        }
        .upcast::<gdk::Texture>()
    };

    cached_textures.insert(TextureCacheId::GL(texture_id), texture.clone());
    used_textures.insert(TextureCacheId::GL(texture_id));

    (texture, pixel_aspect_ratio)
}

#[cfg(all(target_os = "linux", feature = "dmabuf"))]
#[allow(clippy::too_many_arguments)]
fn video_frame_to_dmabuf_texture(
    buffer: gst::Buffer,
    cached_textures: &mut HashMap<TextureCacheId, gdk::Texture>,
    used_textures: &mut HashSet<TextureCacheId>,
    info: &gst_video::VideoInfoDmaDrm,
    n_planes: u32,
    fds: &[i32; 4],
    offsets: &[usize; 4],
    strides: &[usize; 4],
) -> Result<(gdk::Texture, f64), glib::Error> {
    let pixel_aspect_ratio = (info.par().numer() as f64) / (info.par().denom() as f64);

    if let Some(texture) = cached_textures.get(&TextureCacheId::DmaBuf(*fds)) {
        used_textures.insert(TextureCacheId::DmaBuf(*fds));
        return Ok((texture.clone(), pixel_aspect_ratio));
    }

    let (width, height) = cropped_size_from_info(&info.to_video_info().ok().unwrap());
    let mut builder = gdk::DmabufTextureBuilder::new()
        .set_display(&gdk::Display::default().unwrap())
        .set_fourcc(info.fourcc())
        .set_modifier(info.modifier())
        .set_width(width)
        .set_height(height)
        .set_n_planes(n_planes);

    #[cfg(feature = "gtk_v4_20")]
    {
        // Convert DRM fourcc to Gst VideoFormat if possible so we know whether the format is RGB or
        // YCbCr. This is needed for cases where the colorimetry is not fully set. The fallback uses
        // the DMA_DRM format which we assume is YCbCr.
        let color_state = match info.to_video_info() {
            Ok(v) => videoinfo_to_color_state(&v),
            Err(_) => videoinfo_to_color_state(info),
        };
        if color_state.is_some() {
            builder = builder.set_color_state(color_state.as_ref());
        }
    }

    for plane in 0..(n_planes as usize) {
        unsafe {
            builder = builder.set_fd(plane as u32, fds[plane]);
        }
        builder = builder
            .set_offset(plane as u32, offsets[plane] as u32)
            .set_stride(plane as u32, strides[plane] as u32);
    }

    let texture = unsafe {
        builder.build_with_release_func(move || {
            drop(buffer);
        })?
    };

    cached_textures.insert(TextureCacheId::DmaBuf(*fds), texture.clone());
    used_textures.insert(TextureCacheId::DmaBuf(*fds));

    Ok((texture, pixel_aspect_ratio))
}

impl Frame {
    pub(crate) fn into_textures(
        self,
        #[allow(unused_variables)] gdk_context: Option<&gdk::GLContext>,
        cached_textures: &mut HashMap<TextureCacheId, gdk::Texture>,
    ) -> Result<Vec<Texture>, glib::Error> {
        let mut textures = Vec::with_capacity(1 + self.overlays.len());
        let mut used_textures = HashSet::with_capacity(1 + self.overlays.len());

        let width = self.frame.width();
        let height = self.frame.height();
        let has_alpha = self.frame.format_info().has_alpha();
        let orientation = self.frame.orientation();
        let (texture, pixel_aspect_ratio) = match self.frame {
            MappedFrame::SysMem { frame, .. } => {
                video_frame_to_memory_texture(frame, cached_textures, &mut used_textures)
            }
            MappedFrame::GL {
                frame,
                wrapped_context,
                ..
            } => {
                let Some(gdk_context) = gdk_context else {
                    // This will fail badly if the video frame was actually mapped as GL texture
                    // but this case can't really happen as we only do that if we actually have a
                    // GDK GL context.
                    unreachable!();
                };
                video_frame_to_gl_texture(
                    frame,
                    cached_textures,
                    &mut used_textures,
                    gdk_context,
                    &wrapped_context,
                )
            }
            #[cfg(all(target_os = "linux", feature = "dmabuf"))]
            MappedFrame::DmaBuf {
                buffer,
                info,
                n_planes,
                fds,
                offsets,
                strides,
                ..
            } => video_frame_to_dmabuf_texture(
                buffer,
                cached_textures,
                &mut used_textures,
                &info,
                n_planes,
                &fds,
                &offsets,
                &strides,
            )?,
        };

        textures.push(Texture {
            texture,
            x: 0.0,
            y: 0.0,
            width: width as f32 * pixel_aspect_ratio as f32,
            height: height as f32,
            global_alpha: 1.0,
            has_alpha,
            orientation,
        });

        for overlay in self.overlays {
            let has_alpha = overlay.frame.format_info().has_alpha();
            let (texture, _pixel_aspect_ratio) =
                video_frame_to_memory_texture(overlay.frame, cached_textures, &mut used_textures);

            textures.push(Texture {
                texture,
                x: overlay.x as f32,
                y: overlay.y as f32,
                width: overlay.width as f32,
                height: overlay.height as f32,
                global_alpha: overlay.global_alpha,
                has_alpha,
                orientation: Orientation::Rotate0,
            });
        }

        // Remove textures that were not used this time
        cached_textures.retain(|id, _| used_textures.contains(id));

        Ok(textures)
    }
}

impl Frame {
    pub(crate) fn new(
        dbg_obj: &PaintableSink,
        buffer: &gst::Buffer,
        info: &VideoInfo,
        orientation: Orientation,
        wrapped_context: Option<&gst_gl::GLContext>,
    ) -> Result<Self, gst::FlowError> {
        // Empty buffers get filtered out in show_frame
        debug_assert!(buffer.n_memory() > 0);

        #[allow(unused_mut)]
        let mut frame = None;

        // Check we received a buffer with dmabuf memory and if so do some checks before
        // passing it onwards
        #[cfg(all(target_os = "linux", feature = "dmabuf"))]
        if frame.is_none()
            && buffer
                .peek_memory(0)
                .is_memory_type::<gst_allocators::DmaBufMemory>()
        {
            let drm_info;
            let drm_info_ref = match info.dma_drm() {
                Some(drm) => Some(drm),
                None => match gst_video::VideoInfoDmaDrm::from_video_info(info, 0) {
                    Ok(i) => {
                        drm_info = Some(i);
                        drm_info.as_ref()
                    }
                    Err(_) => None,
                },
            };

            if let Some((vmeta, info)) =
                Option::zip(buffer.meta::<gst_video::VideoMeta>(), drm_info_ref)
            {
                let mut fds = [-1i32; 4];
                let mut offsets = [0; 4];
                let mut strides = [0; 4];
                let n_planes = vmeta.n_planes() as usize;

                let vmeta_offsets = vmeta.offset();
                let vmeta_strides = vmeta.stride();

                for plane in 0..n_planes {
                    let Some((range, skip)) =
                        buffer.find_memory(vmeta_offsets[plane]..(vmeta_offsets[plane] + 1))
                    else {
                        break;
                    };

                    let mem = buffer.peek_memory(range.start);
                    let Some(mem) = mem.downcast_memory_ref::<gst_allocators::DmaBufMemory>()
                    else {
                        break;
                    };

                    let fd = mem.fd();
                    fds[plane] = fd;
                    offsets[plane] = mem.offset() + skip;
                    strides[plane] = vmeta_strides[plane] as usize;
                }

                // All fds valid?
                if fds[0..n_planes].iter().all(|fd| *fd != -1) {
                    frame = Some(MappedFrame::DmaBuf {
                        buffer: buffer.clone(),
                        info: info.clone(),
                        n_planes: n_planes as u32,
                        fds,
                        offsets,
                        strides,
                        orientation,
                    });
                }
                if frame.is_some() {
                    gst::debug!(
                        CAT,
                        imp = dbg_obj,
                        "Created frame using DmabufTextureBuilder"
                    );
                }
            }
        }

        if frame.is_none() {
            // Check we received a buffer with GL memory and if the context of that memory
            // can share with the wrapped context around the GDK GL context.
            //
            // If not it has to be uploaded to the GPU.
            let memory_ctx = buffer
                .peek_memory(0)
                .downcast_memory_ref::<gst_gl::GLBaseMemory>()
                .and_then(|m| {
                    let ctx = m.context();
                    if wrapped_context.is_some_and(|wrapped_context| wrapped_context.can_share(ctx))
                    {
                        Some(ctx)
                    } else {
                        None
                    }
                });

            if let Some(memory_ctx) = memory_ctx {
                // If there is no GLSyncMeta yet then we need to add one here now, which requires
                // obtaining a writable buffer.
                let mapped_frame = if buffer.meta::<gst_gl::GLSyncMeta>().is_some() {
                    gst_gl::GLVideoFrame::from_buffer_readable(buffer.clone(), info)
                        .map_err(|_| gst::FlowError::Error)?
                } else {
                    let mut buffer = buffer.clone();
                    {
                        let buffer = buffer.make_mut();
                        gst_gl::GLSyncMeta::add(buffer, memory_ctx);
                    }
                    gst_gl::GLVideoFrame::from_buffer_readable(buffer, info)
                        .map_err(|_| gst::FlowError::Error)?
                };

                // Now that it's guaranteed that there is a sync meta and the frame is mapped, set
                // a sync point so we can ensure that the texture is ready later when making use of
                // it as gdk::GLTexture.
                let meta = mapped_frame.buffer().meta::<gst_gl::GLSyncMeta>().unwrap();
                meta.set_sync_point(memory_ctx);

                frame = Some(MappedFrame::GL {
                    frame: mapped_frame,
                    wrapped_context: wrapped_context.unwrap().clone(),
                    orientation,
                });
                if frame.is_some() {
                    gst::debug!(CAT, imp = dbg_obj, "Created frame using GLTextureBuilder");
                }
            }
        }

        let mut frame = Self {
            frame: match frame {
                Some(frame) => frame,
                None => {
                    let readable_frame = match buffer.n_memory() {
                        1 => buffer.clone(),
                        _ => {
                            gst::debug!(CAT, imp = dbg_obj, "Buffer is not contiguous, copying");

                            let mut copy_buffer = gst::Buffer::with_size(info.size()).unwrap();

                            {
                                let copy_buffer = copy_buffer.get_mut().unwrap();

                                buffer
                                    .copy_into(copy_buffer, gst::BUFFER_COPY_METADATA, 0..0)
                                    .unwrap();

                                let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(
                                    buffer, info,
                                )
                                .unwrap();
                                let mut copy_frame =
                                    gst_video::VideoFrameRef::from_buffer_ref_writable(
                                        copy_buffer,
                                        info,
                                    )
                                    .unwrap();

                                frame.copy(&mut copy_frame).unwrap();
                            }
                            copy_buffer
                        }
                    };

                    let frame = MappedFrame::SysMem {
                        frame: gst_video::VideoFrame::from_buffer_readable(readable_frame, info)
                            .map_err(|_| gst::FlowError::Error)?,
                        orientation,
                    };
                    gst::debug!(
                        CAT,
                        imp = dbg_obj,
                        "Created frame using MemoryTextureBuilder"
                    );
                    frame
                }
            },
            overlays: vec![],
        };

        frame.overlays = frame
            .frame
            .buffer()
            .iter_meta::<gst_video::VideoOverlayCompositionMeta>()
            .flat_map(|meta| {
                meta.overlay()
                    .iter()
                    .filter_map(|rect| {
                        let buffer = rect
                            .pixels_unscaled_argb(gst_video::VideoOverlayFormatFlags::GLOBAL_ALPHA);
                        let (x, y, width, height) = rect.render_rectangle();
                        let global_alpha = rect.global_alpha();

                        let vmeta = buffer.meta::<gst_video::VideoMeta>().unwrap();
                        let info = gst_video::VideoInfo::builder(
                            vmeta.format(),
                            vmeta.width(),
                            vmeta.height(),
                        )
                        .build()
                        .unwrap();
                        let frame =
                            gst_video::VideoFrame::from_buffer_readable(buffer, &info).ok()?;

                        Some(Overlay {
                            frame,
                            x,
                            y,
                            width,
                            height,
                            global_alpha,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        Ok(frame)
    }
}
