use anyhow::Error;
use clap::Parser;
use gst::prelude::*;

#[derive(Parser, Debug)]
struct Args {
    whip_endpoint: String,
}

fn main() -> Result<(), Error> {
    gst::init()?;

    let args = Args::parse();

    let pipeline = gst::Pipeline::builder().build();

    let videotestsrc = gst::ElementFactory::make("videotestsrc").build()?;
    let audiotestsrc = gst::ElementFactory::make("audiotestsrc").build()?;
    let vqueue = gst::ElementFactory::make("queue").build()?;
    let aqueue = gst::ElementFactory::make("queue").build()?;
    let whipclientsink = gst::ElementFactory::make("whipclientsink").build()?;
    whipclientsink
        .dynamic_cast_ref::<gst::ChildProxy>()
        .unwrap()
        .set_child_property("signaller::whip-endpoint", args.whip_endpoint);

    pipeline.add_many([&videotestsrc, &vqueue, &whipclientsink])?;
    gst::Element::link_many([&videotestsrc, &vqueue, &whipclientsink])?;

    pipeline.add_many([&audiotestsrc, &aqueue])?;
    gst::Element::link_many([&audiotestsrc, &aqueue, &whipclientsink])?;

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
