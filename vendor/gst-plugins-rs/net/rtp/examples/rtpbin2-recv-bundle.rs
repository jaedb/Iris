// SPDX-License-Identifier: MPL-2.0

/// Receives one audio stream and one video stream bundled in a single RTP session.
/// Use `rtpbin2-send-bundle` as a receiver.
use futures::pin_mut;
use futures::prelude::*;
use gst::prelude::*;
use itertools::Itertools as _;

const RTP_ID: &str = "example-rtp-id";
const SESSION_ID: u32 = 0;
const VIDEO_PT: u8 = 96;
const AUDIO_PT: u8 = 97;

const HOST: &str = "127.0.0.1";
const RECV_PORT: u16 = 5004;
const SEND_PORT: u16 = 5005;

const LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(200);

#[tokio::main]
async fn main() {
    gst::init().unwrap();
    gstrsrtp::plugin_register_static().unwrap();

    let pipeline = gst::Pipeline::new();

    let rtprecv = gst::ElementFactory::make("rtprecv")
        .name(format!("rtprecv-{SESSION_ID}"))
        .property("rtp-id", RTP_ID)
        .property("latency", LATENCY.mseconds() as u32)
        .build()
        .unwrap();
    pipeline.add(&rtprecv).unwrap();

    let _recv_rtp_sink_pad = rtprecv
        .request_pad_simple(&format!("rtp_sink_{SESSION_ID}"))
        .unwrap();

    let session = rtprecv.emit_by_name::<gst::glib::Object>("get-session", &[&SESSION_ID]);
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

    rtprecv.connect_pad_added({
        |recv, pad| {
            let pad_name = pad.name();
            if !pad_name.starts_with("rtp_src_") {
                return;
            }

            let (_prefix, _direction, session_id, pt, _ssrc) =
                pad_name.split('_').collect_tuple().unwrap();
            let _session_id = session_id.parse::<u32>().unwrap();
            let pt = pt.parse::<u8>().unwrap();

            println!("Adding src pad: {pad_name}");

            let parent = recv.parent().unwrap().downcast::<gst::Bin>().unwrap();
            match pt {
                VIDEO_PT => {
                    let depay_queue = gst::ElementFactory::make("queue")
                        .name("video-depay-queue")
                        .property("max-size-bytes", 0u32)
                        .property("max-size-time", 2.seconds() + 50.mseconds())
                        .property("max-size-time", gst::ClockTime::ZERO)
                        .property("max-size-buffers", 0u32)
                        .property_from_str("leaky", "downstream")
                        .build()
                        .unwrap();
                    let depay = gst::ElementFactory::make("rtpvp8depay2").build().unwrap();
                    let dec = gst::ElementFactory::make("vp8dec").build().unwrap();
                    let conv = gst::ElementFactory::make("videoconvert").build().unwrap();
                    let sink_queue = gst::ElementFactory::make("queue")
                        .name("video-sink-queue")
                        .property("max-size-bytes", 0u32)
                        .property("max-size-time", gst::ClockTime::ZERO)
                        .property("max-size-buffers", 1u32)
                        .build()
                        .unwrap();
                    let sink = gst::ElementFactory::make("autovideosink").build().unwrap();
                    let elems = [&depay_queue, &depay, &dec, &conv, &sink_queue, &sink];
                    parent.add_many(elems).unwrap();
                    pad.link(&depay_queue.static_pad("sink").unwrap()).unwrap();
                    gst::Element::link_many(elems).unwrap();

                    sink.sync_state_with_parent().unwrap();
                    sink_queue.sync_state_with_parent().unwrap();
                    conv.sync_state_with_parent().unwrap();
                    dec.sync_state_with_parent().unwrap();
                    depay.sync_state_with_parent().unwrap();
                    depay_queue.sync_state_with_parent().unwrap();
                }
                AUDIO_PT => {
                    let depay_queue = gst::ElementFactory::make("queue")
                        .name("audio-depay-queue")
                        .property("max-size-bytes", 0u32)
                        // This queue needs to be big enough for holding enough
                        // audio until a keyframe is decoded on the video branch
                        // or otherwise the pipeline might get stuck prerolling.
                        //
                        // Make it twice the keyframe interval for safety.
                        //
                        // The alternative would be to use a leaky queue or to use
                        // async=false on the sink, or a combination of that.
                        .property("max-size-time", 2.seconds() + 50.mseconds())
                        .property("max-size-buffers", 0u32)
                        .property_from_str("leaky", "downstream")
                        .build()
                        .unwrap();
                    let depay = gst::ElementFactory::make("rtpopusdepay2").build().unwrap();
                    let dec = gst::ElementFactory::make("opusdec").build().unwrap();
                    let conv = gst::ElementFactory::make("audioconvert").build().unwrap();
                    let sink_queue = gst::ElementFactory::make("queue")
                        .name("audio-sink-queue")
                        .property("max-size-bytes", 0u32)
                        .property("max-size-time", gst::ClockTime::ZERO)
                        .property("max-size-buffers", 1u32)
                        .build()
                        .unwrap();
                    let sink = gst::ElementFactory::make("autoaudiosink").build().unwrap();

                    let elems = [&depay_queue, &depay, &dec, &conv, &sink_queue, &sink];
                    parent.add_many(elems).unwrap();
                    pad.link(&depay_queue.static_pad("sink").unwrap()).unwrap();
                    gst::Element::link_many(elems).unwrap();

                    sink.sync_state_with_parent().unwrap();
                    sink_queue.sync_state_with_parent().unwrap();
                    conv.sync_state_with_parent().unwrap();
                    dec.sync_state_with_parent().unwrap();
                    depay.sync_state_with_parent().unwrap();
                    depay_queue.sync_state_with_parent().unwrap();
                }
                other => eprintln!("Unexpected PT {other:?} in pad name {pad_name}"),
            }
        }
    });

    // RTP / RTCP from peer
    let src = gst::ElementFactory::make("udpsrc")
        .name(format!("udpsrc-{SESSION_ID}"))
        .property("port", RECV_PORT as i32)
        .property("caps", gst::Caps::new_empty_simple("application/x-rtp"))
        .build()
        .unwrap();
    pipeline.add(&src).unwrap();
    let queue_rtp = gst::ElementFactory::make("queue")
        .name(format!("queue-rtp-{SESSION_ID}"))
        .property("max-size-bytes", 0u32)
        .property("max-size-time", LATENCY + 50.mseconds())
        .property("max-size-buffers", 0u32)
        .build()
        .unwrap();
    pipeline.add(&queue_rtp).unwrap();
    src.link(&queue_rtp).unwrap();
    queue_rtp
        .link_pads(
            Some("src"),
            &rtprecv,
            Some(&format!("rtp_sink_{SESSION_ID}")),
        )
        .unwrap();

    // RTCP to peer
    let rtpsend = gst::ElementFactory::make("rtpsend")
        .name("send")
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtpsend).unwrap();

    let sink = gst::ElementFactory::make("udpsink")
        .name(format!("udpsink-{SESSION_ID}"))
        // don't wait for the first RTCP packet for preroll
        .property("async", false)
        .property("port", SEND_PORT as i32)
        .property("host", HOST)
        .build()
        .unwrap();
    pipeline.add(&sink).unwrap();
    rtpsend
        .link_pads(Some(&format!("rtcp_src_{SESSION_ID}")), &sink, Some("sink"))
        .unwrap();

    // Share the same socket in source and sink
    src.set_state(gst::State::Ready).unwrap();
    let socket = src.property::<glib::Object>("used-socket");
    sink.set_property("socket", socket);

    pipeline.set_state(gst::State::Playing).unwrap();
    println!("Playing");

    let ctrl_c = tokio::signal::ctrl_c().fuse();
    pin_mut!(ctrl_c);

    let mut bus_recv_stream = pipeline.bus().unwrap().stream();

    loop {
        futures::select_biased! {
            _ = ctrl_c => {
                println!("\nShutting down due to user request");
                break;
            }
            msg_recv = bus_recv_stream.next() => {
                use gst::MessageView::*;
                let Some(msg) = msg_recv else { continue };
                match msg.view() {
                    Latency(_) => {
                        let _ = pipeline.recalculate_latency();
                    }
                    Eos(_) => {
                        println!("Got EoS");
                        break;
                    }
                    Error(err) => {
                        eprintln!("recv pipeline: {err:?}");
                        break;
                    }
                    _ => (),
                }
            }
        };
    }

    pipeline.debug_to_dot_file(
        gst::DebugGraphDetails::all(),
        "rtpbin2-recv-bundle-stopping",
    );
    pipeline.set_state(gst::State::Null).unwrap();

    // This is needed by some tracers to write their log file
    unsafe {
        gst::deinit();
    }
}
