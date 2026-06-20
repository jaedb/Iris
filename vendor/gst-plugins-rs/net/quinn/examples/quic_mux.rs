use clap::Parser;
use gst::glib;
use gst::prelude::*;
use std::str::FromStr;
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gst-quinn-mux",
        gst::DebugColorFlags::empty(),
        Some("gst-quinn-mux"),
    )
});

#[derive(Parser, Debug)]
#[command(name = "gst-quinn-mux")]
#[command(version = "0.1")]
#[command(about = "Code for testing multiplexing with QUIC", long_about = None)]
struct Cli {
    #[clap(long, short, action)]
    receiver: bool,
    #[clap(long, short, action)]
    webtransport: bool,
}

fn video_bin(text: String) -> gst::Bin {
    let videosrc = gst::ElementFactory::make("videotestsrc").build().unwrap();
    let overlay = gst::ElementFactory::make("clockoverlay").build().unwrap();
    let convert = gst::ElementFactory::make("videoconvert").build().unwrap();
    let capsf = gst::ElementFactory::make("capsfilter").build().unwrap();
    let enc = gst::ElementFactory::make("x264enc").build().unwrap();
    let caps = gst::Caps::from_str(
        "video/x-h264,width=640,height=480,format=I420,framerate=25/1,stream-format=byte-stream",
    )
    .unwrap();

    let bin = gst::Bin::builder().name(text.clone()).build();

    capsf.set_property("caps", caps);

    overlay.set_property("text", text);
    overlay.set_property_from_str("valignment", "top");
    overlay.set_property_from_str("halignment", "left");
    overlay.set_property_from_str("font-desc", "Sans, 72");

    bin.add_many([&videosrc, &overlay, &convert, &enc, &capsf])
        .expect("Failed to add elements");

    videosrc.link(&overlay).unwrap();
    overlay.link(&convert).unwrap();
    convert.link(&enc).unwrap();
    enc.link(&capsf).unwrap();

    let parse_srcpad = capsf.static_pad("src").unwrap();
    let srcpad = gst::GhostPad::builder(gst::PadDirection::Src)
        .name("src")
        .build();
    srcpad.set_target(Some(&parse_srcpad)).unwrap();

    bin.add_pad(&srcpad).unwrap();

    bin
}

fn depay_bin(pipeline: &gst::Pipeline, bin_name: String) -> gst::Bin {
    let bin = gst::Bin::builder().name(bin_name).build();

    let queue = gst::ElementFactory::make("queue").build().unwrap();
    let decode = gst::ElementFactory::make("decodebin").build().unwrap();

    bin.add(&queue).unwrap();
    bin.add(&decode).unwrap();

    queue.link(&decode).unwrap();

    queue.sync_state_with_parent().unwrap();
    decode.sync_state_with_parent().unwrap();

    let queue_sinkpad = queue.static_pad("sink").unwrap();
    let sinkpad = gst::GhostPad::builder(gst::PadDirection::Sink)
        .name("sink")
        .build();
    sinkpad.set_target(Some(&queue_sinkpad)).unwrap();

    let srcpad = gst::GhostPad::builder(gst::PadDirection::Src)
        .name("src")
        .build();

    bin.add_pad(&sinkpad).unwrap();
    bin.add_pad(&srcpad).unwrap();

    let pipeline_weak = pipeline.downgrade();
    decode.connect("pad-added", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to demux pad-added must be pad");
        gst::info!(CAT, "decodebin pad {} added", pad.name());
        srcpad.set_target(Some(&pad)).unwrap();

        let compositor = pipeline.by_name("compositor").unwrap();
        let compositor_sink_pad = compositor.request_pad_simple("sink_%u").unwrap();

        srcpad.link(&compositor_sink_pad).unwrap();

        match compositor_sink_pad.name().as_str() {
            "sink_0" => {
                compositor_sink_pad.set_property("xpos", 0);
                compositor_sink_pad.set_property("ypos", 0);
            }
            "sink_1" => {
                compositor_sink_pad.set_property("xpos", 640);
                compositor_sink_pad.set_property("ypos", 0);
            }
            "sink_2" => {
                compositor_sink_pad.set_property("xpos", 0);
                compositor_sink_pad.set_property("ypos", 480);
            }
            "sink_3" => {
                compositor_sink_pad.set_property("xpos", 640);
                compositor_sink_pad.set_property("ypos", 480);
            }
            _ => (),
        }

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "decodebin-pad-added");

        None
    });

    bin
}

