//
// Copyright (C) 2021 Bilal Elmoussaoui <bil.elmoussaoui@gmail.com>
// Copyright (C) 2021 Jordan Petridis <jordan@centricular.com>
// Copyright (C) 2021-2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use super::{frame, SinkEvent};
use crate::sink::frame::Frame;
use crate::sink::paintable::Paintable;

use glib::thread_guard::ThreadGuard;
use gtk::prelude::*;
use gtk::{gdk, glib};

use gst::{prelude::*, subclass::prelude::*};
use gst_base::subclass::prelude::*;
use gst_gl::prelude::{GLContextExt as _, *};
use gst_video::subclass::prelude::*;

use std::sync::LazyLock;
use std::sync::{
    atomic::{self, AtomicBool},
    Mutex,
};

use crate::utils;

// Global GL context that is created by the first sink and kept around until the end of the
// process. This is provided to other elements in the pipeline to make sure they create GL contexts
// that are sharing with the GTK GL context.
enum GLContext {
    Uninitialized,
    Unsupported,
    #[allow(unused)]
    Initialized {
        display: gst_gl::GLDisplay,
        wrapped_context: gst_gl::GLContext,
        gdk_context: ThreadGuard<gdk::GLContext>,
    },
}

static GL_CONTEXT: Mutex<GLContext> = Mutex::new(GLContext::Uninitialized);

pub(crate) static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gtk4paintablesink",
        gst::DebugColorFlags::empty(),
        Some("GTK4 Paintable sink"),
    )
});

struct StreamConfig {
    info: Option<super::frame::VideoInfo>,
    /// Orientation from a global scope tag
    global_orientation: frame::Orientation,
    /// Orientation from a stream scope tag
    stream_orientation: Option<frame::Orientation>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        StreamConfig {
            info: None,
            global_orientation: frame::Orientation::Rotate0,
            stream_orientation: None,
        }
    }
}

#[derive(Default)]
pub struct PaintableSink {
    paintable: Mutex<Option<ThreadGuard<Paintable>>>,
    window: Mutex<Option<ThreadGuard<gtk::Window>>>,
    config: Mutex<StreamConfig>,
    sender: Mutex<Option<async_channel::Sender<SinkEvent>>>,
    pending_frame: Mutex<Option<Frame>>,
    cached_caps: Mutex<Option<gst::Caps>>,
    settings: Mutex<Settings>,
    window_resized: AtomicBool,
}

#[derive(Default)]
struct Settings {
    window_width: u32,
    window_height: u32,
}

