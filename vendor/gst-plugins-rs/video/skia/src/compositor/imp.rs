// SPDX-License-Identifier: MPL-2.0
use gst::glib::Properties;
use gst_base::subclass::prelude::*;
use gst_video::{prelude::*, subclass::prelude::*};
use std::{
    ops::ControlFlow,
    sync::{LazyLock, Mutex},
};

use super::*;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "skiacompositor",
        gst::DebugColorFlags::FG_BLUE,
        Some("Skia compositor"),
    )
});

mod video_format {
    static MAPPINGS: &[(skia::ColorType, gst_video::VideoFormat)] = &[
        (skia::ColorType::RGBA8888, gst_video::VideoFormat::Rgba),
        (skia::ColorType::BGRA8888, gst_video::VideoFormat::Bgra),
        (skia::ColorType::RGB888x, gst_video::VideoFormat::Rgbx),
        (skia::ColorType::RGB565, gst_video::VideoFormat::Rgb16),
        (skia::ColorType::Gray8, gst_video::VideoFormat::Gray8),
    ];

    pub fn gst_to_skia(video_format: gst_video::VideoFormat) -> Option<skia::ColorType> {
        MAPPINGS
            .iter()
            .find_map(|&(ct, vf)| (vf == video_format).then_some(ct))
    }

    pub fn gst_formats() -> Vec<gst_video::VideoFormat> {
        MAPPINGS.iter().map(|&(_, vf)| vf).collect()
    }
}

#[derive(glib::Enum, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
#[enum_type(name = "GstSkiaCompositorBackground")]
#[repr(u32)]
pub enum Background {
    #[default]
    Checker = 0,
    Black = 1,
    White = 2,
    Transparent = 3,
}

#[derive(Default, Properties, Debug)]
#[properties(wrapper_type = super::SkiaCompositor)]
pub struct SkiaCompositor {
    #[property(name = "background", get, set, builder(Background::Checker))]
    background: Mutex<Background>,
}

impl SkiaCompositor {
    fn should_draw_background(&self, token: &gst_video::subclass::AggregateFramesToken) -> bool {
        let obj = self.obj();
        let info = match obj.video_info() {
            Some(info) => info,
            None => return false,
        };

        let bg_rect = gst_video::VideoRectangle {
            x: 0,
            y: 0,
            w: info.width() as i32,
            h: info.height() as i32,
        };

        for pad in obj.sink_pads() {
            let pad = pad.downcast_ref::<SkiaCompositorPad>().unwrap();
            if pad.is_inactive() || pad.prepared_frame(token).is_none() {
                continue;
            }

            if self.pad_obscures_rectangle(pad, &bg_rect, token) {
                return false;
            }
        }

        true
    }

    fn pad_obscures_rectangle(
        &self,
        pad: &SkiaCompositorPad,
        rect: &gst_video::VideoRectangle,
        token: &gst_video::subclass::AggregateFramesToken,
    ) -> bool {
        let mut fill_border = true;
        let mut border_argb = 0xff000000;

        if !pad.has_current_buffer(token) {
            return false;
        }

        if pad.alpha() != 1.0
            || pad
                .video_info()
                .expect("Pad has a buffer, it must have VideoInfo data")
                .has_alpha()
        {
            gst::trace!(
                CAT,
                imp = self,
                "Pad {} has alpha or alpha channel",
                pad.name()
            );
            return false;
        }

        if let Some(config) = pad.property::<Option<gst::Structure>>("converter-config") {
            border_argb = config.get::<u32>("border-argb").unwrap_or(border_argb);
            fill_border = config.get::<bool>("fill-border").unwrap_or(fill_border);
        }

        if !fill_border || (border_argb & 0xff000000) != 0xff000000 {
            gst::trace!(CAT, imp = self, "Pad {} has border", pad.name());
            return false;
        }

        let mut pad_rect = gst_video::VideoRectangle {
            x: pad.xpos() as i32,
            y: pad.ypos() as i32,
            w: 0,
            h: 0,
        };

        let out_info = self.obj().video_info().unwrap();
        let (output_width, output_height) = self.mixer_pad_get_output_size(pad, out_info.par());
        pad_rect.w = output_width as i32;
        pad_rect.h = output_height as i32;

        if !self.is_rectangle_contained(rect, &pad_rect) {
            return false;
        }

        true
    }

    fn is_rectangle_contained(
        &self,
        rect1: &gst_video::VideoRectangle,
        rect2: &gst_video::VideoRectangle,
    ) -> bool {
        rect2.x <= rect1.x
            && rect2.y <= rect1.y
            && rect2.x + rect2.w >= rect1.x + rect1.w
            && rect2.y + rect2.h >= rect1.y + rect1.h
    }

