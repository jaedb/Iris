use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use pango::prelude::*;

use std::sync::LazyLock;

use std::collections::HashSet;
use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "onvifmetadataoverlay",
        gst::DebugColorFlags::empty(),
        Some("ONVIF metadata overlay element"),
    )
});

const DEFAULT_FONT_DESC: &str = "monospace 12";

#[derive(Debug)]
struct Point {
    x: u32,
    y: u32,
}

// Shape description in cairo coordinates (0, 0) is top left
#[derive(Debug)]
struct Shape {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    points: Vec<Point>,
    // Optional text rendered from top left of rectangle
    tag: Option<String>,
}

#[derive(Default)]
struct State {
    video_info: Option<gst_video::VideoInfo>,
    composition: Option<gst_video::VideoOverlayComposition>,
    layout: Option<pango::Layout>,
    attach: bool,
}

// SAFETY: Required because `pango::Layout` is not `Send` but the whole `State` needs to be.
// We ensure that no additional references to the layout are ever created, which makes it safe
// to send it to other threads as long as only a single thread uses it concurrently.
unsafe impl Send for State {}

struct Settings {
    font_desc: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_desc: String::from(DEFAULT_FONT_DESC),
        }
    }
}

pub struct OnvifMetadataOverlay {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl OnvifMetadataOverlay {
    fn negotiate(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let video_info = {
            let state = self.state.lock().unwrap();
            match state.video_info.as_ref() {
                Some(video_info) => Ok(video_info.clone()),
                None => {
                    gst::element_imp_error!(
                        self,
                        gst::CoreError::Negotiation,
                        ["Element hasn't received valid video caps at negotiation time"]
                    );
                    Err(gst::FlowError::NotNegotiated)
                }
            }?
        };

        let mut caps = video_info.to_caps().unwrap();
        let mut downstream_accepts_meta = false;

        let upstream_has_meta = caps
            .features(0)
            .map(|f| f.contains(gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION))
            .unwrap_or(false);

        if !upstream_has_meta {
            let mut caps_clone = caps.clone();
            let overlay_caps = caps_clone.make_mut();

            if let Some(features) = overlay_caps.features_mut(0) {
                features.add(gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION);
                let peercaps = self.srcpad.peer_query_caps(Some(&caps_clone));
                downstream_accepts_meta = !peercaps.is_empty();
                if downstream_accepts_meta {
                    caps = caps_clone;
                }
            }
        }

        gst::debug!(
            CAT,
            imp = self,
            "upstream has meta: {}, downstream accepts meta: {}",
            upstream_has_meta,
            downstream_accepts_meta
        );

        if upstream_has_meta || downstream_accepts_meta {
            let mut query = gst::query::Allocation::new(Some(&caps), false);

            if !self.srcpad.push_event(gst::event::Caps::new(&caps)) {
                return Err(gst::FlowError::NotNegotiated);
            }

            if !self.srcpad.peer_query(&mut query)
                && self.srcpad.pad_flags().contains(gst::PadFlags::FLUSHING)
            {
                return Err(gst::FlowError::NotNegotiated);
            }

            let attach = query
                .find_allocation_meta::<gst_video::VideoOverlayCompositionMeta>()
                .is_some();

            gst::debug!(CAT, imp = self, "attach meta: {}", attach);

            self.state.lock().unwrap().attach = attach;

            Ok(gst::FlowSuccess::Ok)
        } else {
            self.state.lock().unwrap().attach = false;

            if !self.srcpad.push_event(gst::event::Caps::new(&caps)) {
                Err(gst::FlowError::NotNegotiated)
            } else {
                Ok(gst::FlowSuccess::Ok)
            }
        }
    }