impl Drop for PaintableSink {
    fn drop(&mut self) {
        let mut paintable = self.paintable.lock().unwrap();
        if let Some(paintable) = paintable.take() {
            glib::MainContext::default().invoke(|| drop(paintable))
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PaintableSink {
    const NAME: &'static str = "GstGtk4PaintableSink";
    type Type = super::PaintableSink;
    type ParentType = gst_video::VideoSink;
    type Interfaces = (gst::ChildProxy,);
}

impl ObjectImpl for PaintableSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecObject::builder::<super::paintable::Paintable>("paintable")
                    .nick("Paintable")
                    .blurb("The Paintable the sink renders to")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("window-width")
                    .nick("Window width")
                    .blurb("the width of the main widget rendering the paintable")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("window-height")
                    .nick("Window height")
                    .blurb("the height of the main widget rendering the paintable")
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "paintable" => {
                // Fix segfault when GTK3 and GTK4 are loaded (e.g. `gst-inspect-1.0 -a`)
                // checking if GtkBin is registered to know if libgtk3.so is already present
                // GtkBin was dropped for GTK4 https://gitlab.gnome.org/GNOME/gtk/-/commit/3c165b3b77
                if glib::types::Type::from_name("GtkBin").is_some() {
                    gst::error!(CAT, imp = self, "Skipping the creation of paintable to avoid segfault between GTK3 and GTK4");
                    return None::<&Paintable>.to_value();
                }

                let mut paintable_guard = self.paintable.lock().unwrap();
                let mut created = false;
                if paintable_guard.is_none() {
                    created = true;
                    self.create_paintable(&mut paintable_guard);
                }

                let paintable = match &*paintable_guard {
                    Some(ref paintable) => paintable,
                    None => {
                        gst::error!(CAT, imp = self, "Failed to create paintable");
                        return None::<&Paintable>.to_value();
                    }
                };

                // Getter must be called from the main thread
                if !paintable.is_owner() {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Can't retrieve Paintable from non-main thread"
                    );
                    return None::<&Paintable>.to_value();
                }

                let paintable = paintable.get_ref().clone();
                drop(paintable_guard);

                if created {
                    let self_ = self.to_owned();
                    glib::MainContext::default().invoke(move || {
                        let paintable_guard = self_.paintable.lock().unwrap();
                        if let Some(paintable) = &*paintable_guard {
                            let paintable_clone = paintable.get_ref().clone();
                            drop(paintable_guard);
                            self_.obj().child_added(&paintable_clone, "paintable");
                        }
                    });
                }

                paintable.to_value()
            }
            "window-width" => {
                let settings = self.settings.lock().unwrap();
                settings.window_width.to_value()
            }
            "window-height" => {
                let settings = self.settings.lock().unwrap();
                settings.window_height.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "window-width" => {
                let mut settings = self.settings.lock().unwrap();
                let value = value.get().expect("type checked upstream");
                if settings.window_width != value {
                    self.window_resized.store(true, atomic::Ordering::SeqCst);
                }
                settings.window_width = value;
            }
            "window-height" => {
                let mut settings = self.settings.lock().unwrap();
                let value = value.get().expect("type checked upstream");
                if settings.window_height != value {
                    self.window_resized.store(true, atomic::Ordering::SeqCst);
                }
                settings.window_height = value;
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for PaintableSink {}

impl ElementImpl for PaintableSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "GTK 4 Paintable Sink",
                "Sink/Video",
                "A GTK 4 Paintable sink",
                "Bilal Elmoussaoui <bil.elmoussaoui@gmail.com>, Jordan Petridis <jordan@centricular.com>, Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // Those are the supported formats by a gdk::Texture
            let mut caps = gst::Caps::new_empty();
            {
                let caps = caps.get_mut().unwrap();

                #[cfg(all(target_os = "linux", feature = "dmabuf"))]
                {
                    for features in [
                        [
                            gst_allocators::CAPS_FEATURE_MEMORY_DMABUF,
                            gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION,
                        ]
                        .as_slice(),
                        [gst_allocators::CAPS_FEATURE_MEMORY_DMABUF].as_slice(),
                    ] {
                        let c = gst_video::VideoCapsBuilder::new()
                            .format(gst_video::VideoFormat::DmaDrm)
                            .features(features.iter().copied())
                            .build();
                        caps.append(c);
                    }
                }

                for features in [
                    #[cfg(any(
                        feature = "x11egl",
                        feature = "x11glx",
                        feature = "waylandegl",
                        target_os = "macos",
                        target_os = "windows"
                    ))]
                    Some(gst::CapsFeatures::new([
                        gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY,
                        gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION,
                    ])),
                    #[cfg(any(
                        feature = "x11egl",
                        feature = "x11glx",
                        feature = "waylandegl",
                        target_os = "macos",
                        target_os = "windows"
                    ))]
                    Some(gst::CapsFeatures::new([
                        gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY,
                    ])),
                    Some(gst::CapsFeatures::new([
                        "memory:SystemMemory",
                        gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION,
                    ])),
                    Some(gst::CapsFeatures::new([
                        gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION,
                    ])),
                    None,
                ] {
                    {
                        const GL_FORMATS: &[gst_video::VideoFormat] =
                            &[gst_video::VideoFormat::Rgba, gst_video::VideoFormat::Rgb];
                        const NON_GL_FORMATS: &[gst_video::VideoFormat] = &[
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Yuy2,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Yvyu,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Uyvy,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Vyuy,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Nv12,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Nv21,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Nv16,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Nv61,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Nv24,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P01010le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P01010be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P012Le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P012Be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P016Le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::P016Be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Yuv9,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Yvu9,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y41b,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I420,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Yv12,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y42b,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y444,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42010le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42010be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42210le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42210be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44410le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44410be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42012le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42012be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42212le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::I42212be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44412le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44412be,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44416le,
                            #[cfg(feature = "gtk_v4_20")]
                            gst_video::VideoFormat::Y44416be,
                            #[cfg(feature = "gtk_v4_14")]
                            gst_video::VideoFormat::Bgrx,
                            #[cfg(feature = "gtk_v4_14")]
                            gst_video::VideoFormat::Xrgb,
                            #[cfg(feature = "gtk_v4_14")]
                            gst_video::VideoFormat::Rgbx,
                            #[cfg(feature = "gtk_v4_14")]
                            gst_video::VideoFormat::Xbgr,
                            gst_video::VideoFormat::Bgra,
                            gst_video::VideoFormat::Argb,
                            gst_video::VideoFormat::Rgba,
                            gst_video::VideoFormat::Abgr,
                            gst_video::VideoFormat::Rgb,
                            gst_video::VideoFormat::Bgr,
                        ];

                        let formats = if features.as_ref().is_some_and(|features| {
                            features.contains(gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY)
                        }) {
                            GL_FORMATS
                        } else {
                            NON_GL_FORMATS
                        };

                        let mut c = gst_video::video_make_raw_caps(formats).build();

                        if let Some(features) = features {
                            let c = c.get_mut().unwrap();

                            if features.contains(gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY) {
                                c.set("texture-target", "2D")
                            }
                            c.set_features_simple(Some(features));
                        }
                        caps.append(c);
                    }
                }
            }

            vec![gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap()]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                let create_window = glib::program_name().as_deref() == Some("gst-launch-1.0")
                    || glib::program_name().as_deref() == Some("gst-play-1.0")
                    || std::env::var("GST_GTK4_WINDOW").as_deref() == Ok("1");

                if create_window {
                    let res = utils::invoke_on_main_thread(gtk::init);

                    if let Err(err) = res {
                        gst::error!(CAT, imp = self, "Failed to create initialize GTK: {err}");
                        return Err(gst::StateChangeError);
                    }
                }

                let mut paintable_guard = self.paintable.lock().unwrap();
                let mut created = false;
                if paintable_guard.is_none() {
                    created = true;
                    self.create_paintable(&mut paintable_guard);
                }

                if paintable_guard.is_none() {
                    gst::error!(CAT, imp = self, "Failed to create paintable");
                    return Err(gst::StateChangeError);
                }

                drop(paintable_guard);

                if created {
                    let self_ = self.to_owned();
                    glib::MainContext::default().invoke(move || {
                        let paintable_guard = self_.paintable.lock().unwrap();
                        if let Some(paintable) = &*paintable_guard {
                            let paintable_clone = paintable.get_ref().clone();
                            drop(paintable_guard);
                            self_.obj().child_added(&paintable_clone, "paintable");
                        }
                    });
                }

                // Notify the pipeline about the GL display and wrapped context so that any other
                // elements in the pipeline ideally use the same / create GL contexts that are
                // sharing with this one.
                {
                    let gl_context = GL_CONTEXT.lock().unwrap();
                    if let GLContext::Initialized {
                        display,
                        wrapped_context,
                        ..
                    } = &*gl_context
                    {
                        let display = display.clone();
                        let wrapped_context = wrapped_context.clone();
                        drop(gl_context);

                        gst_gl::gl_element_propagate_display_context(&*self.obj(), &display);
                        let mut ctx = gst::Context::new("gst.gl.app_context", true);
                        {
                            let ctx = ctx.get_mut().unwrap();
                            ctx.structure_mut().set("context", &wrapped_context);
                        }
                        let _ = self.obj().post_message(
                            gst::message::HaveContext::builder(ctx)
                                .src(&*self.obj())
                                .build(),
                        );
                    }
                }

                if create_window {
                    self.create_window();
                }
            }
            _ => (),
        }

        let res = self.parent_change_state(transition);

        match transition {
            gst::StateChange::PausedToReady => {
                *self.config.lock().unwrap() = StreamConfig::default();
                let _ = self.pending_frame.lock().unwrap().take();

                // Flush frames from the GDK paintable but don't wait
                // for this to finish as this can other deadlock.
                let self_ = self.to_owned();
                glib::MainContext::default().invoke(move || {
                    let paintable = self_.paintable.lock().unwrap();
                    if let Some(paintable) = &*paintable {
                        paintable.get_ref().handle_flush_frames();
                    }
                });
            }
            gst::StateChange::ReadyToNull => {
                let mut window_guard = self.window.lock().unwrap();
                if let Some(window) = window_guard.take() {
                    drop(window_guard);

                    glib::MainContext::default().invoke(move || {
                        let window = window.get_ref();
                        window.close();
                    });
                }
            }
            _ => (),
        }

        res
    }
}

