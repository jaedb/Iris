use anyhow::Error;
use futures::prelude::*;
use gst::prelude::*;
use std::collections::HashMap;
use std::io::prelude::*;
use tokio::task;

struct Producer {
    pipeline: gst::Pipeline,
    sink: gst::Element,
    overlay: gst::Element,
}

struct Consumer {
    pipeline: gst::Pipeline,
    src: gst::Element,
}

fn create_sink_pipeline(producer_name: &str) -> Result<Producer, Error> {
    let pipeline = gst::Pipeline::builder()
        .name(format!("producer-{producer_name}"))
        .build();

    let videotestsrc = gst::ElementFactory::make("videotestsrc")
        .property_from_str("pattern", "ball")
        .property("is-live", true)
        .build()?;
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("framerate", gst::Fraction::new(50, 1))
                .build(),
        )
        .build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let overlay = gst::ElementFactory::make("textoverlay")
        .property("font-desc", "Sans 30")
        .property("text", format!("Producer: {producer_name}"))
        .property_from_str("valignment", "top")
        .build()?;
    let timeoverlay = gst::ElementFactory::make("timeoverlay")
        .property("font-desc", "Sans 30")
        .property_from_str("valignment", "center")
        .property_from_str("halignment", "center")
        .build()?;
    let sink = gst::ElementFactory::make("intersink")
        .property("producer-name", producer_name)
        .build()?;

    pipeline.add_many([
        &videotestsrc,
        &capsfilter,
        &queue,
        &overlay,
        &timeoverlay,
        &sink,
    ])?;
    gst::Element::link_many([
        &videotestsrc,
        &capsfilter,
        &queue,
        &overlay,
        &timeoverlay,
        &sink,
    ])?;

    Ok(Producer {
        pipeline,
        sink,
        overlay,
    })
}

fn create_src_pipeline(producer_name: &str, consumer_name: &str) -> Result<Consumer, Error> {
    let pipeline = gst::Pipeline::builder()
        .name(format!("consumer-{consumer_name}"))
        .build();

    let src = gst::ElementFactory::make("intersrc")
        .property("producer-name", producer_name)
        .build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let vconv = gst::ElementFactory::make("videoconvert").build()?;
    let overlay = gst::ElementFactory::make("textoverlay")
        .property("font-desc", "Sans 30")
        .property("text", format!("Consumer: {consumer_name}"))
        .property_from_str("valignment", "bottom")
        .build()?;
    let vconv2 = gst::ElementFactory::make("videoconvert").build()?;
    let sink = gst::ElementFactory::make("autovideosink").build()?;

    pipeline.add_many([&src, &queue, &vconv, &overlay, &vconv2, &sink])?;
    gst::Element::link_many([&src, &queue, &vconv, &overlay, &vconv2, &sink])?;

    Ok(Consumer { pipeline, src })
}

fn prompt_on() {
    print!("$ ");
    let _ = std::io::stdout().flush();
}

