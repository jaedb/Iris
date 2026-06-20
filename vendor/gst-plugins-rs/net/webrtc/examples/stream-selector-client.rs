use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Error;
use clap::Parser;
use gst::glib;
use gst::prelude::*;
use rand::Rng;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
/// Program arguments
struct Args {
    /// Frequency for selection of enabled / disabled streams, in seconds
    frequency: u64,
}

fn connect_input_stream(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    compositor: &gst::Element,
    capsfilter: &gst::Element,
) -> Result<(), Error> {
    let depay = gst::ElementFactory::make("rtph264depay")
        .property("request-keyframe", true)
        .property("wait-for-keyframe", true)
        .build()?;
    let decode = gst::ElementFactory::make("avdec_h264").build()?;
    pipeline.add_many([&depay, &decode])?;
    gst::Element::link_many([&depay, &decode])?;

    let n_sink_pads = compositor.num_sink_pads() as i32;

    let srcpad = decode.static_pad("src").unwrap();
    let sinkpad = compositor.request_pad_simple("sink_%u").unwrap();

    let pos_x = n_sink_pads % 4 * 320;
    let pos_y = n_sink_pads / 4 * 240;

    sinkpad.set_property("width", 320);
    sinkpad.set_property("height", 240);
    sinkpad.set_property("xpos", pos_x);
    sinkpad.set_property("ypos", pos_y);

    let width = std::cmp::min(320 * 4, (n_sink_pads + 1) * 320);
    let height = pos_y + 240;

    let caps = gst::Caps::builder("video/x-raw")
        .field("width", width)
        .field("height", height)
        .build();

    capsfilter.set_property("caps", caps);

    srcpad.link(&sinkpad).unwrap();

    depay.sync_state_with_parent().unwrap();
    decode.sync_state_with_parent().unwrap();

    pad.link(&depay.static_pad("sink").unwrap()).unwrap();

    Ok(())
}

fn build_pipeline(args: &Args) -> Result<gst::Pipeline, Error> {
    let pipeline = gst::Pipeline::new();

    let src = gst::ElementFactory::make("webrtcsrc")
        .property("connect-to-first-producer", true)
        .property("enable-control-data-channel", true)
        .build()?;
    let compositor = gst::ElementFactory::make("compositor")
        .property("force-live", true)
        .build()?;
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("width", 1920)
                .field("height", 1080)
                .build(),
        )
        .build()?;
    let sink = gst::ElementFactory::make("autovideosink").build()?;

    pipeline.add_many([&src, &compositor, &capsfilter, &sink])?;

    gst::Element::link_many([&compositor, &capsfilter, &sink])?;

    let n_medias: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));

    let n_medias_clone = n_medias.clone();
    let frequency = args.frequency;
    src.connect_pad_added(glib::clone!(
        #[weak]
        pipeline,
        #[weak]
        compositor,
        #[weak]
        capsfilter,
        move |src, pad| {
            if let Err(err) = connect_input_stream(&pipeline, pad, &compositor, &capsfilter) {
                pipeline.post_error_message(gst::error_msg!(
                    gst::StreamError::Failed,
                    ["Failed to connect input stream: {err:?}",]
                ));
            }

            if let Some(n_medias) = *n_medias_clone.lock().unwrap() {
                if n_medias == src.num_src_pads() as usize + 1 {
                    let src_weak = src.downgrade();
                    std::thread::spawn(move || loop {
                        std::thread::sleep(Duration::from_secs(frequency));

                        let Some(src) = src_weak.upgrade() else {
                            break;
                        };

                        let mut rng = rand::rng();
                        for srcpad in src.src_pads() {
                            let s = gst::Structure::builder("example/enabled-stream")
                                .field("enabled", rng.random::<bool>())
                                .build();

                            let custom_event = gst::event::CustomUpstream::builder(s).build();
                            srcpad.send_event(custom_event);
                        }
                    });
                }
            }
        }
    ));

    let signaller = src.property::<glib::Object>("signaller");
    signaller.connect_closure(
        "webrtcbin-ready",
        false,
        glib::closure!(
            #[strong]
            n_medias,
            move |_signaller: &glib::Object, _session_id: &str, webrtcbin: &gst::Element| {
                webrtcbin.connect_notify(
                    Some("signaling-state"),
                    glib::clone!(
                        #[strong]
                        n_medias,
                        move |element, _param_spec| {
                            let state = element
                                .property::<gst_webrtc::WebRTCSignalingState>("signaling-state");

                            if state == gst_webrtc::WebRTCSignalingState::HaveRemoteOffer {
                                let description = element
                                    .property::<gst_webrtc::WebRTCSessionDescription>(
                                        "remote-description",
                                    );

                                *n_medias.lock().unwrap() =
                                    Some(description.sdp().medias().count());
                            }
                        }
                    ),
                );
            }
        ),
    );

    Ok(pipeline)
}

fn run_pipeline(pipeline: gst::Pipeline) -> Result<(), Error> {
    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().unwrap();

    while let Some(message) = bus.timed_pop(gst::ClockTime::NONE) {
        match message.view() {
            gst::MessageView::Eos(_) => {
                eprintln!("EOS!");
                break;
            }
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Error in element {:?}, {}",
                    err.src().map(|o| o.name()),
                    err.debug()
                        .map(|d| format!("debug:\n{d}"))
                        .unwrap_or(String::from("no debug"))
                );

                pipeline.debug_to_dot_file_with_ts(
                    gst::DebugGraphDetails::ALL,
                    format!("{}-error", pipeline.name(),),
                );

                break;
            }
            gst::MessageView::Latency(_) => {
                let _ = pipeline.recalculate_latency();
            }
            gst::MessageView::StateChanged(sc) => {
                if sc.src() == Some(pipeline.upcast_ref()) {
                    pipeline.debug_to_dot_file_with_ts(
                        gst::DebugGraphDetails::ALL,
                        format!("{}-{:?}-{:?}", pipeline.name(), sc.old(), sc.current()),
                    );
                }
            }
            _ => (),
        }
    }

    let _ = pipeline.set_state(gst::State::Null);

    Ok(())
}

fn main() -> Result<(), Error> {
    gst::init()?;

    let args = Args::parse();

    let pipeline = build_pipeline(&args)?;

    run_pipeline(pipeline)
}
