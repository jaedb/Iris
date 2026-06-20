// SPDX-License-Identifier: MPL-2.0

use crate::streamproducer::InterStreamProducer;
use anyhow::Error;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_utils::streamproducer::{
    ConsumerSettings, DEFAULT_CONSUMER_MAX_BUFFERS, DEFAULT_CONSUMER_MAX_BYTES,
    DEFAULT_CONSUMER_MAX_TIME,
};

use std::sync::Mutex;

use std::sync::LazyLock;

const DEFAULT_PRODUCER_NAME: &str = "default";

#[derive(Debug)]
struct Settings {
    producer_name: String,
    consumer: ConsumerSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            producer_name: DEFAULT_PRODUCER_NAME.to_string(),
            consumer: ConsumerSettings::default(),
        }
    }
}

pub struct InterSrc {
    srcpad: gst::GhostPad,
    appsrc: gst_app::AppSrc,
    settings: Mutex<Settings>,
}

impl InterSrc {
    fn prepare(&self) -> Result<(), Error> {
        let settings = self.settings.lock().unwrap();

        InterStreamProducer::subscribe(&settings.producer_name, &self.appsrc, settings.consumer);

        Ok(())
    }

    fn unprepare(&self) {
        let settings = self.settings.lock().unwrap();

        InterStreamProducer::unsubscribe(&settings.producer_name, &self.appsrc);
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new("intersrc", gst::DebugColorFlags::empty(), Some("Inter Src"))
});

#[glib::object_subclass]
impl ObjectSubclass for InterSrc {
    const NAME: &'static str = "GstInterSrc";

    type Type = super::InterSrc;
    type ParentType = gst::Bin;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::GhostPad::from_template(&templ);

        Self {
            srcpad: srcpad.upcast(),
            appsrc: gst_app::AppSrc::builder().name("appsrc").build(),
            settings: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for InterSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("producer-name")
                    .nick("Producer Name")
                    .blurb("Producer Name to consume from")
                    .doc_show_default()
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("max-buffers")
                    .nick("Max Buffers")
                    .blurb("Maximum number of buffers to queue (0=unlimited)")
                    .default_value(DEFAULT_CONSUMER_MAX_BUFFERS)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("max-bytes")
                    .nick("Max Bytes")
                    .blurb("Maximum number of bytes to queue (0=unlimited)")
                    .default_value(DEFAULT_CONSUMER_MAX_BYTES.into())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("max-time")
                    .nick("Max Time")
                    .blurb("Maximum number of nanoseconds to queue (0=unlimited)")
                    // appsrc `max-time` property is an Int64 even though negative range is not used
                    .maximum(i64::MAX as u64)
                    .default_value(DEFAULT_CONSUMER_MAX_TIME.nseconds())
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "producer-name" => {
                let mut settings = self.settings.lock().unwrap();
                let old_producer_name = settings.producer_name.clone();
                settings.producer_name = value
                    .get::<String>()
                    .unwrap_or_else(|_| DEFAULT_PRODUCER_NAME.to_string());

                if InterStreamProducer::unsubscribe(&old_producer_name, &self.appsrc) {
                    InterStreamProducer::subscribe(
                        &settings.producer_name,
                        &self.appsrc,
                        settings.consumer,
                    );
                }
            }
            "max-buffers" => {
                self.settings.lock().unwrap().consumer.max_buffer = value.get::<u64>().unwrap();
            }
            "max-bytes" => {
                self.settings.lock().unwrap().consumer.max_bytes =
                    gst::format::Bytes::from_u64(value.get::<u64>().unwrap());
            }
            "max-time" => {
                self.settings.lock().unwrap().consumer.max_time =
                    gst::ClockTime::from_nseconds(value.get::<u64>().unwrap());
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "producer-name" => {
                let settings = self.settings.lock().unwrap();
                settings.producer_name.to_value()
            }
            "max-buffers" => self.settings.lock().unwrap().consumer.max_buffer.to_value(),
            "max-bytes" => self.settings.lock().unwrap().consumer.max_bytes.to_value(),
            "max-time" => self.settings.lock().unwrap().consumer.max_time.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SOURCE);

        // The name of GstObjects can still be changed until they become child of another object.
        self.appsrc
            .set_property("name", format!("{}-appsrc", self.obj().name()));
        gst_utils::StreamProducer::configure_consumer_with(
            &self.appsrc,
            self.settings.lock().unwrap().consumer,
        );
        obj.add(&self.appsrc).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
        self.srcpad
            .set_target(Some(&self.appsrc.static_pad("src").unwrap()))
            .unwrap();
    }
}

impl GstObjectImpl for InterSrc {}

impl ElementImpl for InterSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Inter Src",
                "Generic/Src",
                "Inter Src",
                "Mathieu Duponchelle <mathieu@centricular.com>",
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
            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        if transition == gst::StateChange::ReadyToPaused {
            if let Err(err) = self.prepare() {
                gst::element_error!(
                    self.obj(),
                    gst::StreamError::Failed,
                    ["Failed to prepare: {}", err]
                );
                return Err(gst::StateChangeError);
            }
        }

        let ret = self.parent_change_state(transition)?;

        if transition == gst::StateChange::PausedToReady {
            self.unprepare();
        }

        Ok(ret)
    }
}

impl BinImpl for InterSrc {}