    fn render_shape_buffer(
        &self,
        state: &mut State,
        width: u32,
        height: u32,
        points: &[Point],
        tag: Option<&str>,
    ) -> Option<(gst::Buffer, u32, u32)> {
        let mut text_width = 0;
        let mut text_height = 0;

        // If we have text to render, update the layout first in order to compute
        // the final size
        let layout = tag.and_then(|tag| {
            state.layout.as_ref().unwrap().set_text(tag);

            state.layout.clone()
        });

        if let Some(ref layout) = layout {
            let (_ink_rect, logical_rect) = layout.extents();

            text_width = logical_rect.width() / pango::SCALE;
            text_height = logical_rect.height() / pango::SCALE;
        }

        let total_height = height.max(text_height as u32);
        let total_width = width.max(text_width as u32);

        let mut buffer = gst::Buffer::with_size((total_width * total_height) as usize * 4).ok()?;

        gst_video::VideoMeta::add(
            buffer.get_mut().unwrap(),
            gst_video::VideoFrameFlags::empty(),
            #[cfg(target_endian = "little")]
            gst_video::VideoFormat::Bgra,
            #[cfg(target_endian = "big")]
            gst_video::VideoFormat::Argb,
            total_width,
            total_height,
        )
        .ok()?;

        let buffer = buffer.into_mapped_buffer_writable().unwrap();

        // Pass ownership of the buffer to the cairo surface but keep around
        // a raw pointer so we can later retrieve it again when the surface
        // is done
        let buffer_ptr = buffer.buffer().as_ptr();
        let surface = cairo::ImageSurface::create_for_data(
            buffer,
            cairo::Format::ARgb32,
            total_width as i32,
            total_height as i32,
            total_width as i32 * 4,
        )
        .ok()?;

        let cr = cairo::Context::new(&surface).ok()?;
        let line_width = 1.;

        // Clear background
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().ok()?;

        if points.is_empty() {
            // Render bounding box
            cr.save().ok()?;

            cr.move_to(line_width, line_width);
            cr.line_to(line_width, height as f64 - line_width);
            cr.line_to(width as f64 - line_width, height as f64 - line_width);
            cr.line_to(width as f64 - line_width, line_width);
            cr.close_path();
            cr.set_source_rgba(1., 0., 0., 1.);
            cr.set_line_width(line_width);
            let _ = cr.stroke();

            cr.restore().ok()?;
        } else {
            // Render polygon
            cr.save().ok()?;

            cr.move_to(points[0].x as f64, points[0].y as f64);

            for point in &points[1..] {
                cr.line_to(point.x as f64, point.y as f64)
            }

            cr.close_path();
            cr.set_source_rgba(1., 0., 0., 1.);
            cr.set_line_width(line_width);
            let _ = cr.stroke();

            cr.restore().ok()?;
        }

        // Finally render the text, if any
        if let Some(layout) = layout {
            cr.save().ok()?;

            cr.move_to(0., 0.);
            cr.set_operator(cairo::Operator::Over);
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            pangocairo::functions::layout_path(&cr, &layout);
            cr.stroke().ok()?;

            cr.restore().ok()?;
            cr.save().ok()?;

            cr.move_to(0., 0.);
            cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
            pangocairo::functions::show_layout(&cr, &layout);

            cr.restore().ok()?;
        }

        drop(cr);

        // Safety: The surface still owns a mutable reference to the buffer but our reference
        // to the surface here is the last one. After dropping the surface the buffer would be
        // freed, so we keep an additional strong reference here before dropping the surface,
        // which is then returned. As such it's guaranteed that nothing is using the buffer
        // anymore mutably.
        unsafe {
            assert_eq!(
                cairo::ffi::cairo_surface_get_reference_count(surface.to_raw_none()),
                1
            );
            let buffer = glib::translate::from_glib_none(buffer_ptr);
            drop(surface);
            Some((buffer, total_width, total_height))
        }
    }