    fn mixer_pad_get_output_size(
        &self,
        pad: &SkiaCompositorPad,
        out_par: gst::Fraction,
    ) -> (f32, f32) {
        let mut pad_width;
        let mut pad_height;

        let obj = self.obj();
        let video_info = obj.video_info().unwrap();
        pad_width = if pad.width() <= 0. {
            video_info.width() as f32
        } else {
            pad.width()
        };
        pad_height = if pad.height() <= 0. {
            video_info.height() as f32
        } else {
            pad.height()
        };

        if pad_width == 0. || pad_height == 0. {
            return (0., 0.);
        }

        let dar = match gst_video::calculate_display_ratio(
            pad_width as u32,
            pad_height as u32,
            video_info.par(),
            out_par,
        ) {
            None => return (0., 0.),
            Some(dar) => dar,
        };

        if pad_height % dar.numer() as f32 == 0. {
            pad_width = pad_height * dar.numer() as f32 / dar.denom() as f32;
        } else if pad_width % dar.denom() as f32 == 0. {
            pad_height = pad_width * dar.denom() as f32 / dar.numer() as f32;
        } else {
            pad_width = pad_height * dar.numer() as f32 / dar.denom() as f32;
        }

        (pad_width, pad_height)
    }

    fn draw_background(&self, canvas: &skia::Canvas, info: &gst_video::VideoInfo) {
        let mut paint = skia::Paint::default();
        match *self.background.lock().unwrap() {
            Background::Black => paint.set_color(skia::Color::BLACK),
            Background::White => paint.set_color(skia::Color::WHITE),
            Background::Transparent => paint.set_color(skia::Color::TRANSPARENT),
            Background::Checker => {
                let square_size: f32 = 10.;
                let size = canvas.base_layer_size();

                for i in 0..(size.width / square_size as i32) {
                    for j in 0..(size.height / square_size as i32) {
                        let is_even = (i + j) % 2 == 0;
                        paint.set_color(if is_even {
                            skia::Color::DARK_GRAY
                        } else {
                            skia::Color::GRAY
                        });

                        let x = i as f32 * square_size;
                        let y = j as f32 * square_size;

                        let rect = skia::Rect::from_xywh(x, y, square_size, square_size);
                        canvas.draw_rect(rect, &paint);
                    }
                }

                return;
            }
        };
        paint.set_style(skia::paint::Style::Fill);
        paint.set_anti_alias(true);

        canvas.draw_rect(
            skia::Rect::from_xywh(0., 0., info.width() as f32, info.height() as f32),
            &paint,
        );
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SkiaCompositor {
    const NAME: &'static str = "GstSkiaCompositor";
    type Type = super::SkiaCompositor;
    type ParentType = gst_video::VideoAggregator;
    type Interfaces = (gst::ChildProxy,);
}

#[glib::derived_properties]
impl ObjectImpl for SkiaCompositor {}
impl GstObjectImpl for SkiaCompositor {}

impl ElementImpl for SkiaCompositor {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: std::sync::OnceLock<gst::subclass::ElementMetadata> =
            std::sync::OnceLock::new();

        Some(ELEMENT_METADATA.get_or_init(|| {
            gst::subclass::ElementMetadata::new(
                "Skia Compositor",
                "Compositor/Video",
                "Skia based compositor",
                "Thibault Saunier <tsaunier@igalia.com>, Sebastian Dröge <sebastian@centricular.com>",
            )
        }))
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: std::sync::OnceLock<Vec<gst::PadTemplate>> =
            std::sync::OnceLock::new();

        PAD_TEMPLATES.get_or_init(|| {
            vec![
                gst::PadTemplate::new(
                    "src",
                    gst::PadDirection::Src,
                    gst::PadPresence::Always,
                    // Support formats supported by Skia and GStreamer on the src side
                    &gst_video::VideoCapsBuilder::new()
                        .format_list(video_format::gst_formats())
                        .build(),
                )
                .unwrap(),
                gst::PadTemplate::with_gtype(
                    "sink_%u",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Request,
                    // Support all formats as inputs will be converted to the output format
                    // automatically by the VideoAggregatorConvertPad base class
                    &gst_video::VideoCapsBuilder::new().build(),
                    SkiaCompositorPad::static_type(),
                )
                .unwrap(),
            ]
        })
    }

    // Notify via the child proxy interface whenever a new pad is added or removed.
    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let element = self.obj();
        let pad = self.parent_request_new_pad(templ, name, caps)?;
        element.child_added(&pad, &pad.name());
        Some(pad)
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let element = self.obj();
        element.child_removed(pad, &pad.name());
        self.parent_release_pad(pad);
    }
}

