// The goal of this example is to demonstrate how to affect bitrate allocation
// when congestion control is happening in a session with multiple encoders

use anyhow::Error;
use gst::glib;
use gst::prelude::*;

fn main() -> Result<(), Error> {
    gst::init()?;

    // Create a very simple webrtc producer, offering a single video stream
    let pipeline = gst::Pipeline::builder().build();

    let videotestsrc = gst::ElementFactory::make("videotestsrc").build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let webrtcsink = gst::ElementFactory::make("webrtcsink").build()?;

    webrtcsink.connect_closure(
        "define-encoder-bitrates",
        false,
        glib::closure!(|_webrtcsink: &gst::Element,
                        _consumer_id: &str,
                        overall: i32,
                        _in_structure: gst::Structure| {
            let out_s = gst::Structure::builder("webrtcsink/encoder-bitrates")
                .field(
                    "video_0",
                    overall.mul_div_round(75, 100).expect("should be scalable"),
                )
                .field(
                    "video_1",
                    overall.mul_div_round(25, 100).expect("should be scalable"),
                )
                .build();

            Some(out_s)
        }),
    );

    webrtcsink.set_property("run-signalling-server", true);
    webrtcsink.set_property("run-web-server", true);

    pipeline.add_many([&videotestsrc, &queue, &webrtcsink])?;
    gst::Element::link_many([&videotestsrc, &queue, &webrtcsink])?;

    let videotestsrc = gst::ElementFactory::make("videotestsrc").build()?;
    let queue = gst::ElementFactory::make("queue").build()?;

    pipeline.add_many([&videotestsrc, &queue])?;
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