    // Update our overlay composition with a set of rectangles
    fn overlay_shapes(&self, state: &mut State, shapes: Vec<Shape>) {
        if shapes.is_empty() {
            state.composition = None;
            return;
        }

        if state.layout.is_none() {
            let fontmap = pangocairo::FontMap::new();
            let context = fontmap.create_context();
            context.set_language(Some(&pango::Language::from_string("en_US")));
            context.set_base_dir(pango::Direction::Ltr);
            let layout = pango::Layout::new(&context);
            layout.set_alignment(pango::Alignment::Left);
            let font_desc =
                pango::FontDescription::from_string(&self.settings.lock().unwrap().font_desc);
            layout.set_font_description(Some(&font_desc));

            state.layout = Some(layout);
        }

        let mut composition = gst_video::VideoOverlayComposition::default();
        let composition_mut = composition.get_mut().unwrap();
        for shape in &shapes {
            // Sanity check: don't render 0-sized shapes
            if shape.width == 0 || shape.height == 0 {
                continue;
            }

            gst::debug!(
                CAT,
                imp = self,
                "Rendering shape with tag {:?} x {} y {} width {} height {}",
                shape.tag,
                shape.x,
                shape.y,
                shape.width,
                shape.height
            );

            let (buffer, width, height) = match self.render_shape_buffer(
                state,
                shape.width,
                shape.height,
                &shape.points,
                shape.tag.as_deref(),
            ) {
                Some(ret) => ret,
                None => {
                    gst::error!(CAT, imp = self, "Failed to render buffer");
                    state.composition = None;
                    return;
                }
            };

            let rect = gst_video::VideoOverlayRectangle::new_raw(
                &buffer,
                shape.x as i32,
                shape.y as i32,
                width,
                height,
                gst_video::VideoOverlayFormatFlags::PREMULTIPLIED_ALPHA,
            );

            composition_mut.add_rectangle(&rect);
        }

        state.composition = Some(composition);
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        if self.srcpad.check_reconfigure() {
            if let Err(err) = self.negotiate() {
                if self.srcpad.pad_flags().contains(gst::PadFlags::FLUSHING) {
                    self.srcpad.mark_reconfigure();
                    return Ok(gst::FlowSuccess::Ok);
                } else {
                    return Err(err);
                }
            }
        }

        let mut state = self.state.lock().unwrap();

        let video_info = state.video_info.as_ref().unwrap();
        let width = video_info.width() as i32;
        let height = video_info.height() as i32;

        if let Ok(meta) = gst::meta::CustomMeta::from_buffer(&buffer, "OnvifXMLFrameMeta") {
            let s = meta.structure();
            let mut shapes: Vec<Shape> = Vec::new();

            if let Ok(frames) = s.get::<gst::BufferList>("frames") {
                gst::log!(CAT, imp = self, "Overlaying {} frames", frames.len());

                // Metadata for multiple frames may be attached to this frame, either because:
                //
                // * Multiple analytics modules are producing metadata
                // * The metadata for two frames produced by the same module is attached
                //   to this frame, for instance because of resynchronization or other
                //   timing-related situations
                //
                // We want to display all detected objects for the first case, but only the
                // latest version for the second case. As frames are sorted in increasing temporal
                // order, we iterate them in reverse to start with the most recent, and deduplicate
                // by object id.

                let mut object_ids = HashSet::new();

                // Default values for translation and scaling
                let mut x_translate: f64 = 0.0;
                let mut y_translate: f64 = 0.0;
                let mut x_scale: f64 = 1.0;
                let mut y_scale: f64 = 1.0;

                for buffer in frames.iter().rev() {
                    let buffer = buffer.map_readable().map_err(|_| {
                        gst::element_imp_error!(
                            self,
                            gst::ResourceError::Read,
                            ["Failed to map buffer readable"]
                        );

                        gst::FlowError::Error
                    })?;

                    let utf8 = std::str::from_utf8(buffer.as_ref()).map_err(|err| {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["Failed to decode buffer as UTF-8: {}", err]
                        );

                        gst::FlowError::Error
                    })?;

                    let root =
                        xmltree::Element::parse(std::io::Cursor::new(utf8)).map_err(|err| {
                            gst::element_imp_error!(
                                self,
                                gst::ResourceError::Read,
                                ["Failed to parse buffer as XML: {}", err]
                            );

                            gst::FlowError::Error
                        })?;

                    for object in root
                        .get_child(("VideoAnalytics", crate::ONVIF_METADATA_SCHEMA))
                        .map(|e| e.children.iter().filter_map(|n| n.as_element()))
                        .into_iter()
                        .flatten()
                    {
                        if object.name == "Frame"
                            && object.namespace.as_deref() == Some(crate::ONVIF_METADATA_SCHEMA)
                        {
                            for transformation in object
                                .children
                                .iter()
                                .filter_map(|n| n.as_element())
                                .filter(|e| {
                                    e.name == "Transformation"
                                        && e.namespace.as_deref()
                                            == Some(crate::ONVIF_METADATA_SCHEMA)
                                })
                            {
                                gst::trace!(
                                    CAT,
                                    imp = self,
                                    "Handling transformation {:?}",
                                    transformation
                                );

                                let translate = match transformation
                                    .get_child(("Translate", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(translate) => translate,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "Transform with no Translate node"
                                        );
                                        continue;
                                    }
                                };

                                x_translate = match translate
                                    .attributes
                                    .get("x")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "Translate with no x attribute"
                                        );
                                        continue;
                                    }
                                };

                                y_translate = match translate
                                    .attributes
                                    .get("y")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "Translate with no y attribute"
                                        );
                                        continue;
                                    }
                                };