// Implementation of gst_base::Aggregator virtual methods.
impl AggregatorImpl for SkiaCompositor {
    fn sink_query(
        &self,
        aggregator_pad: &gst_base::AggregatorPad,
        query: &mut gst::QueryRef,
    ) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let caps = aggregator_pad.pad_template_caps();
                let filter = q.filter();

                let caps = if let Some(filter) = filter {
                    filter.intersect_with_mode(&caps, gst::CapsIntersectMode::First)
                } else {
                    caps
                };

                q.set_result(&caps);

                true
            }
            QueryViewMut::AcceptCaps(q) => {
                let caps = q.caps();
                let template_caps = aggregator_pad.pad_template_caps();
                let res = caps.is_subset(&template_caps);
                q.set_result(res);

                true
            }
            _ => self.parent_sink_query(aggregator_pad, query),
        }
    }
}

impl VideoAggregatorImpl for SkiaCompositor {
    fn aggregate_frames(
        &self,
        token: &gst_video::subclass::AggregateFramesToken,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let obj = self.obj();

        // Map the output frame writable.
        let out_info = obj.video_info().unwrap();

        let mut mapped_mem = outbuf.map_writable().map_err(|_| gst::FlowError::Error)?;

        let width = out_info.width() as i32;
        let height = out_info.height() as i32;
        let out_img_info = skia::ImageInfo::new(
            skia::ISize { width, height },
            video_format::gst_to_skia(out_info.format()).unwrap(),
            skia::AlphaType::Unpremul,
            None,
        );
        let draw_background = self.should_draw_background(token);
        let mut pads_to_draw = Vec::with_capacity(obj.num_sink_pads() as usize);
        obj.foreach_sink_pad(|_obj, pad| {
            let pad = pad.downcast_ref::<SkiaCompositorPad>().unwrap();
            let frame = match pad.prepared_frame(token) {
                Some(frame) => frame,
                None => return ControlFlow::Continue(()),
            };

            if pad.alpha() == 0. {
                return ControlFlow::Continue(());
            }

            if pads_to_draw.is_empty()
                && !draw_background
                && out_info.width() == frame.width()
                && out_info.height() == frame.height()
                && out_info.format() == frame.info().format()
            {
                gst::trace!(CAT, imp = self, "Copying frame directly to output buffer");
                mapped_mem.copy_from_slice(frame.plane_data(0).unwrap());

                return ControlFlow::Continue(());
            }

            pads_to_draw.push((pad.clone(), frame));

            ControlFlow::Continue(())
        });

        let mut surface =
            skia::surface::surfaces::wrap_pixels(&out_img_info, &mut mapped_mem, None, None)
                .ok_or(gst::FlowError::Error)?;

        let canvas = surface.canvas();
        if draw_background {
            self.draw_background(canvas, &out_info);
        }

        for (pad, frame) in pads_to_draw {
            let mut paint = skia::Paint::default();
            paint.set_anti_alias(pad.anti_alias());
            paint.set_blend_mode(pad.operator().into());
            paint.set_alpha_f(pad.alpha() as f32);
            let img_info = skia::ImageInfo::new(
                skia::ISize {
                    width: frame.width() as i32,
                    height: frame.height() as i32,
                },
                video_format::gst_to_skia(frame.info().format()).unwrap(),
                skia::AlphaType::Unpremul,
                None,
            );

            // SAFETY: We own the data throughout all the drawing process as we own a readable
            // reference on the underlying GStreamer buffer
            let image = unsafe {
                skia::image::images::raster_from_data(
                    &img_info,
                    skia::Data::new_bytes(frame.plane_data(0).unwrap()),
                    frame.info().stride()[0] as usize,
                )
            }
            .expect("Wrong image parameters to raster from data.");

            let mut desired_width = pad.width();
            if desired_width <= 0. {
                desired_width = frame.width() as f32;
            }
            let mut desired_height = pad.height();
            if desired_height <= 0. {
                desired_height = frame.height() as f32;
            }
            let src_rect = skia::Rect::from_wh(frame.width() as f32, frame.height() as f32); // Source rectangle
            let dst_rect =
                skia::Rect::from_xywh(pad.xpos(), pad.ypos(), desired_width, desired_height);
            gst::log!(
                CAT,
                imp = self,
                "Drawing frame from pad {} at {:?} to {:?}",
                pad.name(),
                src_rect,
                dst_rect
            );
            canvas.draw_image_rect(
                image,
                Some((&src_rect, skia::canvas::SrcRectConstraint::Strict)),
                dst_rect,
                &paint,
            );
        }
        drop(surface);

        Ok(gst::FlowSuccess::Ok)
    }
}

impl ChildProxyImpl for SkiaCompositor {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_sink_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .sink_pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}
