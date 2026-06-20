// Copyright (C) 2024 Benjamin Gaignard <benjamin.gaignard@collabora.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_analytics::*;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Default)]
struct State {
    video_info: Option<gst_video::VideoInfo>,
}

pub struct OnvifMeta2RelationMeta {
    // Input media stream with ONVIF metadata
    sinkpad: gst::Pad,
    // Output media stream with relation metadata
    srcpad: gst::Pad,
    state: Mutex<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "onvifmeta2relationmeta",
        gst::DebugColorFlags::empty(),
        Some("ONVIF metadata to Relation meta"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for OnvifMeta2RelationMeta {
    const NAME: &'static str = "GstOnvifMeta2RelationMeta";
    type Type = super::OnvifMeta2RelationMeta;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                OnvifMeta2RelationMeta::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |convert| convert.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                OnvifMeta2RelationMeta::catch_panic_pad_function(
                    parent,
                    || false,
                    |convert| convert.sink_event(pad, event),
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
        }
    }
}

impl ObjectImpl for OnvifMeta2RelationMeta {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for OnvifMeta2RelationMeta {}

impl ElementImpl for OnvifMeta2RelationMeta {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIF metadata to relation metadata",
                "Metadata",
                "Convert ONVIF metadata to relation metadata",
                "Benjamin Gaignard <benjamin.gaignard@collabora.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::new_any();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl OnvifMeta2RelationMeta {
    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(c) => {
                let mut state = self.state.lock().unwrap();

                state.video_info = match gst_video::VideoInfo::from_caps(c.caps()) {
                    Err(_) => return false,
                    Ok(info) => Some(info),
                };
                drop(state);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut input_buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let buf = input_buffer.make_mut();
        let state = self.state.lock().unwrap();

        let video_info = state
            .video_info
            .as_ref()
            .ok_or(gst::FlowError::NotNegotiated)?;
        let width = video_info.width() as i32;
        let height = video_info.height() as i32;

        if let Ok(metas) = gst::meta::CustomMeta::from_buffer(buf, "OnvifXMLFrameMeta") {
            let s = metas.structure();

            // Default values for translation and scaling
            let mut x_translate = 0.0f64;
            let mut y_translate = 0.0f64;
            let mut x_scale = 1.0f64;
            let mut y_scale = 1.0f64;

            if let Ok(frames) = s.get::<gst::BufferList>("frames") {
                let mut object_ids = HashSet::new();

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
                                gst::StreamError::Decode,
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
                                gst::trace!(CAT, obj = pad, "Handling object {:?}", object);

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
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "XML Object with no Appearance"
                                        );
                                        continue;
                                    }
                                };

                                let shape = match appearance
                                    .get_child(("Shape", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(shape) => shape,
                                    None => {
                                        gst::warning!(CAT, imp = self, "XML Object with no Shape");
                                        continue;
                                    }
                                };

                                let class = match appearance
                                    .get_child(("Class", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(class) => class,
                                    None => {
                                        gst::warning!(CAT, imp = self, "XML Object with no Class");
                                        continue;
                                    }
                                };

                                let t = match class
                                    .get_child(("Type", crate::ONVIF_METADATA_SCHEMA))
                                {
                                    Some(t) => t,
                                    None => {
                                        gst::warning!(CAT, imp = self, "XML Class with no Type");
                                        continue;
                                    }
                                };

                                let likelihood = match t
                                    .attributes
                                    .get("Likelihood")
                                    .and_then(|val| val.parse::<f64>().ok())
                                {
                                    Some(val) => val,
                                    None => {
                                        gst::warning!(
                                            CAT,
                                            imp = self,
                                            "Type with no Likelihood attribute"
                                        );
                                        continue;
                                    }
                                };

                                let tag: String = match t.get_text() {
                                    Some(tag) => tag.to_string(),
                                    None => {
                                        gst::warning!(CAT, imp = self, "XML Type with no text");
                                        continue;
                                    }
                                };

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

                                let left = match bbox
                                    .attributes
                                    .get("left")
                                    .and_then(|val| val.parse::<f64>().ok())
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

                                let right = match bbox
                                    .attributes
                                    .get("right")
                                    .and_then(|val| val.parse::<f64>().ok())
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

                                let top = match bbox
                                    .attributes
                                    .get("top")
                                    .and_then(|val| val.parse::<f64>().ok())
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

                                let bottom = match bbox
                                    .attributes
                                    .get("bottom")
                                    .and_then(|val| val.parse::<f64>().ok())
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

                                gst::info!(
				    CAT,
				    imp = self,
				    "Object detected with label : {}, likelihood: {}, bounding box: {}x{} at ({},{})",
				    tag, likelihood, (x2 - x1), (y2 - y1), x1, y1);

                                let mut arm = AnalyticsRelationMeta::add(buf);
                                let quark = glib::Quark::from_str(tag);
                                let _ = arm.add_od_mtd(
                                    quark,
                                    x1 as i32,
                                    y1 as i32,
                                    (y2 - y1) as i32,
                                    (x2 - x1) as i32,
                                    likelihood as f32,
                                );
                                let _ = arm.add_one_cls_mtd(likelihood as f32, quark);
                            }
                        }
                    }
                }
            }
        }

        if let Ok(metas) = gst::meta::CustomMeta::from_mut_buffer(buf, "OnvifXMLFrameMeta") {
            metas.remove().unwrap();
        }

        self.srcpad.push(input_buffer)
    }
}
