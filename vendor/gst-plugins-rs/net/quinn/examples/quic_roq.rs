use clap::Parser;
use gst::glib;
use gst::prelude::*;
use std::str::FromStr;
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gst-quinn-roq",
        gst::DebugColorFlags::empty(),
        Some("gst-quinn-roq"),
    )
});

#[derive(Parser, Debug)]
#[command(name = "gst-quinn-roq")]
#[command(version = "0.1")]
#[command(about = "Code for testing RTP with QUIC", long_about = None)]
struct Cli {
    #[clap(long, short, action)]
    receiver: bool,
    #[clap(long, short, action)]
    vp8: bool,
}

fn connect_overrun(pipeline: &gst::Pipeline, queue: &gst::Element, is_receiver: bool) {
    let pipeline_weak = pipeline.downgrade();
    queue.connect("overrun", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(element) => element,
            None => return None,
        };

        let queue = args[0]
            .get::<gst::Element>()
            .expect("First argument to overrun must be a queue");

        let cur_level_bytes = queue.property::<u32>("current-level-bytes");
        let cur_level_buffers = queue.property::<u32>("current-level-buffers");
        let cur_level_time = queue.property::<u64>("current-level-time");

        gst::info!(
            CAT,
            "Queue {} overrun, bytes: {}, buffers: {}, time: {}",
            queue.name(),
            cur_level_bytes,
            cur_level_buffers,
            cur_level_time
        );

        let dot_name = if is_receiver {
            format!("{}-overrun-receive", queue.name()).to_string()
        } else {
            format!("{}-overrun-send", queue.name()).to_string()
        };

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), dot_name);

        None
    });
}

