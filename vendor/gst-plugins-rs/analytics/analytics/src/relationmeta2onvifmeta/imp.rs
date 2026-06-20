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
use std::sync::LazyLock;

use std::sync::Mutex;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRsOnvifNtpTimeSource")]
pub enum TimeSource {
    #[enum_value(name = "UNIX time based on realtime clock.", nick = "clock")]
    Clock,
    #[enum_value(name = "Running time is in UTC", nick = "running-time")]
    RunningTime,
    #[enum_value(name = "Pipeline clock is UTC", nick = "clock-time")]
    ClockTime,
}

const DEFAULT_TIME_SOURCE: TimeSource = TimeSource::Clock;

#[derive(Debug, Clone, Copy)]
struct Settings {
    timesource: TimeSource,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            timesource: DEFAULT_TIME_SOURCE,
        }
    }
}

#[derive(Default)]
struct State {
    video_info: Option<gst_video::VideoInfo>,
    segment: gst::FormattedSegment<gst::ClockTime>,
}

pub struct RelationMeta2OnvifMeta {
    // Input media stream with relation metadata
    sinkpad: gst::Pad,
    // Output media stream with ONVIF metadata
    srcpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "relationmeta2onvifmeta",
        gst::DebugColorFlags::empty(),
        Some("Relation metadata to ONVIF metadata"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RelationMeta2OnvifMeta {
    const NAME: &'static str = "GstRelationMeta2OnvifMeta";
    type Type = super::RelationMeta2OnvifMeta;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                RelationMeta2OnvifMeta::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |convert| convert.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                RelationMeta2OnvifMeta::catch_panic_pad_function(
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
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for RelationMeta2OnvifMeta {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default("time-source", DEFAULT_TIME_SOURCE)
                    .nick("time source")
                    .blurb("Time source for UTC timestamps")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "time-source" => {
                let mut settings = self.settings.lock().unwrap();
                settings.timesource = value.get::<TimeSource>().expect("type checked upstream");
            }
            _ => unreachable!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "time-source" => {
                let settings = self.settings.lock().unwrap();
                settings.timesource.to_value()
            }
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

impl GstObjectImpl for RelationMeta2OnvifMeta {}

impl ElementImpl for RelationMeta2OnvifMeta {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Relation metadata to ONVIF metadata",
                "Metadata",
                "Convert relation metadata to ONVIF metadata",
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

impl RelationMeta2OnvifMeta {
    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        match event.view() {
            EventView::Caps(c) => {
                let mut state = self.state.lock().unwrap();
                let info = match gst_video::VideoInfo::from_caps(c.caps()) {
                    Ok(info) => info,
                    Err(_) => {
                        gst::error!(CAT, obj = pad, "Failed to parse caps {:?}", c.caps());
                        return false;
                    }
                };
                state.video_info = Some(info);
                self.srcpad.push_event(event)
            }
            EventView::Segment(s) => {
                let mut state = self.state.lock().unwrap();
                let segment = s.segment().clone();
                let segment = match segment.downcast::<gst::ClockTime>() {
                    Ok(segment) => segment,
                    Err(_) => {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Event,
                            ["Only time segments are supported"]
                        );
                        return false;
                    }
                };

                state.segment = segment.clone();
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn get_utc_time(&self, running_time: gst::ClockTime) -> gst::ClockTime {
        match self.settings.lock().unwrap().timesource {
            TimeSource::Clock => {
                if let (Some(base_time), Some(clock)) = (self.obj().base_time(), self.obj().clock())
                {
                    let utc_now = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                        .try_into()
                        .unwrap();
                    let utc_now = gst::ClockTime::from_nseconds(utc_now);
                    let running_time_now = clock.time() - base_time;

                    let rt_diff = utc_now - running_time_now;
                    running_time_now + rt_diff
                } else {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Can only use the clock if the pipeline has a clock"
                    );
                    gst::ClockTime::ZERO
                }
            }
            TimeSource::RunningTime => running_time,
            TimeSource::ClockTime => {
                if let Some(base_time) = self.obj().base_time() {
                    running_time + base_time
                } else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Clock time was selected but there is no base time"
                    );
                    gst::ClockTime::ZERO
                }
            }
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut input_buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let metas = input_buffer.meta::<AnalyticsRelationMeta>();

        let mut xml = xmltree::Element::new("MetadataStream");
        xml.namespaces
            .get_or_insert_with(|| xmltree::Namespace(Default::default()))
            .put("tt", crate::ONVIF_METADATA_SCHEMA);
        xml.namespace = Some(String::from(crate::ONVIF_METADATA_SCHEMA));
        xml.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

        let mut video_analytics = xmltree::Element::new("VideoAnalytics");
        video_analytics.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
        let state = self.state.lock().unwrap();

        let current_time =
            self.get_utc_time(state.segment.to_running_time(input_buffer.pts()).unwrap());
        let dt =
            gst::DateTime::from_unix_epoch_utc_usecs(current_time.useconds().try_into().unwrap())
                .unwrap()
                .to_g_date_time()
                .unwrap()
                .format_iso8601()
                .unwrap();

        let mut frame = xmltree::Element::new("Frame");
        frame.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
        frame
            .attributes
            .insert("UtcTime".to_string(), dt.to_string());

        let mut transformation = xmltree::Element::new("Transformation");
        transformation.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

        let mut translate = xmltree::Element::new("Translate");
        translate.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
        translate
            .attributes
            .insert("x".to_string(), (-1.0).to_string());
        translate
            .attributes
            .insert("y".to_string(), (-1.0).to_string());

        let mut scale = xmltree::Element::new("Scale");
        scale.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

        let video_info = state.video_info.as_ref().unwrap();
        let x = format!("{:.5}", 2_f64 / (video_info.width() as f64));
        let y = format!("{:.5}", 2_f64 / (video_info.height() as f64));
        scale.attributes.insert("x".to_string(), x);
        scale.attributes.insert("y".to_string(), y);

        transformation
            .children
            .push(xmltree::XMLNode::Element(translate));
        transformation
            .children
            .push(xmltree::XMLNode::Element(scale));

        frame
            .children
            .push(xmltree::XMLNode::Element(transformation));

        if let Some(metas) = metas {
            for meta in metas.iter::<AnalyticsODMtd>() {
                let loc = meta.location().unwrap();

                let mut object = xmltree::Element::new("Object");
                object.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
                object
                    .attributes
                    .insert("ObjectId".to_string(), meta.id().to_string());

                let mut appearance = xmltree::Element::new("Appearance");
                appearance.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

                let mut shape = xmltree::Element::new("Shape");
                shape.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

                let mut bounding_box = xmltree::Element::new("BoundingBox");
                bounding_box.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
                bounding_box
                    .attributes
                    .insert("left".to_string(), loc.x.to_string());
                bounding_box
                    .attributes
                    .insert("top".to_string(), loc.y.to_string());
                bounding_box
                    .attributes
                    .insert("right".to_string(), (loc.x + loc.w).to_string());
                bounding_box
                    .attributes
                    .insert("bottom".to_string(), (loc.y + loc.h).to_string());
                shape.children.push(xmltree::XMLNode::Element(bounding_box));

                let mut class = xmltree::Element::new("Class");
                class.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));

                let mut t = xmltree::Element::new("Type");
                t.prefix = Some(String::from(crate::ONVIF_METADATA_PREFIX));
                t.attributes
                    .insert("Likelihood".to_string(), loc.loc_conf_lvl.to_string());
                let obj_name = if let Some(obj_type) = meta.obj_type() {
                    obj_type.as_str()
                } else {
                    "Unknown"
                };
                t.children
                    .push(xmltree::XMLNode::Text(obj_name.to_string()));

                gst::trace!(
                    CAT,
                    imp = self,
                    "Transformed object: {}x{}@({}h{})  prob: {} name: {:?}",
                    loc.w,
                    loc.w,
                    loc.x,
                    loc.y,
                    loc.loc_conf_lvl,
                    meta.obj_type()
                );

                class.children.push(xmltree::XMLNode::Element(t));

                appearance.children.push(xmltree::XMLNode::Element(shape));
                appearance.children.push(xmltree::XMLNode::Element(class));

                object.children.push(xmltree::XMLNode::Element(appearance));

                frame.children.push(xmltree::XMLNode::Element(object));
            }
        } else {
            gst::trace!(CAT, imp = self, "No meta, nothing to transform");
        }

        video_analytics
            .children
            .push(xmltree::XMLNode::Element(frame));
        xml.children
            .push(xmltree::XMLNode::Element(video_analytics));

        let mut text = Vec::new();
        if let Err(err) = xml.write_with_config(
            &mut text,
            xmltree::EmitterConfig {
                perform_indent: true,
                ..xmltree::EmitterConfig::default()
            },
        ) {
            gst::error!(CAT, obj = pad, "Can't serialize XML element: {}", err);
        }

        gst::trace!(
            CAT,
            imp = self,
            "Generated ONVIF xml: {}",
            std::str::from_utf8(&text).unwrap()
        );

        let meta_buf = gst::Buffer::from_mut_slice(text);
        let mut buflist = gst::BufferList::new();
        buflist.get_mut().unwrap().add(meta_buf);

        let buf = input_buffer.make_mut();
        let mut onvif_meta = gst::meta::CustomMeta::add(buf, "OnvifXMLFrameMeta").unwrap();
        let s = onvif_meta.mut_structure();
        s.set("frames", buflist);

        gst::trace!(CAT, obj = pad, "Send buffer {:?}", buf);
        self.srcpad.push(input_buffer)
    }
}
