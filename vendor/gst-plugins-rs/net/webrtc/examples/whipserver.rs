use std::process::exit;

use anyhow::Error;
use clap::Parser;
use gst::prelude::*;

#[derive(Parser, Debug)]
struct Args {
    host_addr: String,
}

fn link_video(pad: &gst::Pad, pipeline: &gst::Pipeline) {
    let q = gst::ElementFactory::make("queue")
        .name(format!("queue_{}", pad.name()))
        .build()
        .unwrap();

    let vc = gst::ElementFactory::make("videoconvert")
        .name(format!("videoconvert_{}", pad.name()))
        .build()
        .unwrap();

    let vsink = gst::ElementFactory::make("autovideosink")
        .name(format!("vsink_{}", pad.name()))
        .build()
        .unwrap();

    pipeline.add_many([&q, &vc, &vsink]).unwrap();
    gst::Element::link_many([&q, &vc, &vsink]).unwrap();
    let qsinkpad = q.static_pad("sink").unwrap();
    pad.link(&qsinkpad).expect("linking should work");

    q.sync_state_with_parent().unwrap();
    vc.sync_state_with_parent().unwrap();
    vsink.sync_state_with_parent().unwrap();
}

fn unlink_video(pad: &gst::Pad, pipeline: &gst::Pipeline) {
    let q = pipeline
        .by_name(format!("queue_{}", pad.name()).as_str())
        .unwrap();
    let vc = pipeline
        .by_name(format!("videoconvert_{}", pad.name()).as_str())
        .unwrap();
    let vsink = pipeline
        .by_name(format!("vsink_{}", pad.name()).as_str())
        .unwrap();

    q.set_state(gst::State::Null).unwrap();
    vc.set_state(gst::State::Null).unwrap();
    vsink.set_state(gst::State::Null).unwrap();

    pipeline.remove_many([&q, &vc, &vsink]).unwrap();
}

fn link_audio(pad: &gst::Pad, pipeline: &gst::Pipeline) {
    let aq = gst::ElementFactory::make("queue")
        .name(format!("aqueue_{}", pad.name()))
        .build()
        .unwrap();

    let ac = gst::ElementFactory::make("audioconvert")
        .name(format!("audioconvert_{}", pad.name()))
        .build()
        .unwrap();

    let asink = gst::ElementFactory::make("autoaudiosink")
        .name(format!("asink_{}", pad.name()))
        .build()
        .unwrap();

    pipeline.add_many([&aq, &ac, &asink]).unwrap();
    gst::Element::link_many([&aq, &ac, &asink]).unwrap();
    let qsinkpad = aq.static_pad("sink").unwrap();
    pad.link(&qsinkpad).expect("linking should work");

    aq.sync_state_with_parent().unwrap();
    ac.sync_state_with_parent().unwrap();
    asink.sync_state_with_parent().unwrap();
}

fn unlink_audio(pad: &gst::Pad, pipeline: &gst::Pipeline) {
    let aq = pipeline
        .by_name(format!("aqueue_{}", pad.name()).as_str())
        .unwrap();
    let ac = pipeline
        .by_name(format!("audioconvert_{}", pad.name()).as_str())
        .unwrap();
    let asink = pipeline
        .by_name(format!("asink_{}", pad.name()).as_str())
        .unwrap();

    aq.set_state(gst::State::Null).unwrap();
    ac.set_state(gst::State::Null).unwrap();
    asink.set_state(gst::State::Null).unwrap();

    pipeline.remove_many([&aq, &ac, &asink]).unwrap();
}

fn main() -> Result<(), Error> {
    gst::init()?;

    let args = Args::parse();

    let pipeline = gst::Pipeline::builder().build();
    let ws = gst::ElementFactory::make("whipserversrc").build()?;
    ws.dynamic_cast_ref::<gst::ChildProxy>()
        .unwrap()
        .set_child_property("signaller::host-addr", args.host_addr);

    ws.set_property("enable-data-channel-navigation", true);

    let pipe = pipeline.clone();
    ws.connect_pad_added(move |_ws, pad| {
        if pad.name().contains("video_") {
            link_video(pad, &pipe);
        } else if pad.name().contains("audio_") {
            link_audio(pad, &pipe);
        } else {
            println!("unknown pad type {}", pad.name());
        }
    });

    let pipe = pipeline.clone();
    ws.connect_pad_removed(move |_ws, pad| {
        if pad.name().contains("video_") {
            unlink_video(pad, &pipe);
        } else if pad.name().contains("audio_") {
            unlink_audio(pad, &pipe);
        } else {
            println!("unknown pad type {}", pad.name());
        }
    });
    pipeline.add(&ws)?;
    pipeline.set_state(gst::State::Playing)?;

    let p = pipeline.clone();
    ctrlc::set_handler(move || {
        p.set_state(gst::State::Null).unwrap();
        exit(0);
    })
    .expect("Error setting Ctrl-C handler");

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