                                let scale = match transformation
                                    .get_child(("Scale", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(translate) => translate,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "Transform with no Scale node"
                                        );
                                        continue;
                                    }
                                };

                                x_scale = match scale
                                    .attributes
                                    .get("x")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(CAT, imp = self, "Scale with no x attribute");
                                        continue;
                                    }
                                };

                                y_scale = match scale
                                    .attributes
                                    .get("y")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(CAT, imp = self, "Scale with no y attribute");
                                        continue;
                                    }
                                };
                            }

                            for object in object
                                .children
                                .iter()
                                .filter_map(|n| n.as_element())
                                .filter(|e| {
                                    e.name == "Object"
                                        && e.namespace.as_deref()
                                            == Some(crate::ONVIF_METADATA_SCHEMA)
                                })
                            {
                                gst::trace!(CAT, imp = self, "Handling object {:?}", object);

                                let object_id = match object.attributes.get("ObjectId") {
                                    Some(id) => id.to_string(),
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "XML Object with no ObjectId"
                                        );
                                        continue;
                                    }
                                };

                                if !object_ids.insert(object_id.clone()) {
                                    gst::debug!(
                                        CAT,
                                        "Skipping older version of object {}",
                                        object_id
                                    );
                                    continue;
                                }

                                let appearance = match object
                                    .get_child(("Appearance", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(appearance) => appearance,
                                    None => continue,
                                };

                                let shape = match appearance
                                    .get_child(("Shape", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(shape) => shape,
                                    None => continue,
                                };

                                let tag = appearance
                                    .get_child(("Class", crate::ONVIF_METADATA_SCHEMA))
                                    .and_then(|class| {
                                        class.get_child(("Type", crate::ONVIF_METADATA_SCHEMA))
                                    })
                                    .and_then(|t| t.get_text())
                                    .map(|t| t.into_owned());

                                let bbox = match shape
                                    .get_child(("BoundingBox", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(bbox) => bbox,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "XML Shape with no BoundingBox"
                                        );
                                        continue;
                                    }
                                };

                                let left: f64 = match bbox
                                    .attributes
                                    .get("left")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "BoundingBox with no left attribute"
                                        );
                                        continue;
                                    }
                                };

                                let right: f64 = match bbox
                                    .attributes
                                    .get("right")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "BoundingBox with no right attribute"
                                        );
                                        continue;
                                    }
                                };

                                let top: f64 = match bbox
                                    .attributes
                                    .get("top")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "BoundingBox with no top attribute"
                                        );
                                        continue;
                                    }
                                };

                                let bottom: f64 = match bbox
                                    .attributes
                                    .get("bottom")
                                    .and_then(|val| val.parse().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "BoundingBox with no bottom attribute"
                                        );
                                        continue;
                                    }
                                };

                                let x1 = ((1.0 + x_translate) * width as f64 / 2.0) as i32
                                    + ((left * x_scale * (width / 2) as f64) as i32);
                                let x2 = ((1.0 + x_translate) * width as f64 / 2.0) as i32
                                    + ((right * x_scale * (width / 2) as f64) as i32);
                                let y1 = ((1.0 + y_translate) * height as f64 / 2.0) as i32
                                    + ((top * y_scale * (height / 2) as f64) as i32);
                                let y2 = ((1.0 + y_translate) * height as f64 / 2.0) as i32
                                    + ((bottom * y_scale * (height / 2) as f64) as i32);

                                let w = (x2 - x1) as u32;
                                let h = (y2 - y1) as u32;

                                let mut points = vec![];

                                if let Some(polygon) =
                                    shape.get_child(("Polygon", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    for point in
                                        polygon.children.iter().filter_map(|n| n.as_element())
                                    {
                                        if point.name == "Point"
                                            && point.namespace.as_deref()
                                                == Some(crate::ONVIF_METADATA_SCHEMA)
                                        {
                                            let px: f64 = match point
                                                .attributes
                                                .get("x")
                                                .and_then(|val| val.parse().ok())
                                            {
                                                Some(val) => val,
                                                None => {
                                                    gst::warning!(
                                                        CAT,
                                                        imp = self,
                                                        "Point with no x attribute"
                                                    );
                                                    continue;
                                                }
                                            };

                                            let py: f64 = match point
                                                .attributes
                                                .get("y")
                                                .and_then(|val| val.parse().ok())
                                            {
                                                Some(val) => val,
                                                None => {
                                                    gst::warning!(
                                                        CAT,
                                                        imp = self,
                                                        "Point with no y attribute"
                                                    );
                                                    continue;
                                                }
                                            };

                                            let px = width / 2 + ((px * (width / 2) as f64) as i32);
                                            let px = (px as u32).saturating_sub(x1 as u32).min(w);

                                            let py =
                                                height / 2 - ((py * (height / 2) as f64) as i32);
                                            let py = (py as u32).saturating_sub(y1 as u32).min(h);

                                            points.push(Point { x: px, y: py });
                                        }
                                    }
                                }

                                shapes.push(Shape {
                                    x: x1 as u32,
                                    y: y1 as u32,
                                    width: w,
                                    height: h,
                                    points,
                                    tag,
                                });
                            }
                        }
                    }
                }

                if !frames.is_empty() {
                    self.overlay_shapes(&mut state, shapes);
                }
            }
        }

        if let Some(composition) = &state.composition {
            let buffer = buffer.make_mut();
            if state.attach {
                gst_video::VideoOverlayCompositionMeta::add(buffer, composition);
            } else {
                let mut frame = gst_video::VideoFrameRef::from_buffer_ref_writable(
                    buffer,
                    state.video_info.as_ref().unwrap(),
                )
                .unwrap();

                if composition.blend(&mut frame).is_err() {
                    gst::error!(CAT, obj = pad, "Failed to blend composition");
                }
            }
        }
        drop(state);

        self.srcpad.push(buffer)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(c) => {
                let mut state = self.state.lock().unwrap();
                state.video_info = gst_video::VideoInfo::from_caps(c.caps()).ok();
                drop(state);
                self.srcpad.check_reconfigure();
                match self.negotiate() {
                    Ok(_) => true,
                    Err(_) => {
                        self.srcpad.mark_reconfigure();
                        true
                    }
                }
            }
            EventView::FlushStop(..) => {
                let mut state = self.state.lock().unwrap();
                state.composition = None;
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for OnvifMetadataOverlay {
    const NAME: &'static str = "GstOnvifMetadataOverlay";
    type Type = super::OnvifMetadataOverlay;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                OnvifMetadataOverlay::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |overlay| overlay.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                OnvifMetadataOverlay::catch_panic_pad_function(
                    parent,
                    || false,
                    |overlay| overlay.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::PROXY_CAPS)
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for OnvifMetadataOverlay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("font-desc")
                .nick("Font Description")
                .blurb("Pango font description of font to be used for rendering")
                .default_value(Some(DEFAULT_FONT_DESC))
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "font-desc" => {
                self.settings.lock().unwrap().font_desc = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_FONT_DESC.into());
                self.state.lock().unwrap().layout.take();
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "font-desc" => self.settings.lock().unwrap().font_desc.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for OnvifMetadataOverlay {}

impl ElementImpl for OnvifMetadataOverlay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIF Metadata overlay",
                "Video/Overlay",
                "Renders ONVIF analytics meta over raw video frames",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoFormat::iter_raw()
                .into_video_caps()
                .unwrap()
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

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused | gst::StateChange::PausedToReady => {
                // Reset the whole state
                let mut state = self.state.lock().unwrap();
                *state = State::default();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
