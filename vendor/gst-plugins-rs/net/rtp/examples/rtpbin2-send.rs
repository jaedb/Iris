// SPDX-License-Identifier: MPL-2.0

/// Sends one audio stream and one video stream in two separate RTP sessions.
/// Use `rtpbin2-recv` as a receiver.
use futures::pin_mut;
use futures::prelude::*;
use gst::prelude::*;

use std::{env, process};

const USAGE: &str = r#"Usage: rtpbin2-send [media-arg]

where 'media-arg' is either:

* -a or --audio-only: only stream audio
* -v or --video-only: only stream video
* by default, stream audio and video.
"#;

const RTP_ID: &str = "example-rtp-id";
const HOST: &str = "127.0.0.1";
const VIDEO_PT: u8 = 96;
const AUDIO_PT: u8 = 97;

#[derive(Debug)]
struct SessionParameters {
    session_id: u32,
    pt: u8,
    rtp_recv_port: u16,
    rtcp_recv_port: u16,
    rtcp_send_port: u16,
}

fn add_rtp_send(
    pipeline: &gst::Pipeline,
    rtpsend: &gst::Element,
    rtprecv: &gst::Element,
    src: &gst::Element,
    params: SessionParameters,
) {
    let sync = gst::ElementFactory::make("clocksync")
        .name(format!("clocksync-{}-{}", params.session_id, params.pt))
        .build()
        .unwrap();
    pipeline.add(&sync).unwrap();
    src.link(&sync).unwrap();

    sync.link_pads(
        Some("src"),
        rtpsend,
        Some(&format!("rtp_sink_{}", params.session_id)),
    )
    .unwrap();

    // RTP / RTCP to peer
    let rtp_queue = gst::ElementFactory::make("queue")
        .name(format!("queue-rtp-{}-{}", params.session_id, params.pt))
        .property("max-size-bytes", 0u32)
        .property("max-size-time", 100.mseconds())
        .property("max-size-buffers", 0u32)
        .build()
        .unwrap();
    pipeline.add(&rtp_queue).unwrap();

    let rtp_sink = gst::ElementFactory::make("udpsink")
        .name(format!("udpsink-rtp-{}-{}", params.session_id, params.pt))
        // sync of the RTP packets is handled before rtpsend
        .property("sync", false)
        .property("port", params.rtp_recv_port as i32)
        .property("host", HOST)
        .build()
        .unwrap();
    pipeline.add(&rtp_sink).unwrap();
    rtpsend
        .link_pads(
            Some(&format!("rtp_src_{}", params.session_id)),
            &rtp_queue,
            Some("sink"),
        )
        .unwrap();
    rtp_queue.link(&rtp_sink).unwrap();

    let rtcp_sink = gst::ElementFactory::make("udpsink")
        .name(format!("udpsink-rtcp-{}-{}", params.session_id, params.pt))
        // don't wait for the first RTCP packet for preroll
        .property("async", false)
        .property("port", params.rtcp_recv_port as i32)
        .property("host", HOST)
        .build()
        .unwrap();
    pipeline.add(&rtcp_sink).unwrap();
    rtpsend
        .link_pads(
            Some(&format!("rtcp_src_{}", params.session_id)),
            &rtcp_sink,
            Some("sink"),
        )
        .unwrap();

    // RTCP from peer
    let rtcp_src = gst::ElementFactory::make("udpsrc")
        .name(format!("udpsrc-rtcp-{}-{}", params.session_id, params.pt))
        .property("port", params.rtcp_send_port as i32)
        .property("caps", gst::Caps::new_empty_simple("application/x-rtcp"))
        .build()
        .unwrap();
    pipeline.add(&rtcp_src).unwrap();
    rtcp_src
        .link_pads(
            Some("src"),
            rtprecv,
            Some(&format!("rtcp_sink_{}", params.session_id)),
        )
        .unwrap();
}

#[tokio::main]
async fn main() -> process::ExitCode {
    let mut with_audio = true;
    let mut with_video = true;

    if let Some(media_arg) = env::args().nth(1) {
        match media_arg.as_str() {
            "--audio-only" | "-a" => with_video = false,
            "--video-only" | "-v" => with_audio = false,
            _ => {
                println!("{USAGE}");
                return process::ExitCode::FAILURE;
            }
        }
    }

    gst::init().unwrap();
    gstrsrtp::plugin_register_static().unwrap();

    let pipeline = gst::Pipeline::new();

    let rtpsend = gst::ElementFactory::make("rtpsend")
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtpsend).unwrap();

    let rtprecv = gst::ElementFactory::make("rtprecv")
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtprecv).unwrap();

    if with_video {
        println!("Adding video stream...");

        let video_src = gst::ElementFactory::make("videotestsrc")
            .property("is-live", true)
            .build()
            .unwrap();
        let video_enc = gst::ElementFactory::make("vp8enc")
            .property("deadline", 1i64)
            .build()
            .unwrap();
        let video_pay = gst::ElementFactory::make("rtpvp8pay2")
            .property("pt", VIDEO_PT as u32)
            .property_from_str("picture-id-mode", "7-bit")
            .build()
            .unwrap();
        let elems = [&video_src, &video_enc, &video_pay];
        pipeline.add_many(elems).unwrap();
        gst::Element::link_many(elems).unwrap();

        add_rtp_send(
            &pipeline,
            &rtpsend,
            &rtprecv,
            &video_pay,
            SessionParameters {
                session_id: 0,
                pt: VIDEO_PT,
                rtp_recv_port: 5004,
                rtcp_recv_port: 5005,
                rtcp_send_port: 5007,
            },
        );
    }

    if with_audio {
        println!("Adding audio stream...");

        let audio_src = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", true)
            .property("volume", 0.03f64)
            .build()
            .unwrap();
        let audio_enc = gst::ElementFactory::make("opusenc").build().unwrap();
        let audio_pay = gst::ElementFactory::make("rtpopuspay2")
            .property("pt", AUDIO_PT as u32)
            .build()
            .unwrap();
        let elems = [&audio_src, &audio_enc, &audio_pay];
        pipeline.add_many(elems).unwrap();
        gst::Element::link_many(elems).unwrap();

        add_rtp_send(
            &pipeline,
            &rtpsend,
            &rtprecv,
            &audio_pay,
            SessionParameters {
                session_id: 1,
                pt: AUDIO_PT,
                rtp_recv_port: 5008,
                rtcp_recv_port: 5009,
                rtcp_send_port: 5011,
            },
        );
    }

    pipeline.set_state(gst::State::Playing).unwrap();
    println!("Playing");

    let ctrl_c = tokio::signal::ctrl_c().fuse();
    pin_mut!(ctrl_c);

    let mut bus_send_stream = pipeline.bus().unwrap().stream();

    loop {
        futures::select_biased! {
            _ = ctrl_c => {
                println!("\nShutting down due to user request");
                break;
            }
            msg_send = bus_send_stream.next() => {
                use gst::MessageView::*;
                let Some(msg) = msg_send else { continue };
                match msg.view() {
                    Latency(_) => {
                        let _ = pipeline.recalculate_latency();
                    }
                    Error(err) => {
                        eprintln!("send pipeline: {err:?}");
                        break;
                    }
                    _ => (),
                }
            }
        };
    }

    pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "rtpbin2-send-stopping");
    pipeline.set_state(gst::State::Null).unwrap();

    // This is needed by some tracers to write their log file
    unsafe {
        gst::deinit();
    }

    process::ExitCode::SUCCESS
}
