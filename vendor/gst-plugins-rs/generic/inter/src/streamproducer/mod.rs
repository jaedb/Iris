use gst::prelude::*;
use gst_utils::streamproducer::{ConsumerSettings, ProducerSettings};

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{anyhow, Error};
use std::sync::LazyLock;

pub enum InterStreamProducer {
    Pending {
        consumers: HashMap<gst_app::AppSrc, ConsumerSettings>,
    },
    Active {
        producer: gst_utils::StreamProducer,
        links: HashMap<gst_app::AppSrc, gst_utils::ConsumptionLink>,
    },
}

static PRODUCERS: LazyLock<Mutex<HashMap<String, InterStreamProducer>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn toplevel(obj: &gst::Object) -> gst::Object {
    if let Some(parent) = obj.parent() {
        toplevel(&parent)
    } else {
        obj.clone()
    }
}

fn ensure_different_toplevel(producer: &gst_app::AppSink, consumer: &gst_app::AppSrc) {
    let top_a = toplevel(producer.upcast_ref());
    let top_b = toplevel(consumer.upcast_ref());

    if top_a == top_b {
        gst::glib::g_critical!(
            "gstrsinter",
            "Intersink with appsink {} should not share the same toplevel bin \
             as intersrc with appsrc {}, this results in loops in latency calculation",
            producer.name(),
            consumer.name()
        );
    }
}

impl InterStreamProducer {
    pub fn acquire(
        name: &str,
        appsink: &gst_app::AppSink,
        settings: ProducerSettings,
    ) -> Result<gst_utils::StreamProducer, Error> {
        let mut producers = PRODUCERS.lock().unwrap();

        if let Some(producer) = producers.remove(name) {
            match producer {
                InterStreamProducer::Pending { consumers } => {
                    let producer = gst_utils::StreamProducer::with(appsink, settings);
                    let mut links = HashMap::new();

                    for (consumer, settings) in consumers {
                        ensure_different_toplevel(appsink, &consumer);

                        let link = producer
                            .add_consumer_with(&consumer, settings)
                            .expect("consumer should not have already been added");
                        links.insert(consumer, link);
                    }

                    producers.insert(
                        name.to_string(),
                        InterStreamProducer::Active {
                            producer: producer.clone(),
                            links,
                        },
                    );

                    Ok(producer)
                }
                InterStreamProducer::Active { .. } => {
                    producers.insert(name.to_string(), producer);

                    Err(anyhow!(
                        "An active producer already exists with name {}",
                        name
                    ))
                }
            }
        } else {
            let producer = gst_utils::StreamProducer::with(appsink, settings);

            producers.insert(
                name.to_string(),
                InterStreamProducer::Active {
                    producer: producer.clone(),
                    links: HashMap::new(),
                },
            );

            Ok(producer)
        }
    }

    pub fn release(name: &str) -> Option<gst_app::AppSink> {
        let mut producers = PRODUCERS.lock().unwrap();

        if let Some(producer) = producers.remove(name) {
            match producer {
                InterStreamProducer::Pending { .. } => None,
                InterStreamProducer::Active { links, .. } if links.is_empty() => None,
                InterStreamProducer::Active { links, producer } => {
                    producers.insert(
                        name.to_string(),
                        InterStreamProducer::Pending {
                            consumers: HashMap::from_iter(
                                links
                                    .iter()
                                    .map(|(consumer, link)| (consumer.clone(), link.settings())),
                            ),
                        },
                    );

                    Some(producer.appsink().clone())
                }
            }
        } else {
            None
        }
    }

    pub fn subscribe(name: &str, consumer: &gst_app::AppSrc, settings: ConsumerSettings) {
        let mut producers = PRODUCERS.lock().unwrap();

        if let Some(producer) = producers.get_mut(name) {
            match producer {
                InterStreamProducer::Pending { consumers } => {
                    consumers.insert(consumer.clone(), settings);
                }
                InterStreamProducer::Active { producer, links } => {
                    ensure_different_toplevel(producer.appsink(), consumer);

                    let link = producer
                        .add_consumer_with(consumer, settings)
                        .expect("consumer should not already have been added");
                    links.insert(consumer.clone(), link);
                }
            }
        } else {
            let producer = InterStreamProducer::Pending {
                consumers: [(consumer.clone(), settings)].into(),
            };
            producers.insert(name.to_string(), producer);
        }
    }

    pub fn unsubscribe(name: &str, consumer: &gst_app::AppSrc) -> bool {
        let mut producers = PRODUCERS.lock().unwrap();

        if let Some(producer) = producers.get_mut(name) {
            match producer {
                InterStreamProducer::Pending { consumers } => consumers.remove(consumer).is_some(),
                InterStreamProducer::Active { links, .. } => links.remove(consumer).is_some(),
            }
        } else {
            false
        }
    }
}
