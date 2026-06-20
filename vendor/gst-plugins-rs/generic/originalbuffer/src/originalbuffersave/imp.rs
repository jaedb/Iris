// Copyright (C) 2024 Collabora Ltd
//   @author: Olivier Crête <olivier.crete@collabora.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use crate::originalbuffermeta::OriginalBufferMeta;

pub struct OriginalBufferSave {
    src_pad: gst::Pad,
    sink_pad: gst::Pad,
}

use std::sync::LazyLock;
#[allow(dead_code)]
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "originalbuffersave",
        gst::DebugColorFlags::empty(),
        Some("Save Original buffer as meta"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for OriginalBufferSave {
    const NAME: &'static str = "GstOriginalBufferSave";
    type Type = super::OriginalBufferSave;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let sink_templ = klass.pad_template("sink").unwrap();
        let src_templ = klass.pad_template("src").unwrap();

        let sink_pad = gst::Pad::builder_from_template(&sink_templ)
            .chain_function(|pad, parent, buffer| {
                OriginalBufferSave::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |obj| obj.sink_chain(pad, buffer),
                )
            })
            .query_function(|pad, parent, query| {
                OriginalBufferSave::catch_panic_pad_function(
                    parent,
                    || false,
                    |obj| obj.sink_query(pad, parent, query),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::PROXY_ALLOCATION)
            .build();

        let src_pad = gst::Pad::builder_from_template(&src_templ)
            .event_function(|pad, parent, event| {
                OriginalBufferSave::catch_panic_pad_function(
                    parent,
                    || false,
                    |obj| obj.src_event(pad, parent, event),
                )
            })
            .build();

        Self { src_pad, sink_pad }
    }
}

impl ObjectImpl for OriginalBufferSave {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sink_pad).unwrap();
        obj.add_pad(&self.src_pad).unwrap();
    }
}

impl GstObjectImpl for OriginalBufferSave {}

impl ElementImpl for OriginalBufferSave {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Original Buffer Save",
                "Generic",
                "Saves a reference to the buffer in a meta",
                "Olivier Crête <olivier.crete@collabora.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
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

impl OriginalBufferSave {
    fn forward_query(&self, query: gst::Query) -> Option<gst::Query> {
        let mut s = gst::Structure::new_empty("gst-original-buffer-forward-query");
        s.set("query", query);

        let mut query = gst::query::Custom::new(s);
        if self.src_pad.peer_query(&mut query) {
            let s = query.structure_mut();
            if let (Ok(true), Ok(q)) = (s.get("result"), s.get::<gst::Query>("query")) {
                Some(q)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        inbuf: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut buf = inbuf.copy();
        let caps = pad.current_caps();

        if let Some(mut meta) = buf.make_mut().meta_mut::<OriginalBufferMeta>() {
            meta.replace(inbuf, caps);
        } else {
            OriginalBufferMeta::add(buf.make_mut(), inbuf, caps);
        }

        self.src_pad.push(buf)
    }

    fn sink_query(
        &self,
        pad: &gst::Pad,
        parent: Option<&impl IsA<gst::Object>>,
        query: &mut gst::QueryRef,
    ) -> bool {
        let ret = gst::Pad::query_default(pad, parent, query);
        if !ret {
            return ret;
        }

        if let gst::QueryViewMut::Caps(q) = query.view_mut() {
            if let Some(caps) = q.result_owned() {
                let forwarding_q = gst::query::Caps::new(Some(&caps)).into();

                if let Some(forwarding_q) = self.forward_query(forwarding_q) {
                    if let gst::QueryView::Caps(c) = forwarding_q.view() {
                        let res = c
                            .result_owned()
                            .map(|c| c.intersect_with_mode(&caps, gst::CapsIntersectMode::First));
                        q.set_result(&res);
                    }
                }
            }
        }

        // We should also do allocation queries, but that requires supporting the same
        // intersection semantics as gsttee, which should be in a helper function.

        true
    }

    fn src_event(
        &self,
        pad: &gst::Pad,
        parent: Option<&impl IsA<gst::Object>>,
        event: gst::Event,
    ) -> bool {
        let event = if event.has_name("gst-original-buffer-forward-upstream-event") {
            event.structure().unwrap().get("event").unwrap()
        } else {
            event
        };

        gst::Pad::event_default(pad, parent, event)
    }
}
