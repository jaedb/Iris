use anyhow::Error;
use clap::Parser;
use gst::glib;
use gst::prelude::*;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
/// Program arguments
struct Args {
    /// The number of streams to serve over the peer connection
    n_streams: u32,
}

fn build_pipeline(args: &Args) -> Result<gst::Pipeline, Error> {
    let pipeline = gst::Pipeline::new();

    let sink = gst::ElementFactory::make("webrtcsink")
        .property("run-signalling-server", true)
        .property("enable-control-data-channel", true)
        .property("video-caps", gst::Caps::new_empty_simple("video/x-h264"))
        .build()?;

    pipeline.add(&sink)?;

    let mut valves: Vec<gst::Element> = vec![];

    for i in 0..args.n_streams {
        let src = gst::ElementFactory::make("videotestsrc").build()?;
        let textoverlay = gst::ElementFactory::make("textoverlay")
            .property("text", i.to_string())
            .property("font-desc", "sans 72")
            .build()?;
        let valve = gst::ElementFactory::make("valve")
            .property_from_str("drop-mode", "transform-to-gap")
            .build()?;

        pipeline.add_many([&src, &textoverlay, &valve])?;
        gst::Element::link_many([&src, &textoverlay, &valve, &sink])?;

        valve.static_pad("src").unwrap().add_probe(
            gst::PadProbeType::EVENT_UPSTREAM,
            glib::clone!(
                #[weak]
                valve,
                #[upgrade_or]
                gst::PadProbeReturn::Ok,
                move |_pad, probe_info| {
                    let event = probe_info.event().unwrap();

                    if let gst::EventView::CustomUpstream(cup) = event.view() {
                        let s = cup.structure().unwrap();

                        if s.name() == "example/enabled-stream" {
                            let enabled: bool = s.get("enabled").unwrap();

                            valve.set_property("drop", !enabled);
                        }
                    }

                    gst::PadProbeReturn::Ok
                }
            ),
        );

        valves.push(valve);
    }

    let signaller = sink.property::<glib::Object>("signaller");
    signaller.connect_closure(
        "webrtcbin-ready",
        false,
        glib::closure!(move |_signaller: &glib::Object,
                             _session_id: &str,
                             _webrtcbin: &gst::Element| {
            for valve in &valves {
                valve.set_property("drop", false);
            }
        }),
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