fn video_bin(pipeline: &gst::Pipeline, text: String, use_vp8: bool) -> gst::Bin {
    let videosrc = gst::ElementFactory::make("videotestsrc").build().unwrap();
    let capsf = gst::ElementFactory::make("capsfilter").build().unwrap();
    let overlay = gst::ElementFactory::make("clockoverlay").build().unwrap();
    let convert = gst::ElementFactory::make("videoconvert").build().unwrap();
    let queue_src = gst::ElementFactory::make("queue").build().unwrap();
    let enc = if use_vp8 {
        gst::ElementFactory::make("vp8enc").build().unwrap()
    } else {
        gst::ElementFactory::make("x264enc").build().unwrap()
    };
    let enc_caps = gst::ElementFactory::make("capsfilter").build().unwrap();
    let rtppay = if use_vp8 {
        gst::ElementFactory::make("rtpvp8pay").build().unwrap()
    } else {
        gst::ElementFactory::make("rtph264pay").build().unwrap()
    };
    let queue = gst::ElementFactory::make("queue").build().unwrap();

    connect_overrun(pipeline, &queue_src, false);
    connect_overrun(pipeline, &queue, false);

    let bin = gst::Bin::builder().name(text.clone()).build();

    if use_vp8 {
        enc.set_property("deadline", 2i64);
        enc.set_property("threads", 4i32);
        enc.set_property_from_str("end-usage", "cbr");
    } else {
        enc.set_property_from_str("speed-preset", "ultrafast");
        enc.set_property_from_str("tune", "zerolatency");
        enc.set_property("threads", 4u32);
    }

    videosrc.set_property_from_str("animation-mode", "wall-time");
    // videosrc.set_property("flip", true);
    videosrc.set_property_from_str("pattern", "ball");
    videosrc.set_property("is-live", true);

    queue.set_property("max-size-buffers", 0u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

    let caps =
        gst::Caps::from_str("video/x-raw,width=640,height=480,format=I420,framerate=25/1").unwrap();
    capsf.set_property("caps", caps);

    let caps = if use_vp8 {
        gst::Caps::from_str("video/x-vp8").unwrap()
    } else {
        gst::Caps::from_str("video/x-h264,stream-format=byte-stream").unwrap()
    };
    enc_caps.set_property("caps", caps);

    if text == "Stream 1" {
        rtppay.set_property("pt", 101u32);
    } else if text == "Stream 2" {
        rtppay.set_property("pt", 102u32);
    } else if text == "Datagram 1" {
        rtppay.set_property("pt", 103u32);
    } else if text == "Datagram 2" {
        rtppay.set_property("pt", 104u32);
    }

    rtppay.set_property("mtu", u32::MAX);

    if use_vp8 {
        rtppay.set_property_from_str("picture-id-mode", "15-bit");
    } else {
        rtppay.set_property_from_str("aggregate-mode", "zero-latency");
    }

    overlay.set_property("text", text);
    overlay.set_property_from_str("valignment", "top");
    overlay.set_property_from_str("halignment", "left");
    overlay.set_property_from_str("font-desc", "Sans, 60");

    bin.add_many([
        &videosrc, &capsf, &overlay, &convert, &queue_src, &enc, &enc_caps, &rtppay, &queue,
    ])
    .expect("Failed to add elements");

    videosrc.link(&capsf).unwrap();
    capsf.link(&overlay).unwrap();
    overlay.link(&convert).unwrap();
    convert.link(&queue_src).unwrap();
    queue_src.link(&enc).unwrap();
    enc.link(&enc_caps).unwrap();
    enc_caps.link(&rtppay).unwrap();
    rtppay.link(&queue).unwrap();

    let queue_srcpad = queue.static_pad("src").unwrap();
    let srcpad = gst::GhostPad::builder(gst::PadDirection::Src)
        .name("src")
        .build();
    srcpad.set_target(Some(&queue_srcpad)).unwrap();

    bin.add_pad(&srcpad).unwrap();

    bin
}

fn depay_bin(pipeline: &gst::Pipeline, bin_name: String) -> gst::Bin {
    let bin = gst::Bin::builder().name(bin_name).build();

    let queue = gst::ElementFactory::make("queue").build().unwrap();
    let jitter = gst::ElementFactory::make("rtpjitterbuffer")
        .build()
        .unwrap();
    let decode = gst::ElementFactory::make("decodebin").build().unwrap();

    bin.add(&queue).unwrap();
    bin.add(&jitter).unwrap();
    bin.add(&decode).unwrap();

    decode.set_property("force-sw-decoders", true);

    queue.set_property_from_str("leaky", "downstream");
    queue.set_property("max-size-buffers", 0u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

    jitter.set_property("rtx-next-seqnum", false);

    queue.link(&jitter).unwrap();
    jitter.link(&decode).unwrap();

    queue.sync_state_with_parent().unwrap();
    jitter.sync_state_with_parent().unwrap();
    decode.sync_state_with_parent().unwrap();

    connect_overrun(pipeline, &queue, true);

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

    let bin_weak = bin.downgrade();
    let pipeline_weak = pipeline.downgrade();
    decode.connect("pad-added", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };
        let bin = match bin_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to demux pad-added must be pad");
        gst::info!(CAT, "decodebin pad {} added", pad.name());

        let queue = gst::ElementFactory::make("queue").build().unwrap();
        queue.set_property_from_str("leaky", "downstream");
        queue.set_property("max-size-buffers", 0u32);
        queue.set_property("max-size-bytes", 0u32);
        queue.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

        connect_overrun(&pipeline, &queue, true);

        bin.add(&queue).unwrap();
        queue.sync_state_with_parent().unwrap();

        let queue_srcpad = queue.static_pad("src").unwrap();
        let queue_sinkpad = queue.static_pad("sink").unwrap();

        pad.link(&queue_sinkpad).unwrap();

        srcpad.set_target(Some(&queue_srcpad)).unwrap();

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

fn receive_pipeline(pipeline: &gst::Pipeline, use_vp8: bool) {
    let quicsrc = gst::ElementFactory::make("quinnquicsrc").build().unwrap();
    let queue1 = gst::ElementFactory::make("queue").build().unwrap();
    let roq = gst::ElementFactory::make("quinnroqdemux")
        .name("roq-demux")
        .build()
        .unwrap();
    let compositor = gst::ElementFactory::make("compositor")
        .name("compositor")
        .build()
        .unwrap();
    let queue2 = gst::ElementFactory::make("queue").build().unwrap();
    let sink = gst::ElementFactory::make("autovideosink")
        .name("video-sink")
        .build()
        .unwrap();

    connect_overrun(pipeline, &queue1, true);
    connect_overrun(pipeline, &queue2, true);

    quicsrc.set_property("timeout", 0u32);
    quicsrc.set_property("initial-mtu", 1200u32);
    quicsrc.set_property("min-mtu", 1200u32);
    quicsrc.set_property("upper-bound-mtu", 65527u32);
    quicsrc.set_property("max-udp-payload-size", 65527u32);
    quicsrc.set_property("use-datagram", false);
    quicsrc.set_property("secure-connection", false);
    quicsrc.set_property("server-name", "sanchayanmaity.net");

    queue1.set_property_from_str("leaky", "downstream");
    queue1.set_property("max-size-buffers", 0u32);
    queue1.set_property("max-size-bytes", 0u32);
    queue1.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

    queue2.set_property_from_str("leaky", "downstream");
    queue2.set_property("max-size-buffers", 0u32);
    queue2.set_property("max-size-bytes", 0u32);
    queue2.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

    pipeline
        .add_many([&quicsrc, &queue1, &roq, &compositor, &queue2, &sink])
        .unwrap();

    quicsrc.link(&queue1).unwrap();
    queue1.link(&roq).unwrap();
    compositor.link(&queue2).unwrap();
    queue2.link(&sink).unwrap();

    let pipeline_weak = pipeline.downgrade();
    roq.connect("request-flow-id-map", false, move |args| {
        let _pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let flow_id = args[1]
            .get::<u64>()
            .expect("Second argument to rtpdemux request-flow-id-map must be u64 flow_id");
        gst::info!(CAT, "Requesting caps for flow-id {flow_id}");

        let encoding = if use_vp8 { "VP8" } else { "H264" };

        match flow_id {
            5 => Some(
                gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("encoding-name", encoding)
                    .field("payload", 101)
                    .field("clock-rate", 90000)
                    .build()
                    .to_value(),
            ),
            6 => Some(
                gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("encoding-name", encoding)
                    .field("payload", 102)
                    .field("clock-rate", 90000)
                    .build()
                    .to_value(),
            ),
            2345 => Some(
                gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("encoding-name", encoding)
                    .field("payload", 103)
                    .field("clock-rate", 90000)
                    .build()
                    .to_value(),
            ),
            2346 => Some(
                gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("encoding-name", encoding)
                    .field("payload", 104)
                    .field("clock-rate", 90000)
                    .build()
                    .to_value(),
            ),
            _ => unimplemented!(),
        }
    });

    let pipeline_weak = pipeline.downgrade();
    roq.connect("pad-added", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to rtpdemux pad-added must be pad");
        gst::info!(CAT, "QUIC RTP demuxer pad {} added", pad.name());

        if !pad.name().starts_with("src_") {
            return None;
        }

        let bin = depay_bin(&pipeline, pad.name().to_string());

        pipeline.add(&bin).unwrap();

        bin.sync_state_with_parent().unwrap();

        let sinkpad = bin.static_pad("sink").unwrap();
        pad.link(&sinkpad).unwrap();

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "roq-demux-pad-added");

        None
    });

    let pipeline_weak = pipeline.downgrade();
    roq.connect("pad-removed", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(self_) => self_,
            None => return None,
        };

        let pad = args[1]
            .get::<gst::Pad>()
            .expect("Second argument to demux pad-removed must be pad");
        let pad_name = pad.name();

        gst::info!(CAT, "QUIC RTP demuxer pad {} removed", pad_name);

        if let Some(bin) = pipeline.by_name(&pad_name) {
            let bin_srcpad = bin.static_pad("src").unwrap();

            if let Some(peer) = bin_srcpad.peer() {
                bin_srcpad.unlink(&peer).unwrap();

                gst::info!(CAT, "Unlinked {}", peer.name());

                let compositor = pipeline.by_name("compositor").unwrap();
                compositor.release_request_pad(&peer);

                gst::info!(CAT, "Released pad {} from compositor", peer.name());
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

fn send_pipeline(pipeline: &gst::Pipeline, use_vp8: bool) {
    let video1 = video_bin(pipeline, "Stream 1".to_string(), use_vp8);
    let video2 = video_bin(pipeline, "Stream 2".to_string(), use_vp8);
    let video3 = video_bin(pipeline, "Datagram 1".to_string(), use_vp8);
    let video4 = video_bin(pipeline, "Datagram 2".to_string(), use_vp8);
    let roq = gst::ElementFactory::make("quinnroqmux")
        .name("roq-mux")
        .build()
        .unwrap();
    let queue = gst::ElementFactory::make("queue").build().unwrap();
    let sink = gst::ElementFactory::make("quinnquicsink")
        .name("quic-sink")
        .build()
        .unwrap();

    connect_overrun(pipeline, &queue, false);

    queue.set_property("max-size-buffers", 0u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 10 * gst::ClockTime::SECOND);

    sink.set_property("drop-buffer-for-datagram", true);
    sink.set_property("initial-mtu", 1200u32);
    sink.set_property("min-mtu", 1200u32);
    sink.set_property("upper-bound-mtu", 65527u32);
    sink.set_property("max-udp-payload-size", 65527u32);
    sink.set_property("use-datagram", false);
    sink.set_property("secure-connection", false);
    sink.set_property("server-name", "sanchayanmaity.net");

    pipeline
        .add_many([&video1, &video2, &video3, &video4])
        .unwrap();
    pipeline.add_many([&roq, &queue, &sink]).unwrap();

    let video1_roq_pad = roq.request_pad_simple("stream_%u").unwrap();
    let video2_roq_pad = roq.request_pad_simple("stream_%u").unwrap();
    let video3_roq_pad = roq.request_pad_simple("datagram_%u").unwrap();
    let video4_roq_pad = roq.request_pad_simple("datagram_%u").unwrap();

    video1_roq_pad.set_property("flow-id", 5u64);
    video2_roq_pad.set_property("flow-id", 6u64);
    video3_roq_pad.set_property("flow-id", 2345u64);
    video4_roq_pad.set_property("flow-id", 2346u64);

    let video1_pad = video1.static_pad("src").unwrap();
    let video2_pad = video2.static_pad("src").unwrap();
    let video3_pad = video3.static_pad("src").unwrap();
    let video4_pad = video4.static_pad("src").unwrap();

    video1_pad.link(&video1_roq_pad).unwrap();
    video2_pad.link(&video2_roq_pad).unwrap();
    video3_pad.link(&video3_roq_pad).unwrap();
    video4_pad.link(&video4_roq_pad).unwrap();

    roq.link(&queue).unwrap();
    queue.link(&sink).unwrap();
}

fn main() {
    let cli = Cli::parse();

    gst::init().unwrap();

    let pipeline = gst::Pipeline::new();
    let context = glib::MainContext::default();
    let main_loop = glib::MainLoop::new(Some(&context), false);

    let _ = pipeline.set_state(gst::State::Ready);

    if !cli.receiver {
        send_pipeline(&pipeline, cli.vp8);
        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "roq-send-pipeline");
    } else {
        receive_pipeline(&pipeline, cli.vp8);
        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), "quic-roq-recv-pipeline");
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
            "gst-roq-recv-pipeline"
        } else {
            "gst-roq-send-pipeline"
        };

        pipeline.debug_to_dot_file_with_ts(gst::DebugGraphDetails::all(), name);
    });

    let _ = pipeline.set_state(gst::State::Playing);

    main_loop.run();

    pipeline.set_state(gst::State::Null).unwrap();
}
