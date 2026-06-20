// SPDX-License-Identifier: MPL-2.0

/// Sends one audio stream and one video stream bundled in a single RTP session.
/// Use `rtpbin2-recv-bundle` as a receiver.
use futures::pin_mut;
use futures::prelude::*;
use gst::prelude::*;

const RTP_ID: &str = "example-rtp-id";
const SESSION_ID: u32 = 1234;
const VIDEO_PT: u8 = 96;
const AUDIO_PT: u8 = 97;

const HOST: &str = "127.0.0.1";
const SEND_PORT: u16 = 5004;
const RECV_PORT: u16 = 5005;

#[tokio::main]
async fn main() {
    gst::init().unwrap();
    gstrsrtp::plugin_register_static().unwrap();

    let pipeline = gst::Pipeline::new();

    let rtpsend = gst::ElementFactory::make("rtpsend")
        .name(format!("rtpsend-{SESSION_ID}"))
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtpsend).unwrap();

    let funnel = gst::ElementFactory::make("rtpfunnel")
        .name(format!("rtpfunnel-{SESSION_ID}"))
        .build()
        .unwrap();
    pipeline.add(&funnel).unwrap();
    funnel
        .link_pads(
            Some("src"),
            &rtpsend,
            Some(&format!("rtp_sink_{SESSION_ID}")),
        )
        .unwrap();

    let session = rtpsend.emit_by_name::<gst::glib::Object>("get-session", &[&SESSION_ID]);
    let pt_map = gst::Structure::builder("application/x-rtp2-pt-map")
        .field(
            VIDEO_PT.to_string(),
            gst::Caps::builder("application/x-rtp")
                .field("media", "video")
                .field("payload", VIDEO_PT as i32)
                .field("clock-rate", 90_000i32)
                .field("encoding-name", "VP8")
                .build(),
        )
        .field(
            AUDIO_PT.to_string(),
            gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("payload", AUDIO_PT as i32)
                .field("clock-rate", 48_000i32)
                .field("encoding-name", "OPUS")
                .build(),
        )
        .build();
    session.set_property("pt-map", pt_map);

    let video_src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .build()
        .unwrap();
    let video_enc = gst::ElementFactory::make("vp8enc")
        .property("deadline", 1i64)
        // 1s keyframe interval (30 frames at 30fps)
        .property("keyframe-max-dist", 30i32)
        .build()
        .unwrap();
    let video_pay = gst::ElementFactory::make("rtpvp8pay2")
        .property("pt", VIDEO_PT as u32)
        .property_from_str("picture-id-mode", "7-bit")
        .build()
        .unwrap();
    let video_sync = gst::ElementFactory::make("clocksync")
        .name("video-sync")
        .build()
        .unwrap();
    let elems = [&video_src, &video_enc, &video_pay, &video_sync];
    pipeline.add_many(elems).unwrap();
    gst::Element::link_many(elems).unwrap();
    video_sync
        .link_pads(Some("src"), &funnel, Some("sink_%u"))
        .unwrap();

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
    let audio_sync = gst::ElementFactory::make("clocksync")
        .name("audio-sync")
        .build()
        .unwrap();
    let elems = [&audio_src, &audio_enc, &audio_pay, &audio_sync];
    pipeline.add_many(elems).unwrap();
    gst::Element::link_many(elems).unwrap();
    audio_sync
        .link_pads(Some("src"), &funnel, Some("sink_%u"))
        .unwrap();

    // RTP / RTCP to peer
    let funnel = gst::ElementFactory::make("funnel")
        .name(format!("funnel-{SESSION_ID}"))
        .build()
        .unwrap();
    pipeline.add(&funnel).unwrap();

    let sink = gst::ElementFactory::make("udpsink")
        .name(format!("udpsink-{SESSION_ID}"))
        // sync of the RTP packets is handled before the rtpfunnel
        .property("sync", false)
        .property("port", SEND_PORT as i32)
        .property("host", HOST)
        .build()
        .unwrap();
    pipeline.add(&sink).unwrap();

    funnel.link(&sink).unwrap();

    let rtp_queue = gst::ElementFactory::make("queue")
        .name(format!("queue-rtp-{SESSION_ID}"))
        .property("max-size-bytes", 0u32)
        .property("max-size-time", 100.mseconds())
        .property("max-size-buffers", 0u32)
        .build()
        .unwrap();
    pipeline.add(&rtp_queue).unwrap();

    rtpsend
        .link_pads(
            Some(&format!("rtp_src_{SESSION_ID}")),
            &rtp_queue,
            Some("sink"),
        )
        .unwrap();

    rtp_queue
        .link_pads(Some("src"), &funnel, Some("sink_0"))
        .unwrap();

    rtpsend
        .link_pads(
            Some(&format!("rtcp_src_{SESSION_ID}")),
            &funnel,
            Some("sink_1"),
        )
        .unwrap();

    // RTCP from peer
    let rtprecv = gst::ElementFactory::make("rtprecv")
        .name(format!("rtprecv-{SESSION_ID}"))
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtprecv).unwrap();

    let src = gst::ElementFactory::make("udpsrc")
        .name(format!("udpsrc-{SESSION_ID}"))
        .property("port", RECV_PORT as i32)
        .property("caps", gst::Caps::new_empty_simple("application/x-rtcp"))
        .build()
        .unwrap();
    pipeline.add(&src).unwrap();
    src.link_pads(
        Some("src"),
        &rtprecv,
        Some(&format!("rtcp_sink_{SESSION_ID}")),
    )
    .unwrap();

    // Share the same socket in source and sink
    src.set_state(gst::State::Ready).unwrap();
    let socket = src.property::<glib::Object>("used-socket");
    sink.set_property("socket", socket);

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

    pipeline.debug_to_dot_file(
        gst::DebugGraphDetails::all(),
        "rtpbin2-send-bundle-stopping",
    );
    pipeline.set_state(gst::State::Null).unwrap();

    // This is needed by some tracers to write their log file
    unsafe {
        gst::deinit();
    }
}