fn receive_pipeline(pipeline: &gst::Pipeline, use_webtransport: bool) {
    let quicsrc = if use_webtransport {
        gst::ElementFactory::make("quinnwtclientsrc")
            .build()
            .unwrap()
    } else {
        gst::ElementFactory::make("quinnquicsrc").build().unwrap()
    };
    let demux = gst::ElementFactory::make("quinnquicdemux").build().unwrap();
    let compositor = gst::ElementFactory::make("compositor")
        .name("compositor")
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("autovideosink")
        .name("video-sink")
        .build()
        .unwrap();

    if use_webtransport {
        quicsrc.set_property("url", "https://127.0.0.1:4445");
    } else {
        quicsrc.set_property("initial-mtu", 1200u32);
        quicsrc.set_property("min-mtu", 1200u32);
        quicsrc.set_property("upper-bound-mtu", 65527u32);
        quicsrc.set_property("max-udp-payload-size", 65527u32);
        quicsrc.set_property("server-name", "quinnmux-test");
    }

    quicsrc.set_property("secure-connection", false);

    pipeline
        .add_many([&quicsrc, &demux, &compositor, &sink])
        .unwrap();

    quicsrc.link(&demux).unwrap();
    compositor.link(&sink).unwrap();

    let pipeline_weak = pipeline.downgrade();
    demux.connect("pad-added", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to demux pad-added must be pad");
        gst::info!(CAT, "QUIC demuxer pad {} added", pad.name());

        let bin = depay_bin(&pipeline, pad.name().to_string());

        pipeline.add(&bin).unwrap();

        bin.sync_state_with_parent().unwrap();

        let sinkpad = bin.static_pad("sink").unwrap();
        pad.link(&sinkpad).unwrap();

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "demux-pad-added");

        None
    });

    let pipeline_weak = pipeline.downgrade();
    demux.connect("pad-removed", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to demux pad-removed must be pad");
        let pad_name = pad.name();

        gst::info!(CAT, "QUIC demuxer pad {} removed", pad_name);

        if let Some(bin) = pipeline.by_name(&pad_name) {
            let compositor = pipeline.by_name("compositor").unwrap();
            let bin_srcpad = bin.static_pad("src").unwrap();

            if let Some(peer) = bin_srcpad.peer() {
                bin_srcpad.unlink(&peer).unwrap();

                gst::info!(CAT, "Unlinked {}", peer.name());

                compositor.release_request_pad(&peer);

                gst::info!(CAT, "Released pad {} from compositor", peer.name());
            }

            if compositor.num_sink_pads() == 0 {
                compositor.send_event(gst::event::Eos::new());
            }

            pipeline.remove(&bin).unwrap();

            let pipeline_weak = pipeline.downgrade();
            glib::idle_add_once(move || {
                let _ = match pipeline_weak.upgrade() {
                    Some(self_) => self_,
                    None => return,
                };

                let _ = bin.set_state(gst::State::Null);
            });

            pipeline.debug_to_dot_file_with_ts(
                gst::DebugGraphDetails::all(),
                format!("removed-bin-downstream-to-{pad_name}"),
            );
        }

        None
    });
}