impl BaseSinkImpl for PaintableSink {
    fn caps(&self, filter: Option<&gst::Caps>) -> Option<gst::Caps> {
        let cached_caps = self
            .cached_caps
            .lock()
            .expect("Failed to lock cached caps mutex")
            .clone();

        let mut tmp_caps = cached_caps.unwrap_or_else(|| {
            let templ = Self::pad_templates();
            templ[0].caps().clone()
        });

        gst::debug!(CAT, imp = self, "Advertising our own caps: {tmp_caps:?}");

        if let Some(filter_caps) = filter {
            gst::debug!(
                CAT,
                imp = self,
                "Intersecting with filter caps: {filter_caps:?}",
            );

            tmp_caps = filter_caps.intersect_with_mode(&tmp_caps, gst::CapsIntersectMode::First);
        };

        gst::debug!(CAT, imp = self, "Returning caps: {tmp_caps:?}");
        Some(tmp_caps)
    }

    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        gst::info!(CAT, imp = self, "Setting caps {caps:?}");

        #[allow(unused_mut)]
        let mut video_info = None;
        #[cfg(all(target_os = "linux", feature = "dmabuf"))]
        {
            if let Ok(info) = gst_video::VideoInfoDmaDrm::from_caps(caps) {
                video_info = Some(info.into());
            }
        }