fn monitor_pipeline(pipeline: &gst::Pipeline, base_time: gst::ClockTime) -> Result<(), Error> {
    pipeline.set_clock(Some(&gst::SystemClock::obtain()))?;
    pipeline.set_start_time(gst::ClockTime::NONE);
    pipeline.set_base_time(base_time);

    pipeline.set_state(gst::State::Playing)?;

    let mut bus_stream = pipeline.bus().expect("Pipeline should have a bus").stream();

    let pipeline_clone = pipeline.downgrade();
    task::spawn(async move {
        while let Some(msg) = bus_stream.next().await {
            use gst::MessageView;

            if let Some(pipeline) = pipeline_clone.upgrade() {
                match msg.view() {
                    MessageView::Latency(..) => {
                        let _ = pipeline.recalculate_latency();
                    }
                    MessageView::Eos(..) => {
                        println!(
                            "EOS from {}",
                            msg.src()
                                .map(|s| String::from(s.path_string()))
                                .unwrap_or_else(|| "None".into())
                        );
                        prompt_on();
                        break;
                    }
                    MessageView::Error(err) => {
                        let _ = pipeline.set_state(gst::State::Null);
                        println!(
                            "Got error from {}: {} ({})",
                            msg.src()
                                .map(|s| String::from(s.path_string()))
                                .unwrap_or_else(|| "None".into()),
                            err.error(),
                            err.debug().unwrap_or_else(|| "".into()),
                        );
                        prompt_on();
                        break;
                    }
                    MessageView::StateChanged(sc) => {
                        if msg.src() == Some(pipeline.upcast_ref()) {
                            pipeline.debug_to_dot_file(
                                gst::DebugGraphDetails::all(),
                                format!("{}-{:?}-{:?}", pipeline.name(), sc.old(), sc.current()),
                            );
                        }
                    }
                    _ => (),
                }
            } else {
                break;
            }
        }
    });

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    gst::init()?;

    println!("h for help");

    let base_time = gst::SystemClock::obtain().time();

    let mut producers: HashMap<String, Producer> = HashMap::new();
    let mut consumers: HashMap<String, Consumer> = HashMap::new();

    let mut stdin = std::io::stdin().lock();
    loop {
        let mut buf = String::new();

        prompt_on();

        match stdin.read_line(&mut buf)? {
            0 => {
                eprintln!("EOF!");
                break;
            }
            _ => {
                let command: Vec<_> = buf.split_whitespace().collect();

                match command.first() {
                    Some(&"ap") => {
                        if command.len() != 2 {
                            println!("ap <producer_name>: Add a producer");
                        } else {
                            let producer_name = command.get(1).unwrap().to_string();

                            if producers.contains_key(&producer_name) {
                                println!("Producer with name {producer_name} already exists!");
                                continue;
                            }

                            let producer = create_sink_pipeline(&producer_name)?;
                            monitor_pipeline(&producer.pipeline, base_time)?;

                            println!("Added producer with name {producer_name}");

                            producers.insert(producer_name, producer);
                        }
                    }
                    Some(&"ac") => {
                        if command.len() != 3 {
                            println!("ac <consumer_name> <producer_name>: Add a consumer");
                        } else {
                            let consumer_name = command.get(1).unwrap().to_string();
                            let producer_name = command.get(2).unwrap().to_string();

                            if consumers.contains_key(&consumer_name) {
                                println!("Consumer with name {consumer_name} already exists!");
                                continue;
                            }

                            let consumer = create_src_pipeline(&producer_name, &consumer_name)?;
                            monitor_pipeline(&consumer.pipeline, base_time)?;

                            println!("Added consumer with name {consumer_name} and producer name {producer_name}");

                            consumers.insert(consumer_name, consumer);
                        }
                    }
                    Some(&"rp") => {
                        if command.len() != 2 {
                            println!("rp <producer_name>: Remove a producer");
                        } else {
                            let producer_name = command.get(1).unwrap().to_string();
                            if let Some(producer) = producers.remove(&producer_name) {
                                let _ = producer.pipeline.set_state(gst::State::Null);
                                println!("Removed producer with name {producer_name}");
                            } else {
                                println!("No producer with name {producer_name}");
                            }
                        }
                    }
                    Some(&"rc") => {
                        if command.len() != 2 {
                            println!("rc <consumer_name>: Remove a consumer");
                        } else {
                            let consumer_name = command.get(1).unwrap().to_string();
                            if let Some(consumer) = consumers.remove(&consumer_name) {
                                let _ = consumer.pipeline.set_state(gst::State::Null);
                                println!("Removed consumer with name {consumer_name}");
                            } else {
                                println!("No consumer with name {consumer_name}");
                            }
                        }
                    }
                    Some(&"cnp") => {
                        if command.len() != 3 {
                            println!("cnp <old_producer_name> <new_producer_name>: Change the name of a producer");
                        } else {
                            let old_producer_name = command.get(1).unwrap().to_string();
                            let producer_name = command.get(2).unwrap().to_string();

                            if producers.contains_key(&producer_name) {
                                println!("Producer with name {producer_name} already exists!");
                                continue;
                            }

                            if let Some(producer) = producers.remove(&old_producer_name) {
                                producer.sink.set_property("producer-name", &producer_name);
                                producer
                                    .overlay
                                    .set_property("text", format!("Producer: {producer_name}"));
                                println!(
                                    "Changed producer name {old_producer_name} -> {producer_name}"
                                );
                                producers.insert(producer_name, producer);
                            } else {
                                println!("No producer with name {old_producer_name}");
                            }
                        }
                    }
                    Some(&"cpn") => {
                        if command.len() != 3 {
                            println!("cpn <consumer_name> <new_producer_name>: Change the producer name for a consumer");
                        } else {
                            let consumer_name = command.get(1).unwrap().to_string();
                            let producer_name = command.get(2).unwrap().to_string();

                            if let Some(consumer) = consumers.get_mut(&consumer_name) {
                                consumer.src.set_property("producer-name", &producer_name);
                                println!("Changed producer name for consumer {consumer_name} to {producer_name}");
                            } else {
                                println!("No consumer with name {consumer_name}");
                            }
                        }
                    }
                    Some(&"h") => {
                        println!("h: show this help");
                        println!("ap <producer_name>: Add a producer");
                        println!("ac <consumer_name> <producer_name>: Add a consumer");
                        println!("rp <producer_name>: Remove a producer");
                        println!("rc <consumer_name>: Remove a consumer");
                        println!("cnp <old_producer_name> <new_producer_name>: Change the name of a producer");
                        println!("cpn <consumer_name> <new_producer_name>: Change the producer name for a consumer");
                    }
                    _ => {
                        println!("Unknown command");
                    }
                }
            }
        }
        buf.clear();
    }

    for (_, producer) in producers {
        let _ = producer.pipeline.set_state(gst::State::Null);
    }

    for (_, consumer) in consumers {
        let _ = consumer.pipeline.set_state(gst::State::Null);
    }

    Ok(())
}