fn send_pipeline(pipeline: &gst::Pipeline, use_webtransport: bool) {
    let video1 = video_bin("Stream 1".to_string());
    let video2 = video_bin("Stream 2".to_string());
    let video3 = video_bin("Stream 3".to_string());
    let video4 = video_bin("Stream 4".to_string());
    let mux = gst::ElementFactory::make("quinnquicmux")
        .name("quic-mux")
        .build()
        .unwrap();
    let sink = if use_webtransport {
        gst::ElementFactory::make("quinnwtserversink")
            .name("wt-sink")
            .build()
            .unwrap()
    } else {
        gst::ElementFactory::make("quinnquicsink")
            .name("quic-sink")
            .build()
            .unwrap()
    };

    sink.set_property("drop-buffer-for-datagram", true);
    sink.set_property("initial-mtu", 1200u32);
    sink.set_property("min-mtu", 1200u32);
    sink.set_property("upper-bound-mtu", 65527u32);
    sink.set_property("max-udp-payload-size", 65527u32);
    sink.set_property("use-datagram", false);
    sink.set_property("secure-connection", false);
    sink.set_property("server-name", "quinnmux-test");

    if use_webtransport {
        sink.set_property("address", "127.0.0.1");
        sink.set_property("port", 4445u32);
    }

    pipeline
        .add_many([&video1, &video2, &video3, &video4])
        .unwrap();
    pipeline.add_many([&mux, &sink]).unwrap();

    mux.link(&sink).unwrap();

    let video1_mux_pad = mux.request_pad_simple("stream_%u").unwrap();
    let video2_mux_pad = mux.request_pad_simple("stream_%u").unwrap();
    let video3_mux_pad = mux.request_pad_simple("stream_%u").unwrap();
    let video4_mux_pad = mux.request_pad_simple("stream_%u").unwrap();

    video1_mux_pad.set_property("priority", 1);
    video2_mux_pad.set_property("priority", 2);
    video3_mux_pad.set_property("priority", 3);
    video4_mux_pad.set_property("priority", 4);

    let video1_pad = video1.static_pad("src").unwrap();
    let video2_pad = video2.static_pad("src").unwrap();
    let video3_pad = video3.static_pad("src").unwrap();
    let video4_pad = video4.static_pad("src").unwrap();

    video1_pad.link(&video1_mux_pad).unwrap();
    video2_pad.link(&video2_mux_pad).unwrap();
    video3_pad.link(&video3_mux_pad).unwrap();
    video4_pad.link(&video4_mux_pad).unwrap();

    // Test releasing stream/request pad from muxer
    let weak_pipeline = pipeline.downgrade();
    glib::timeout_add_seconds_once(30, move || {
        let pipeline = match weak_pipeline.upgrade() {
            Some(pipeline) => pipeline,
            None => return,
        };

        gst::info!(CAT, "Adding probe to remove Stream 4....");
        video4_pad.add_probe(gst::PadProbeType::IDLE, move |pad, _probe_info| {
            if let Some(peer) = pad.peer() {
                pad.unlink(&peer).unwrap();

                gst::info!(
                    CAT,
                    "Removing Stream 4,{} and {} unlinked",
                    pad.name(),
                    peer.name()
                );

                if let Some(parent) = peer
                    .parent()
                    .and_then(|p| p.downcast::<gst::Element>().ok())
                {
                    gst::log!(
                        CAT,
                        "Releasing request pad {} from parent {}",
                        peer.name(),
                        parent.name()
                    );
                    parent.release_request_pad(&peer);
                    gst::log!(
                        CAT,
                        "Released request pad {} from parent {}",
                        peer.name(),
                        parent.name()
                    );
                }

                if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
                    pipeline.remove(&parent).unwrap();

                    let weak_pipeline = pipeline.downgrade();
                    glib::idle_add_once(move || {
                        let _ = match weak_pipeline.upgrade() {
                            Some(self_) => self_,
                            None => return,
                        };

                        let _ = parent.set_state(gst::State::Null);
                    });
                }
            }

            gst::info!(CAT, "Removed Stream 4");

            pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "removed-stream-4");

            gst::PadProbeReturn::Drop
        });
    });

    pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "setup-quic-mux");
}

fn main() {
    let cli = Cli::parse();

    gst::init().unwrap();

    let pipeline = gst::Pipeline::new();
    let context = glib::MainContext::default();
    let main_loop = glib::MainLoop::new(Some(&context), false);

    if !cli.receiver {
        send_pipeline(&pipeline, cli.webtransport);
        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "quic-mux-send-pipeline");
    } else {
        receive_pipeline(&pipeline, cli.webtransport);
        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "quic-mux-recv-pipeline");
    }

    let _ = pipeline.set_state(gst::State::Playing);

    let bus = pipeline.bus().unwrap();
    let l_clone = main_loop.clone();
    let _bus_watch = bus
        .add_watch(move |_, msg| {
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    println!("\nReceived End of Stream, quitting...");
                    l_clone.quit();
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };
            glib::ControlFlow::Continue
        })
        .expect("Failed to add bus watch");

    ctrlc::set_handler({
        let pipeline_weak = pipeline.downgrade();
        move || {
            let pipeline = pipeline_weak.upgrade().unwrap();
            println!("\nReceived Ctrl-c, sending EOS...");
            pipeline.send_event(gst::event::Eos::new());
        }
    })
    .unwrap();

    let weak_pipeline = pipeline.downgrade();
    // Capture pipeline graph 5 secs later to capture all STATE changes.
    glib::timeout_add_seconds_once(5, move || {
        let pipeline = match weak_pipeline.upgrade() {
            Some(pipeline) => pipeline,
            None => return,
        };

        let name = if !cli.receiver {
            "gst-quinnquicmux-pipeline"
        } else {
            "gst-quinnquicdemux-pipeline"
        };

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), name);
    });

    let _ = pipeline.set_state(gst::State::Playing);

    main_loop.run();

    pipeline.set_state(gst::State::Null).unwrap();
}