        let video_info = match video_info {
            Some(info) => info,
            None => gst_video::VideoInfo::from_caps(caps)
                .map_err(|_| gst::loggable_error!(CAT, "Invalid caps"))?
                .into(),
        };

        self.config.lock().unwrap().info = Some(video_info);

        Ok(())
    }

    fn propose_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Proposing Allocation query");

        self.parent_propose_allocation(query)?;

        query.add_allocation_meta::<gst_video::VideoMeta>(None);

        let s = {
            let settings = self.settings.lock().unwrap();
            if (settings.window_width, settings.window_height) != (0, 0) {
                gst::debug!(
                    CAT,
                    imp = self,
                    "answering alloc query with size {}x{}",
                    settings.window_width,
                    settings.window_height
                );

                self.window_resized.store(false, atomic::Ordering::SeqCst);

                Some(
                    gst::Structure::builder("GstVideoOverlayCompositionMeta")
                        .field("width", settings.window_width)
                        .field("height", settings.window_height)
                        .build(),
                )
            } else {
                None
            }
        };

        query.add_allocation_meta::<gst_video::VideoOverlayCompositionMeta>(s.as_deref());

        {
            if let GLContext::Initialized {
                wrapped_context, ..
            } = &*GL_CONTEXT.lock().unwrap()
            {
                if wrapped_context.check_feature("GL_ARB_sync")
                    || wrapped_context.check_feature("GL_EXT_EGL_sync")
                {
                    query.add_allocation_meta::<gst_gl::GLSyncMeta>(None)
                }
            }
        }

        Ok(())
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, imp = self, "Handling query {:?}", query);

        match query.view_mut() {
            gst::QueryViewMut::Context(q) => {
                // Avoid holding the locks while we respond to the query
                // The objects are ref-counted anyway.
                let mut display_clone = None;
                let mut wrapped_context_clone = None;
                if let GLContext::Initialized {
                    display,
                    wrapped_context,
                    ..
                } = &*GL_CONTEXT.lock().unwrap()
                {
                    display_clone = Some(display.clone());
                    wrapped_context_clone = Some(wrapped_context.clone());
                }

                if let (Some(display), Some(wrapped_context)) =
                    (display_clone, wrapped_context_clone)
                {
                    return gst_gl::functions::gl_handle_context_query(
                        &*self.obj(),
                        q,
                        Some(&display),
                        None::<&gst_gl::GLContext>,
                        Some(&wrapped_context),
                    );
                }

                BaseSinkImplExt::parent_query(self, query)
            }
            _ => BaseSinkImplExt::parent_query(self, query),
        }
    }

    fn event(&self, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::StreamStart(_) => {
                let mut config = self.config.lock().unwrap();
                config.global_orientation = frame::Orientation::Rotate0;
                config.stream_orientation = None;
            }
            gst::EventView::Tag(ev) => {
                let mut config = self.config.lock().unwrap();
                let tags = ev.tag();
                let scope = tags.scope();
                let orientation = frame::Orientation::from_tags(tags);

                if scope == gst::TagScope::Global {
                    config.global_orientation = orientation.unwrap_or(frame::Orientation::Rotate0);
                } else {
                    config.stream_orientation = orientation;
                }
            }
            _ => (),
        }

        self.parent_event(event)
    }
}

