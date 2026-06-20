// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::ops::ControlFlow;
use std::sync::LazyLock;
use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "onvifmetadataextractor",
        gst::DebugColorFlags::empty(),
        Some("ONVIF Metadata Extractor Element"),
    )
});

#[derive(Default)]
struct Settings {
    remove_metadata: bool,
}

pub struct OnvifMetadataExtractor {
    // Input media stream
    sink_pad: gst::Pad,
    // Output media stream
    src_pad: gst::Pad,
    // Output metadata stream, complete VideoAnalytics XML documents
    meta_src_pad: gst::Pad,
    settings: Mutex<Settings>,
    flow_combiner: Mutex<gst_base::UniqueFlowCombiner>,
}

#[glib::object_subclass]
impl ObjectSubclass for OnvifMetadataExtractor {
    const NAME: &'static str = "GstOnvifMetadataExtractor";
    type Type = super::OnvifMetadataExtractor;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sink_pad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                OnvifMetadataExtractor::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |extractor| extractor.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                OnvifMetadataExtractor::catch_panic_pad_function(
                    parent,
                    || false,
                    |extractor| extractor.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                Self::catch_panic_pad_function(
                    parent,
                    || false,
                    |extractor| extractor.sink_query(pad, query),
                )
            })
            .build();
        let templ = klass.pad_template("src").unwrap();
        let src_pad = gst::Pad::builder_from_template(&templ).build();
        let templ = klass.pad_template("meta_src").unwrap();
        let meta_src_pad = gst::Pad::builder_from_template(&templ).build();

        Self {
            sink_pad,
            src_pad,
            meta_src_pad,
            settings: Mutex::new(Settings::default()),
            flow_combiner: Mutex::new(gst_base::UniqueFlowCombiner::new()),
        }
    }
}

impl ObjectImpl for OnvifMetadataExtractor {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sink_pad).unwrap();
        obj.add_pad(&self.src_pad).unwrap();
        obj.add_pad(&self.meta_src_pad).unwrap();
        self.flow_combiner.lock().unwrap().add_pad(&self.src_pad);
        self.flow_combiner
            .lock()
            .unwrap()
            .add_pad(&self.meta_src_pad);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("remove-onvif-metadata")
                .nick("Remove ONVIF metadata")
                .blurb("Remove ONVIF metadata from output stream")
                .default_value(false)
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "remove-onvif-metadata" => {
                self.settings.lock().unwrap().remove_metadata = value.get().unwrap();
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "remove-onvif-metadata" => self.settings.lock().unwrap().remove_metadata.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for OnvifMetadataExtractor {}

impl ElementImpl for OnvifMetadataExtractor {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIF metadata extractor",
                "Video/Metadata",
                "Extract the ONVIF GstMeta into a separate stream",
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

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let meta_caps = gst::Caps::builder("application/x-onvif-metadata").build();
            let meta_src_pad_template = gst::PadTemplate::new(
                "meta_src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &meta_caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template, meta_src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl OnvifMetadataExtractor {
    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::trace!(CAT, obj = pad, "Received query: {query:?}");
        gst::Pad::query_default(&self.sink_pad, Some(&*self.obj()), query)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(..) => {
                self.src_pad.push_event(event);
                let caps = &self.meta_src_pad.pad_template_caps();
                self.meta_src_pad.push_event(gst::event::Caps::new(caps))
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let remove = self.settings.lock().unwrap().remove_metadata;

        let pts = buffer.pts();
        let dts = buffer.dts();

        if let Ok(metas) =
            gst::meta::CustomMeta::from_mut_buffer(buffer.make_mut(), "OnvifXMLFrameMeta")
        {
            let s = metas.structure();

            if let Ok(frames) = s.get::<gst::BufferList>("frames") {
                frames.foreach(|meta_buf, _idx| {
                    let mut buffer = meta_buf.clone();
                    {
                        let buffer = buffer.make_mut();
                        buffer.set_dts(dts);
                        buffer.set_pts(pts);
                    }

                    let res = self.meta_src_pad.push(buffer);
                    let _ = self
                        .flow_combiner
                        .lock()
                        .unwrap()
                        .update_pad_flow(&self.meta_src_pad, res);
                    if res.is_ok() {
                        ControlFlow::Continue(())
                    } else {
                        ControlFlow::Break(())
                    }
                });
            }

            if remove && metas.remove().is_err() {
                return Err(gst::FlowError::Error);
            }
        }

        let res = self.src_pad.push(buffer);
        self.flow_combiner
            .lock()
            .unwrap()
            .update_pad_flow(&self.src_pad, res)
    }
}
