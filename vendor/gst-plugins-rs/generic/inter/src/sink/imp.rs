// SPDX-License-Identifier: MPL-2.0

use crate::streamproducer::InterStreamProducer;
use anyhow::Error;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_utils::streamproducer::{ProducerSettings, DEFAULT_PRODUCER_SYNC};

use std::sync::Mutex;

use std::sync::LazyLock;

const DEFAULT_PRODUCER_NAME: &str = "default";

#[derive(Debug)]
struct Settings {
    producer_name: String,
    producer: ProducerSettings,
    event_types: Vec<gst::EventType>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            producer_name: DEFAULT_PRODUCER_NAME.to_string(),
            producer: ProducerSettings::default(),
            event_types: vec![gst::EventType::Eos],
        }
    }
}

pub struct InterSink {
    appsink: gst_app::AppSink,
    sinkpad: gst::GhostPad,
    settings: Mutex<Settings>,
}

impl InterSink {
    fn prepare(&self) -> Result<(), Error> {
        let settings = self.settings.lock().unwrap();

        InterStreamProducer::acquire(&settings.producer_name, &self.appsink, settings.producer).map(
            |producer| {
                producer.set_forward_events(settings.event_types.clone());
            },
        )
    }

    fn unprepare(&self) {
        let settings = self.settings.lock().unwrap();
        InterStreamProducer::release(&settings.producer_name);
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "intersink",
        gst::DebugColorFlags::empty(),
        Some("Inter Sink"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for InterSink {
    const NAME: &'static str = "GstInterSink";
    type Type = super::InterSink;
    type ParentType = gst::Bin;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::GhostPad::from_template(&templ);

        Self {
            appsink: gst_app::AppSink::builder().name("appsink").build(),
            sinkpad: sinkpad.upcast(),
            settings: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for InterSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("producer-name")
                    .nick("Producer Name")
                    .blurb("Producer Name to use")
                    .doc_show_default()
                    .mutable_playing()
                    .build(),
                gst::ParamSpecArray::builder("event-types")
                    .element_spec(
                        &glib::ParamSpecEnum::builder_with_default(
                            "event-type",
                            gst::EventType::Eos,
                        )
                        .nick("Event Type")
                        .blurb("Event Type")
                        .build(),
                    )
                    .nick("Forwarded Event Types")
                    .blurb("Forward Event Types (default EOS)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("sync")
                    .nick("Sync")
                    .blurb("Sync on the clock")
                    .default_value(DEFAULT_PRODUCER_SYNC)
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

                if let Some(appsink) = InterStreamProducer::release(&old_producer_name) {
                    if let Err(err) = InterStreamProducer::acquire(
                        &settings.producer_name,
                        &appsink,
                        settings.producer,
                    ) {
                        drop(settings);
                        gst::error!(CAT, imp = self, "{err}");
                        self.post_error_message(gst::error_msg!(
                            gst::StreamError::Failed,
                            ["{err}"]
                        ))
                    } else {
                        drop(settings);
                        // This is required because StreamProducer obtains the latency
                        // it needs to forward from Latency events, and we need to let the
                        // application know it should recalculate latency to get the event
                        // to travel upstream again
                        self.post_message(gst::message::Latency::new());
                    }
                }
            }
            "event-types" => {
                let mut settings = self.settings.lock().unwrap();
                let types = value
                    .get::<gst::Array>()
                    .expect("type checked upstream")
                    .iter()
                    .map(|v| v.get::<gst::EventType>().expect("type checked upstream"))
                    .collect::<Vec<_>>();
                settings.event_types = types;
            }
            "sync" => {
                self.settings.lock().unwrap().producer.sync = value.get::<bool>().unwrap();
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
            "event-types" => {
                let settings = self.settings.lock().unwrap();
                settings
                    .event_types
                    .iter()
                    .map(|x| x.to_send_value())
                    .collect::<gst::Array>()
                    .to_value()
            }
            "sync" => self.settings.lock().unwrap().producer.sync.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SINK);

        // The name of GstObjects can still be changed until they become child of another object.
        self.appsink
            .set_property("name", format!("{}-appsink", self.obj().name()));
        obj.add(&self.appsink).unwrap();
        obj.add_pad(&self.sinkpad).unwrap();
        self.sinkpad
            .set_target(Some(&self.appsink.static_pad("sink").unwrap()))
            .unwrap();
    }
}

impl GstObjectImpl for InterSink {}

impl ElementImpl for InterSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Inter Sink",
                "Generic/Sink",
                "Inter Sink",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![sink_pad_template]
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

impl BinImpl for InterSink {}
