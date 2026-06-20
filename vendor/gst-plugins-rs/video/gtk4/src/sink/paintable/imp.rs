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

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::sink::frame::{self, Frame, Texture};

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gtk4paintable",
        gst::DebugColorFlags::empty(),
        Some("GTK4 Paintable Sink Paintable"),
    )
});

#[derive(Debug)]
pub struct Paintable {
    paintables: RefCell<Vec<Texture>>,
    cached_textures: RefCell<HashMap<super::super::frame::TextureCacheId, gdk::Texture>>,
    gl_context: RefCell<Option<gdk::GLContext>>,
    background_color: Cell<gdk::RGBA>,
    #[cfg(feature = "gtk_v4_10")]
    scaling_filter: Cell<gsk::ScalingFilter>,
    use_scaling_filter: Cell<bool>,
    force_aspect_ratio: Cell<bool>,
    orientation: Cell<frame::Orientation>,
    #[cfg(not(feature = "gtk_v4_10"))]
    premult_shader: gsk::GLShader,
}

impl Default for Paintable {
    fn default() -> Self {
        Self {
            paintables: Default::default(),
            cached_textures: Default::default(),
            gl_context: Default::default(),
            background_color: Cell::new(gdk::RGBA::BLACK),
            #[cfg(feature = "gtk_v4_10")]
            scaling_filter: Cell::new(gsk::ScalingFilter::Linear),
            use_scaling_filter: Cell::new(false),
            force_aspect_ratio: Cell::new(false),
            orientation: Cell::new(frame::Orientation::Auto),
            #[cfg(not(feature = "gtk_v4_10"))]
            premult_shader: gsk::GLShader::from_bytes(&glib::Bytes::from_static(include_bytes!(
                "premult.glsl"
            ))),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Paintable {
    const NAME: &'static str = "GstGtk4Paintable";
    type Type = super::Paintable;
    type Interfaces = (gdk::Paintable,);
}

impl ObjectImpl for Paintable {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecObject::builder::<gdk::GLContext>("gl-context")
                    .nick("GL Context")
                    .blurb("GL context to use for rendering")
                    .construct_only()
                    .build(),
                glib::ParamSpecUInt::builder("background-color")
                    .nick("Background Color")
                    .blurb("Background color to render behind the video frame and in the borders")
                    .default_value(0)
                    .build(),
                #[cfg(feature = "gtk_v4_10")]
                glib::ParamSpecEnum::builder_with_default::<gsk::ScalingFilter>(
                    "scaling-filter",
                    gsk::ScalingFilter::Linear,
                )
                .nick("Scaling Filter")
                .blurb("Scaling filter to use for rendering")
                .build(),
                #[cfg(feature = "gtk_v4_10")]
                glib::ParamSpecBoolean::builder("use-scaling-filter")
                    .nick("Use Scaling Filter")
                    .blurb("Use selected scaling filter or GTK default for rendering")
                    .default_value(false)
                    .build(),
                glib::ParamSpecBoolean::builder("force-aspect-ratio")
                    .nick("Force Aspect Ratio")
                    .blurb("When enabled, scaling will respect original aspect ratio")
                    .default_value(false)
                    .build(),
                glib::ParamSpecEnum::builder::<frame::Orientation>("orientation")
                    .nick("Orientation")
                    .blurb("Orientation of the video frames")
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "gl-context" => self.gl_context.borrow().to_value(),
            "background-color" => {
                let color = self.background_color.get();

                let v = ((f32::clamp(color.red() * 255.0, 0.0, 255.0) as u32) << 24)
                    | ((f32::clamp(color.green() * 255.0, 0.0, 255.0) as u32) << 16)
                    | ((f32::clamp(color.blue() * 255.0, 0.0, 255.0) as u32) << 8)
                    | (f32::clamp(color.alpha() * 255.0, 0.0, 255.0) as u32);

                v.to_value()
            }
            #[cfg(feature = "gtk_v4_10")]
            "scaling-filter" => self.scaling_filter.get().to_value(),
            #[cfg(feature = "gtk_v4_10")]
            "use-scaling-filter" => self.use_scaling_filter.get().to_value(),
            "force-aspect-ratio" => self.force_aspect_ratio.get().to_value(),
            "orientation" => self.orientation.get().to_value(),
            _ => unimplemented!(),
        }
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "gl-context" => {
                *self.gl_context.borrow_mut() = value.get::<Option<gtk::gdk::GLContext>>().unwrap();
            }
            "background-color" => {
                let v = value.get::<u32>().unwrap();
                let red = ((v & 0xff_00_00_00) >> 24) as f32 / 255.0;
                let green = ((v & 0x00_ff_00_00) >> 16) as f32 / 255.0;
                let blue = ((v & 0x00_00_ff_00) >> 8) as f32 / 255.0;
                let alpha = (v & 0x00_00_00_ff) as f32 / 255.0;
                self.background_color
                    .set(gdk::RGBA::new(red, green, blue, alpha))
            }
            #[cfg(feature = "gtk_v4_10")]
            "scaling-filter" => self.scaling_filter.set(value.get().unwrap()),
            #[cfg(feature = "gtk_v4_10")]
            "use-scaling-filter" => self.use_scaling_filter.set(value.get().unwrap()),
            "force-aspect-ratio" => self.force_aspect_ratio.set(value.get().unwrap()),
            "orientation" => {
                let new_orientation = value.get().unwrap();
                if new_orientation != self.orientation.get() {
                    self.orientation.set(new_orientation);
                    self.obj().invalidate_size();
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl PaintableImpl for Paintable {
    fn intrinsic_height(&self) -> i32 {
        if let Some(paintable) = self.paintables.borrow().first() {
            if self
                .effective_orientation(paintable.orientation)
                .is_flip_width_height()
            {
                f32::round(paintable.width) as i32
            } else {
                f32::round(paintable.height) as i32
            }
        } else {
            0
        }
    }

    fn intrinsic_width(&self) -> i32 {
        if let Some(paintable) = self.paintables.borrow().first() {
            if self
                .effective_orientation(paintable.orientation)
                .is_flip_width_height()
            {
                f32::round(paintable.height) as i32
            } else {
                f32::round(paintable.width) as i32
            }
        } else {
            0
        }
    }

    fn intrinsic_aspect_ratio(&self) -> f64 {
        if let Some(paintable) = self.paintables.borrow().first() {
            if self
                .effective_orientation(paintable.orientation)
                .is_flip_width_height()
            {
                paintable.height as f64 / paintable.width as f64
            } else {
                paintable.width as f64 / paintable.height as f64
            }
        } else {
            0.0
        }
    }

    fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
        let snapshot = snapshot.downcast_ref::<gtk::Snapshot>().unwrap();
        let background_color = self.background_color.get();
        let force_aspect_ratio = self.force_aspect_ratio.get();
        let paintables = self.paintables.borrow();

        let Some(first_paintable) = paintables.first() else {
            gst::trace!(CAT, imp = self, "Snapshotting black frame {width}x{height}");
            snapshot.append_color(
                &background_color,
                &graphene::Rect::new(0f32, 0f32, width as f32, height as f32),
            );

            return;
        };

        gst::trace!(CAT, imp = self, "Snapshotting frame {width}x{height}");

        // The first paintable is the actual video frame and defines the overall size.
        //
        // Based on its size relative to the snapshot width/height, all other paintables are
        // scaled accordingly.
        //
        // We also only consider the orientation of the first paintable for now and rotate all
        // overlays consistently with that to follow the behaviour of glvideoflip.
        let effective_orientation = self.effective_orientation(first_paintable.orientation);

        // First do the rotation around the center of the whole snapshot area
        if effective_orientation != frame::Orientation::Rotate0 {
            snapshot.translate(&graphene::Point::new(
                width as f32 / 2.0,
                height as f32 / 2.0,
            ));
        }
        match effective_orientation {
            frame::Orientation::Rotate0 => {}
            frame::Orientation::Rotate90 => {
                snapshot.rotate(90.0);
            }
            frame::Orientation::Rotate180 => {
                snapshot.scale(-1.0, -1.0);
            }
            frame::Orientation::Rotate270 => {
                snapshot.rotate(270.0);
            }
            frame::Orientation::FlipRotate0 => {
                snapshot.scale(-1.0, 1.0);
            }
            frame::Orientation::FlipRotate90 => {
                snapshot.rotate(90.0);
                snapshot.scale(-1.0, 1.0);
            }
            frame::Orientation::FlipRotate180 => {
                snapshot.scale(1.0, -1.0);
            }
            frame::Orientation::FlipRotate270 => {
                snapshot.rotate(270.0);
                snapshot.scale(-1.0, 1.0);
            }
            frame::Orientation::Auto => unreachable!(),
        }
        if effective_orientation != frame::Orientation::Rotate0 {
            if effective_orientation.is_flip_width_height() {
                snapshot.translate(&graphene::Point::new(
                    -height as f32 / 2.0,
                    -width as f32 / 2.0,
                ));
            } else {
                snapshot.translate(&graphene::Point::new(
                    -width as f32 / 2.0,
                    -height as f32 / 2.0,
                ));
            }
        }

        // The rotation is applied now and we're back at the origin at this point

        // Width / height of the overall frame that we're drawing. This has to be flipped
        // if a 90/270 degree rotation is applied.
        let (frame_width, frame_height) = if effective_orientation.is_flip_width_height() {
            (first_paintable.height, first_paintable.width)
        } else {
            (first_paintable.width, first_paintable.height)
        };

        // Amount of scaling that has to be applied to the main frame and all overlays to fill the
        // available area
        let mut scale_x = width / frame_width as f64;
        let mut scale_y = height / frame_height as f64;

        // Usually the caller makes sure that the aspect ratio is preserved. To enforce this here
        // optionally, we scale the frame equally in both directions and center it. In addition the
        // background color is drawn behind the frame to fill the gaps.
        //
        // This is not done by default for performance reasons and usually would draw a <1px
        // background.
        if force_aspect_ratio {
            let mut trans_x = 0.0;
            let mut trans_y = 0.0;

            if (scale_x - scale_y).abs() > f64::EPSILON {
                if scale_x > scale_y {
                    trans_x = (width - (frame_width as f64 * scale_y)) / 2.0;
                    scale_x = scale_y;
                } else {
                    trans_y = (height - (frame_height as f64 * scale_x)) / 2.0;
                    scale_y = scale_x;
                }
            }

            if !background_color.is_clear() && (trans_x > f64::EPSILON || trans_y > f64::EPSILON) {
                // Clamping for the bounds below has to be flipped over for 90/270 degree rotations.
                let (width, height) = if effective_orientation.is_flip_width_height() {
                    (height, width)
                } else {
                    (width, height)
                };

                snapshot.append_color(
                    &background_color,
                    &graphene::Rect::new(0f32, 0f32, width as f32, height as f32),
                );
            }
            if effective_orientation.is_flip_width_height() {
                std::mem::swap(&mut trans_x, &mut trans_y);
            }
            snapshot.translate(&graphene::Point::new(trans_x as f32, trans_y as f32));
        }

        // At this point we're at the origin of the area into which the actual video frame is drawn

        // Make immutable
        let scale_x = scale_x;
        let scale_y = scale_y;

        for (
            idx,
            Texture {
                texture,
                x,
                y,
                width: paintable_width,
                height: paintable_height,
                global_alpha,
                has_alpha,
                orientation: _orientation,
            },
        ) in paintables.iter().enumerate()
        {
            snapshot.push_opacity(*global_alpha as f64);

            // Clamping for the bounds below has to be flipped over for 90/270 degree rotations.
            let (width, height) = if effective_orientation.is_flip_width_height() {
                (height, width)
            } else {
                (width, height)
            };

            let bounds = if !force_aspect_ratio && idx == 0 {
                // While this should end up with width again, be explicit in this case to avoid
                // rounding errors and fill the whole area with the video frame.
                graphene::Rect::new(0.0, 0.0, width as f32, height as f32)
            } else {
                // Scale texture position and size with the same scale factor as the main video
                // frame, and make sure to not render outside (0, 0, width, height).
                let (rect_x, rect_y) = if effective_orientation.is_flip_width_height() {
                    (
                        f32::clamp(*y * scale_y as f32, 0.0, width as f32),
                        f32::clamp(*x * scale_x as f32, 0.0, height as f32),
                    )
                } else {
                    (
                        f32::clamp(*x * scale_x as f32, 0.0, width as f32),
                        f32::clamp(*y * scale_y as f32, 0.0, height as f32),
                    )
                };

                let (texture_width, texture_height) =
                    if effective_orientation.is_flip_width_height() {
                        (
                            f32::min(*paintable_width * scale_y as f32, width as f32),
                            f32::min(*paintable_height * scale_x as f32, height as f32),
                        )
                    } else {
                        (
                            f32::min(*paintable_width * scale_x as f32, width as f32),
                            f32::min(*paintable_height * scale_y as f32, height as f32),
                        )
                    };

                graphene::Rect::new(rect_x, rect_y, texture_width, texture_height)
            };

            // Only premultiply GL textures that expect to be in premultiplied RGBA format.
            //
            // For GTK 4.14 or newer we use the correct format directly when building the
            // texture, but only if a GLES3+ context is used. In that case the NGL renderer is
            // used by GTK, which supports non-premultiplied formats correctly and fast.
            //
            // For GTK 4.10-4.12, or 4.14 and newer if a GLES2 context is used, we use a
            // self-mask to pre-multiply the alpha.
            //
            // For GTK before 4.10, we use a GL shader and hope that it works.
            #[cfg(feature = "gtk_v4_10")]
            {
                let context_requires_premult = {
                    #[cfg(feature = "gtk_v4_14")]
                    {
                        self.gl_context.borrow().as_ref().is_some_and(|context| {
                            context.api() != gdk::GLAPI::GLES || context.version().0 < 3
                        })
                    }

                    #[cfg(not(feature = "gtk_v4_14"))]
                    {
                        true
                    }
                };

                let do_premult =
                    context_requires_premult && texture.is::<gdk::GLTexture>() && *has_alpha;
                if do_premult {
                    snapshot.push_mask(gsk::MaskMode::Alpha);
                    if self.use_scaling_filter.get() {
                        #[cfg(feature = "gtk_v4_10")]
                        snapshot.append_scaled_texture(texture, self.scaling_filter.get(), &bounds);
                    } else {
                        snapshot.append_texture(texture, &bounds);
                    }
                    snapshot.pop(); // pop mask

                    // color matrix to set alpha of the source to 1.0 as it was
                    // already applied via the mask just above.
                    snapshot.push_color_matrix(
                        &graphene::Matrix::from_float({
                            [
                                1.0, 0.0, 0.0, 0.0, //
                                0.0, 1.0, 0.0, 0.0, //
                                0.0, 0.0, 1.0, 0.0, //
                                0.0, 0.0, 0.0, 0.0,
                            ]
                        }),
                        &graphene::Vec4::new(0.0, 0.0, 0.0, 1.0),
                    );
                }

                if self.use_scaling_filter.get() {
                    #[cfg(feature = "gtk_v4_10")]
                    snapshot.append_scaled_texture(texture, self.scaling_filter.get(), &bounds);
                } else {
                    snapshot.append_texture(texture, &bounds);
                }

                if do_premult {
                    snapshot.pop(); // pop color matrix
                    snapshot.pop(); // pop mask 2
                }
            }
            #[cfg(not(feature = "gtk_v4_10"))]
            {
                let do_premult =
                    texture.is::<gdk::GLTexture>() && *has_alpha && gtk::micro_version() < 13;
                if do_premult {
                    snapshot.push_gl_shader(
                        &self.premult_shader,
                        &bounds,
                        gsk::ShaderArgsBuilder::new(&self.premult_shader, None).to_args(),
                    );
                }

                if self.use_scaling_filter.get() {
                    #[cfg(feature = "gtk_v4_10")]
                    snapshot.append_scaled_texture(texture, self.scaling_filter.get(), &bounds);
                } else {
                    snapshot.append_texture(texture, &bounds);
                }

                if do_premult {
                    snapshot.gl_shader_pop_texture(); // pop texture appended above from the shader
                    snapshot.pop(); // pop shader
                }
            }

            snapshot.pop(); // pop opacity
        }
    }
}

impl Paintable {
    fn effective_orientation(
        &self,
        paintable_orientation: frame::Orientation,
    ) -> frame::Orientation {
        let orientation = self.orientation.get();
        if orientation != frame::Orientation::Auto {
            return orientation;
        }

        assert_ne!(paintable_orientation, frame::Orientation::Auto);
        paintable_orientation
    }

    pub(super) fn handle_frame_changed(&self, sink: &crate::PaintableSink, frame: Frame) {
        let context = self.gl_context.borrow();

        gst::trace!(CAT, imp = self, "Received new frame");

        let new_paintables =
            match frame.into_textures(context.as_ref(), &mut self.cached_textures.borrow_mut()) {
                Ok(textures) => textures,
                Err(err) => {
                    gst::element_error!(
                        sink,
                        gst::ResourceError::Failed,
                        ["Failed to transform frame into textures: {err}"]
                    );
                    return;
                }
            };

        let flip_width_height = |(width, height, orientation): (u32, u32, frame::Orientation)| {
            if orientation.is_flip_width_height() {
                (height, width)
            } else {
                (width, height)
            }
        };

        let new_size = new_paintables
            .first()
            .map(|p| {
                flip_width_height((
                    f32::round(p.width) as u32,
                    f32::round(p.height) as u32,
                    p.orientation,
                ))
            })
            .unwrap();

        let old_paintables = self.paintables.replace(new_paintables);
        let old_size = old_paintables.first().map(|p| {
            flip_width_height((
                f32::round(p.width) as u32,
                f32::round(p.height) as u32,
                p.orientation,
            ))
        });

        if Some(new_size) != old_size {
            gst::debug!(
                CAT,
                imp = self,
                "Size changed from {old_size:?} to {new_size:?}",
            );
            self.obj().invalidate_size();
        }

        self.obj().invalidate_contents();
    }

    pub(super) fn handle_flush_frames(&self) {
        gst::debug!(CAT, imp = self, "Flushing frames");
        self.paintables.borrow_mut().clear();
        self.cached_textures.borrow_mut().clear();
        self.obj().invalidate_size();
        self.obj().invalidate_contents();
    }
}