impl VideoSinkImpl for PaintableSink {
    fn show_frame(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Rendering buffer {:?}", buffer);

        if self.window_resized.swap(false, atomic::Ordering::SeqCst) {
            gst::debug!(CAT, imp = self, "Window size changed, needs to reconfigure");
            let obj = self.obj();
            let sink = obj.sink_pad();
            sink.push_event(gst::event::Reconfigure::builder().build());
        }

        // Empty buffer, nothing to render
        if buffer.n_memory() == 0 {
            gst::trace!(
                CAT,
                imp = self,
                "Empty buffer, nothing to render. Returning."
            );
            return Ok(gst::FlowSuccess::Ok);
        };

        let config = self.config.lock().unwrap();
        let info = config.info.as_ref().ok_or_else(|| {
            gst::error!(CAT, imp = self, "Received no caps yet");
            gst::FlowError::NotNegotiated
        })?;
        let orientation = config
            .stream_orientation
            .unwrap_or(config.global_orientation);

        let wrapped_context = {
            {
                let gl_context = GL_CONTEXT.lock().unwrap();
                if let GLContext::Initialized {
                    wrapped_context, ..
                } = &*gl_context
                {
                    Some(wrapped_context.clone())
                } else {
                    None
                }
            }
        };
        let frame = Frame::new(self, buffer, info, orientation, wrapped_context.as_ref())
            .inspect_err(|_err| {
                gst::error!(CAT, imp = self, "Failed to map video frame");
            })?;
        self.pending_frame.lock().unwrap().replace(frame);

        let sender = self.sender.lock().unwrap();
        let sender = sender.as_ref().ok_or_else(|| {
            gst::error!(CAT, imp = self, "Have no main thread sender");
            gst::FlowError::Flushing
        })?;

        match sender.try_send(SinkEvent::FrameChanged) {
            Ok(_) => (),
            Err(async_channel::TrySendError::Full(_)) => {
                gst::warning!(CAT, imp = self, "Have too many pending frames");
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                gst::error!(CAT, imp = self, "Have main thread receiver shut down");
                return Err(gst::FlowError::Flushing);
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl PaintableSink {
    fn pending_frame(&self) -> Option<Frame> {
        self.pending_frame.lock().unwrap().take()
    }

    fn do_action(&self, action: SinkEvent) -> glib::ControlFlow {
        let paintable = self.paintable.lock().unwrap();
        let paintable = match &*paintable {
            Some(paintable) => paintable,
            None => return glib::ControlFlow::Break,
        };

        match action {
            SinkEvent::FrameChanged => {
                let Some(frame) = self.pending_frame() else {
                    return glib::ControlFlow::Continue;
                };
                gst::trace!(CAT, imp = self, "Frame changed");
                paintable.get_ref().handle_frame_changed(&self.obj(), frame);
            }
        }

        glib::ControlFlow::Continue
    }

    fn configure_caps(&self) {
        #[allow(unused_mut)]
        let mut tmp_caps = Self::pad_templates()[0].caps().clone();

        #[cfg(all(target_os = "linux", feature = "dmabuf"))]
        {
            let formats = utils::invoke_on_main_thread(move || {
                let Some(display) = gdk::Display::default() else {
                    return vec![];
                };
                let dmabuf_formats = display.dmabuf_formats();

                let mut formats = vec![];
                let n_formats = dmabuf_formats.n_formats();
                for i in 0..n_formats {
                    let (fourcc, modifier) = dmabuf_formats.format(i);

                    if fourcc == 0 || modifier == (u64::MAX >> 8) {
                        continue;
                    }

                    formats.push(gst_video::dma_drm_fourcc_to_string(fourcc, modifier));
                }

                formats
            });

            if formats.is_empty() {
                // Filter out dmabufs caps from the template pads if we have no supported formats
                tmp_caps = tmp_caps
                    .iter_with_features()
                    .filter(|(_, features)| {
                        !features.contains(gst_allocators::CAPS_FEATURE_MEMORY_DMABUF)
                    })
                    .map(|(s, c)| (s.to_owned(), c.to_owned()))
                    .collect::<gst::Caps>();
            } else {
                let tmp_caps = tmp_caps.make_mut();
                for (s, f) in tmp_caps.iter_with_features_mut() {
                    if f.contains(gst_allocators::CAPS_FEATURE_MEMORY_DMABUF) {
                        s.set("drm-format", gst::List::new(&formats));
                    }
                }
            }
        }

        {
            // Filter out GL caps from the template pads if we have no context
            if !matches!(&*GL_CONTEXT.lock().unwrap(), GLContext::Initialized { .. }) {
                tmp_caps = tmp_caps
                    .iter_with_features()
                    .filter(|(_, features)| {
                        !features.contains(gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY)
                    })
                    .map(|(s, c)| (s.to_owned(), c.to_owned()))
                    .collect::<gst::Caps>();
            }
        }

        self.cached_caps
            .lock()
            .expect("Failed to lock Mutex")
            .replace(tmp_caps);
    }

    fn create_window(&self) {
        let self_ = self.to_owned();
        glib::MainContext::default().invoke(move || {
            let mut window_guard = self_.window.lock().unwrap();
            if window_guard.is_some() {
                return;
            }

            let window = gtk::Window::new();

            let gst_widget = crate::RenderWidget::new(self_.obj().as_ref().upcast_ref());
            window.set_child(Some(&gst_widget));

            window.set_default_size(640, 480);
            if std::env::var("GST_GTK4_WINDOW_FULLSCREEN").as_deref() == Ok("1") {
                window.set_fullscreened(true);
            }

            window.connect_close_request({
                let self_ = self_.clone();
                move |_window| {
                    if self_.window.lock().unwrap().is_some() {
                        gst::element_imp_error!(
                            self_,
                            gst::ResourceError::NotFound,
                            ("Output window was closed")
                        );
                    }

                    glib::Propagation::Proceed
                }
            });

            window.present();

            *window_guard = Some(ThreadGuard::new(window));
        });
    }

    fn create_paintable(&self, paintable_storage: &mut Option<ThreadGuard<Paintable>>) {
        #[cfg(any(
            feature = "x11egl",
            feature = "x11glx",
            feature = "waylandegl",
            target_os = "macos",
            target_os = "windows"
        ))]
        {
            self.initialize_gl_context();
        }
        #[cfg(not(any(
            feature = "x11egl",
            feature = "x11glx",
            feature = "waylandegl",
            target_os = "macos",
            target_os = "windows"
        )))]
        {
            gst::debug!(CAT, imp = self, "No GL platform enabled");
            *GL_CONTEXT.lock().unwrap() = GLContext::Unsupported;
        }

        self.configure_caps();
        self.initialize_paintable(paintable_storage);
    }

    fn initialize_paintable(&self, paintable_storage: &mut Option<ThreadGuard<Paintable>>) {
        gst::debug!(CAT, imp = self, "Initializing paintable");

        // The channel for the SinkEvents
        let (sender, receiver) = async_channel::bounded(3);

        // Spawn an async task on the main context to handle the channel messages
        let main_context = glib::MainContext::default();

        let self_ = self.downgrade();
        main_context.spawn(async move {
            while let Ok(action) = receiver.recv().await {
                let Some(self_) = self_.upgrade() else {
                    break;
                };

                self_.do_action(action);
            }
        });

        // Create the paintable from the main thread
        let paintable = utils::invoke_on_main_thread(move || {
            let gdk_context =
                if let GLContext::Initialized { gdk_context, .. } = &*GL_CONTEXT.lock().unwrap() {
                    Some(gdk_context.get_ref().clone())
                } else {
                    None
                };
            ThreadGuard::new(Paintable::new(gdk_context))
        });

        *paintable_storage = Some(paintable);

        *self.sender.lock().unwrap() = Some(sender);
    }

    #[cfg(any(
        feature = "x11egl",
        feature = "x11glx",
        feature = "waylandegl",
        target_os = "macos",
        target_os = "windows"
    ))]
    fn initialize_gl_context(&self) {
        gst::debug!(CAT, imp = self, "Realizing GDK GL Context");

        let self_ = self.to_owned();
        utils::invoke_on_main_thread(move || {
            self_.initialize_gl_context_main();
        });
    }

    #[cfg(any(
        feature = "x11egl",
        feature = "x11glx",
        feature = "waylandegl",
        target_os = "macos",
        target_os = "windows"
    ))]
    fn initialize_gl_context_main(&self) {
        gst::debug!(CAT, imp = self, "Realizing GDK GL Context from main thread");

        let mut gl_context_guard = GL_CONTEXT.lock().unwrap();
        if !matches!(&*gl_context_guard, GLContext::Uninitialized) {
            gst::debug!(CAT, imp = self, "Already initialized GL context before");
            return;
        }
        *gl_context_guard = GLContext::Unsupported;

        // This can return NULL but only happens in 2 situations:
        // * If the function is called before gtk_init
        // * If the function is called after gdk_display_close(default_display)
        // Both of which are treated as programming errors.
        //
        // However, when we are building the docs, gtk_init doesn't get called
        // and this would cause the documentation generation to error.
        // Thus its okayish to return None here and fallback to software
        // rendering, since this path isn't going to be used by applications
        // anyway.
        //
        // FIXME: add a couple more gtk_init checks across the codebase where
        // applicable since this is no longer going to panic.
        let gdk_display = match gdk::Display::default() {
            Some(display) => display,
            None => {
                gst::warning!(CAT, imp = self, "Failed to retrieve GDK display");
                return;
            }
        };
        let gdk_context = match gdk_display.create_gl_context() {
            Ok(gdk_context) => gdk_context,
            Err(err) => {
                gst::warning!(CAT, imp = self, "Failed to create GDK GL Context: {err}");
                return;
            }
        };

        gst::info!(CAT, imp = self, "Realizing GDK GL Context",);

        if let Err(err) = gdk_context.realize() {
            gst::warning!(CAT, imp = self, "Failed to realize GDK GL Context: {err}");
            return;
        }

        gst::info!(
            CAT,
            imp = self,
            "Successfully realized GDK GL Context of type {}",
            gdk_context.type_().name()
        );

        gdk_context.make_current();

        let Some((display, wrapped_context)) = self.initialize_gst_gl(&gdk_display, &gdk_context)
        else {
            gst::warning!(CAT, imp = self, "Failed to initialize GStreamer GL context");
            return;
        };

        match wrapped_context.activate(true) {
            Ok(_) => gst::info!(CAT, imp = self, "Successfully activated GL Context"),
            Err(_) => {
                gst::error!(CAT, imp = self, "Failed to activate GL context",);
                return;
            }
        };

        if let Err(err) = wrapped_context.fill_info() {
            gst::error!(
                CAT,
                imp = self,
                "Failed to fill info on the GL Context: {err}",
            );
            // Deactivate the context upon failure
            if wrapped_context.activate(false).is_err() {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to deactivate the context after failing fill info",
                );
            }
            return;
        }

        gst::info!(CAT, imp = self, "Successfully initialized GL Context");

        *gl_context_guard = GLContext::Initialized {
            display,
            wrapped_context,
            gdk_context: ThreadGuard::new(gdk_context),
        };
    }

    #[cfg(any(
        feature = "x11egl",
        feature = "x11glx",
        feature = "waylandegl",
        target_os = "macos",
        target_os = "windows"
    ))]
    fn initialize_gst_gl(
        &self,
        gdk_display: &gdk::Display,
        gdk_context: &gdk::GLContext,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        match gdk_context.type_().name() {
            #[cfg(feature = "x11egl")]
            "GdkX11GLContextEGL" => self.initialize_x11egl(gdk_display),
            #[cfg(feature = "x11glx")]
            "GdkX11GLContextGLX" => self.initialize_x11glx(gdk_display),
            #[cfg(feature = "waylandegl")]
            "GdkWaylandGLContext" => self.initialize_waylandegl(gdk_display),
            #[cfg(target_os = "macos")]
            "GdkMacosGLContext" => self.initialize_macosgl(gdk_display),
            #[cfg(target_os = "windows")]
            "GdkWin32GLContextWGL" => self.initialize_wgl(gdk_display, &gdk_context),
            #[cfg(all(target_os = "windows", feature = "winegl"))]
            "GdkWin32GLContextEGL" => self.initialize_winegl(gdk_display),
            context_type => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Unsupported GDK context {context_type} with display type {} for GL. This might be due to not having enabled a backend for GL when building the plugin",
                    gdk_display.type_().name(),
                );
                None
            }
        }
    }

    #[cfg(feature = "x11egl")]
    fn initialize_x11egl(
        &self,
        display: &gdk::Display,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL for x11 EGL backend and display"
        );

        let platform = gst_gl::GLPlatform::EGL;
        let (gl_api, _, _) = gst_gl::GLContext::current_gl_api(platform);
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext");
            return None;
        }

        // FIXME: bindings
        unsafe {
            use glib::translate::*;

            let display = display.downcast_ref::<gdk_x11::X11Display>().unwrap();
            let x11_display =
                gdk_x11::ffi::gdk_x11_display_get_egl_display(display.to_glib_none().0);
            if x11_display.is_null() {
                gst::error!(CAT, imp = self, "Failed to get EGL display");
                return None;
            }

            let gst_display = gst_gl_egl::ffi::gst_gl_display_egl_new_with_egl_display(x11_display);
            let gst_display =
                gst_gl::GLDisplay::from_glib_full(gst_display as *mut gst_gl::ffi::GstGLDisplay);

            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);
            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }

    #[cfg(feature = "x11glx")]
    fn initialize_x11glx(
        &self,
        display: &gdk::Display,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL for x11 GLX backend and display"
        );

        let platform = gst_gl::GLPlatform::GLX;
        let (gl_api, _, _) = gst_gl::GLContext::current_gl_api(platform);
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext");
            return None;
        }

        // FIXME: bindings
        unsafe {
            use glib::translate::*;

            let display = display.downcast_ref::<gdk_x11::X11Display>().unwrap();
            let x11_display = gdk_x11::ffi::gdk_x11_display_get_xdisplay(display.to_glib_none().0);
            if x11_display.is_null() {
                gst::error!(CAT, imp = self, "Failed to get X11 display");
                return None;
            }

            let gst_display = gst_gl_x11::ffi::gst_gl_display_x11_new_with_display(x11_display);
            let gst_display =
                gst_gl::GLDisplay::from_glib_full(gst_display as *mut gst_gl::ffi::GstGLDisplay);

            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);
            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }

    #[cfg(feature = "waylandegl")]
    fn initialize_waylandegl(
        &self,
        display: &gdk::Display,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL for Wayland EGL backend and display"
        );

        let platform = gst_gl::GLPlatform::EGL;
        let (gl_api, _, _) = gst_gl::GLContext::current_gl_api(platform);
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext");
            return None;
        }

        // FIXME: bindings
        unsafe {
            use glib::translate::*;

            // let wayland_display = gdk_wayland::WaylandDisplay::wl_display(display.downcast());
            // get the ptr directly since we are going to use it raw
            let display = display
                .downcast_ref::<gdk_wayland::WaylandDisplay>()
                .unwrap();
            let wayland_display =
                gdk_wayland::ffi::gdk_wayland_display_get_wl_display(display.to_glib_none().0);
            if wayland_display.is_null() {
                gst::error!(CAT, imp = self, "Failed to get Wayland display");
                return None;
            }

            let gst_display =
                gst_gl_wayland::ffi::gst_gl_display_wayland_new_with_display(wayland_display);
            let gst_display =
                gst_gl::GLDisplay::from_glib_full(gst_display as *mut gst_gl::ffi::GstGLDisplay);

            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);

            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }

    #[cfg(target_os = "macos")]
    fn initialize_macosgl(
        &self,
        display: &gdk::Display,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL for macOS backend and display"
        );

        let platform = gst_gl::GLPlatform::CGL;
        let (gl_api, _, _) = gst_gl::GLContext::current_gl_api(platform);
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext");
            return None;
        }

        let gst_display = gst_gl::GLDisplay::new();
        unsafe {
            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);

            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }

    #[cfg(target_os = "windows")]
    fn initialize_wgl(
        &self,
        _display: &gdk::Display,
        context: &gdk::GLContext,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL with for Windows WGL backend and display."
        );

        let platform = gst_gl::GLPlatform::WGL;

        let gl_api = if context.is_legacy() {
            gst_gl::GLAPI::OPENGL
        } else {
            gst_gl::GLAPI::OPENGL3
        };
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext",);
            return None;
        }

        unsafe {
            let gst_display =
                if let Some(display) = gst_gl::GLDisplay::with_type(gst_gl::GLDisplayType::WIN32) {
                    display
                } else {
                    gst::error!(CAT, imp = self, "Failed to get GL display");
                    return None;
                };

            gst_display.filter_gl_api(gl_api);

            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);
            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }

    #[cfg(all(target_os = "windows", feature = "winegl"))]
    fn initialize_winegl(
        &self,
        display: &gdk::Display,
    ) -> Option<(gst_gl::GLDisplay, gst_gl::GLContext)> {
        gst::info!(
            CAT,
            imp = self,
            "Initializing GL with for Windows EGL backend and display."
        );

        let platform = gst_gl::GLPlatform::EGL;

        let (gl_api, _, _) = gst_gl::GLContext::current_gl_api(platform);
        let gl_ctx = gst_gl::GLContext::current_gl_context(platform);

        if gl_ctx == 0 {
            gst::error!(CAT, imp = self, "Failed to get handle from GdkGLContext",);
            return None;
        }

        // FIXME: bindings
        unsafe {
            use gdk_win32::prelude::*;
            use glib::translate::*;

            let d = display.downcast_ref::<gdk_win32::Win32Display>().unwrap();
            let egl_display = d.egl_display().unwrap().as_ptr();

            // TODO: On the binary distribution of GStreamer for Windows, this symbol is not there
            let gst_display =
                gst_gl_egl::ffi::gst_gl_display_egl_from_gl_display(egl_display.cast());
            if gst_display.is_null() {
                gst::error!(CAT, imp = self, "Failed to get EGL display");
                return None;
            }
            let gst_display =
                gst_gl::GLDisplay::from_glib_full(gst_display as *mut gst_gl::ffi::GstGLDisplay);

            gst_display.filter_gl_api(gl_api);

            let wrapped_context =
                gst_gl::GLContext::new_wrapped(&gst_display, gl_ctx, platform, gl_api);

            let wrapped_context = match wrapped_context {
                None => {
                    gst::error!(CAT, imp = self, "Failed to create wrapped GL context");
                    return None;
                }
                Some(wrapped_context) => wrapped_context,
            };

            Some((gst_display, wrapped_context))
        }
    }
}

impl ChildProxyImpl for PaintableSink {
    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        if index != 0 {
            return None;
        }

        let paintable = self.paintable.lock().unwrap();
        paintable
            .as_ref()
            .filter(|p| p.is_owner())
            .map(|p| p.get_ref().upcast_ref::<glib::Object>().clone())
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        if name == "paintable" {
            return self.child_by_index(0);
        }
        None
    }

    fn children_count(&self) -> u32 {
        let paintable = self.paintable.lock().unwrap();
        if paintable.is_some() {
            1
        } else {
            0
        }
    }
}
