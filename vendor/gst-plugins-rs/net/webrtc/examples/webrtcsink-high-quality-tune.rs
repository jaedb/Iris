// The goal of this example is to demonstrate how to tune webrtcsink for a
// high-quality, single consumer use case
//
// By default webrtcsink will use properties on elements such as videoscale
// or the video encoders with the intent of maximising the potential number
// of concurrent consumers while achieving a somewhat decent quality.
//
// In cases where the application knows that CPU usage will not be a concern,
// for instance because there will only ever be a single concurrent consumer,
// or it is running on a supercomputer, it may wish to maximize quality instead.
//
// This example can be used as a starting point by applications that need an
// increased amount of control over webrtcsink internals, bearing in mind that
// webrtcsink does not guarantee stability of said internals.

use anyhow::Error;
use gst::prelude::*;

fn main() -> Result<(), Error> {
    gst::init()?;

    // Create a very simple webrtc producer, offering a single video stream
    let pipeline = gst::Pipeline::builder().build();

    let videotestsrc = gst::ElementFactory::make("videotestsrc").build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let webrtcsink = gst::ElementFactory::make("webrtcsink").build()?;

    // For the sake of the example we will force H264
    webrtcsink.set_property_from_str("video-caps", "video/x-h264");

    // We want to tweak how webrtcsink performs video scaling when needed, as
    // this can have a very visible impact over quality.
    //
    // To achieve that, we will connect to deep-element-added on the consumer
    // pipeline.
    webrtcsink.connect("consumer-pipeline-created", false, |values| {
        let pipeline = values[2].get::<gst::Pipeline>().unwrap();

        pipeline.connect("deep-element-added", false, |values| {
            let element = values[2].get::<gst::Element>().unwrap();

            if element
                .factory()
                .is_some_and(|factory| factory.name() == "videoscale")
            {
                println!("Tuning videoscale");
                element.set_property_from_str("method", "lanczos");
            }

            None
        });

        None
    });

    // We *could* access the consumer encoder from our
    // consumer-pipeline-created handler, but doing so from an encoder-setup
    // callback is better practice, as it will also get called
    // when running the discovery pipelines, and changing properties on the
    // encoder may in theory affect the caps it outputs.
    webrtcsink.connect("encoder-setup", true, |values| {
        let encoder = values[3].get::<gst::Element>().unwrap();

        if let Some(factory) = encoder.factory() {
            println!("Encoder: {}", factory.name());

            match factory.name().as_str() {
                "x264enc" => {
                    println!("Applying extra configuration to x264enc");
                    encoder.set_property_from_str("speed-preset", "medium");
                }
                name => {
                    println!(
                        "Can't tune unsupported H264 encoder {name}, \
                             set GST_PLUGIN_FEATURE_RANK=x264enc:1000 when \
                             running the example"
                    );
                }
            }
        }

        Some(false.to_value())
    });

    pipeline.add_many([&videotestsrc, &queue, &webrtcsink])?;
    gst::Element::link_many([&videotestsrc, &queue, &webrtcsink])?;

    // Now we simply run the pipeline to completion

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().expect("Pipeline should have a bus");

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => {
                println!("EOS");
                break;
            }
            MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null)?;
                eprintln!(
                    "Got error from {}: {} ({})",
                    msg.src()
                        .map(|s| String::from(s.path_string()))
                        .unwrap_or_else(|| "None".into()),
                    err.error(),
                    err.debug().unwrap_or_else(|| "".into()),
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}
