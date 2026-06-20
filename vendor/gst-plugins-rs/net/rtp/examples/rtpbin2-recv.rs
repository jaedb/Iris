// SPDX-License-Identifier: MPL-2.0

/// Receives one audio stream and one video stream from two separate RTP sessions.
/// Use `rtpbin2-send` as a sender.
use futures::pin_mut;
use futures::prelude::*;
use gst::prelude::*;
use itertools::Itertools as _;

const RTP_ID: &str = "example-rtp-id";
const HOST: &str = "127.0.0.1";
const VIDEO_PT: u8 = 96;
const AUDIO_PT: u8 = 97;

const LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(200);

struct SessionParameters {
    session_id: u32,
    pt: u8,
    caps: gst::Caps,
    rtp_recv_port: u16,
    rtcp_recv_port: u16,
    rtcp_send_port: u16,
    #[allow(clippy::type_complexity)]
    on_pad_added: Box<dyn Fn(&gst::Pipeline, &gst::Pad) + Send + Sync + 'static>,
}

fn add_rtp_recv(
    pipeline: &gst::Pipeline,
    rtprecv: &gst::Element,
    rtpsend: &gst::Element,
    params: &SessionParameters,
) {
    let _recv_rtp_sink_pad = rtprecv
        .request_pad_simple(&format!("rtp_sink_{}", params.session_id))
        .unwrap();

    // RTP / RTCP from peer
    let rtp_src = gst::ElementFactory::make("udpsrc")
        .name(format!("udpsrc-rtp-{}-{}", params.session_id, params.pt))
        .property("port", params.rtp_recv_port as i32)
        .property("caps", &params.caps)
        .build()
        .unwrap();
    pipeline.add(&rtp_src).unwrap();
    let queue_rtp = gst::ElementFactory::make("queue")
        .name(format!("queue-rtp-{}-{}", params.session_id, params.pt))
        .property("max-size-bytes", 0u32)
        .property("max-size-time", LATENCY + 50.mseconds())
        .property("max-size-buffers", 0u32)
        .build()
        .unwrap();
    pipeline.add(&queue_rtp).unwrap();
    rtp_src.link(&queue_rtp).unwrap();
    queue_rtp
        .link_pads(
            Some("src"),
            rtprecv,
            Some(&format!("rtp_sink_{}", params.session_id)),
        )
        .unwrap();

    let rtcp_src = gst::ElementFactory::make("udpsrc")
        .name(format!("udpsrc-rtcp-{}-{}", params.session_id, params.pt))
        .property("port", params.rtcp_recv_port as i32)
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

    // RTCP to peer
    let rtcp_sink = gst::ElementFactory::make("udpsink")
        .name(format!("udpsink-rtcp-{}-{}", params.session_id, params.pt))
        // don't wait for the first RTCP packet for preroll
        .property("async", false)
        .property("port", params.rtcp_send_port as i32)
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
}

#[tokio::main]
async fn main() {
    gst::init().unwrap();
    gstrsrtp::plugin_register_static().unwrap();

    let pipeline = gst::Pipeline::new();

    let rtprecv = gst::ElementFactory::make("rtprecv")
        .property("rtp-id", RTP_ID)
        .property("latency", LATENCY.mseconds() as u32)
        .build()
        .unwrap();
    pipeline.add(&rtprecv).unwrap();

    let rtpsend = gst::ElementFactory::make("rtpsend")
        .property("rtp-id", RTP_ID)
        .build()
        .unwrap();
    pipeline.add(&rtpsend).unwrap();

    let sessions = [
        SessionParameters {
            session_id: 0,
            pt: VIDEO_PT,
            caps: gst::Caps::builder("application/x-rtp")
                .field("media", "video")
                .field("payload", VIDEO_PT as i32)
                .field("clock-rate", 90_000i32)
                .field("encoding-name", "VP8")
                .build(),
            rtp_recv_port: 5004,
            rtcp_recv_port: 5005,
            rtcp_send_port: 5007,
            on_pad_added: Box::new(|pipeline, pad| {
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

                let elems = [&depay, &dec, &conv, &sink_queue, &sink];
                pipeline.add_many(elems).unwrap();
                pad.link(&depay.static_pad("sink").unwrap()).unwrap();
                gst::Element::link_many(elems).unwrap();

                sink.sync_state_with_parent().unwrap();
                sink_queue.sync_state_with_parent().unwrap();
                conv.sync_state_with_parent().unwrap();
                dec.sync_state_with_parent().unwrap();
                depay.sync_state_with_parent().unwrap();
            }),
        },
        SessionParameters {
            session_id: 1,
            pt: AUDIO_PT,
            caps: gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("payload", AUDIO_PT as i32)
                .field("clock-rate", 48_000i32)
                .field("encoding-name", "OPUS")
                .build(),
            rtp_recv_port: 5008,
            rtcp_recv_port: 5009,
            rtcp_send_port: 5011,
            on_pad_added: Box::new(|pipeline, pad| {
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

                let elems = [&depay, &dec, &conv, &sink_queue, &sink];
                pipeline.add_many(elems).unwrap();
                pad.link(&depay.static_pad("sink").unwrap()).unwrap();
                gst::Element::link_many(elems).unwrap();

                sink.sync_state_with_parent().unwrap();
                sink_queue.sync_state_with_parent().unwrap();
                conv.sync_state_with_parent().unwrap();
                dec.sync_state_with_parent().unwrap();
                depay.sync_state_with_parent().unwrap();
            }),
        },
    ];

    add_rtp_recv(&pipeline, &rtprecv, &rtpsend, &sessions[0]);
    add_rtp_recv(&pipeline, &rtprecv, &rtpsend, &sessions[1]);

    rtprecv.connect_pad_added(move |recv, pad| {
        let pad_name = pad.name();
        if !pad_name.starts_with("rtp_src_") {
            return;
        }

        let (_prefix, _direction, session_id, pt, ssrc) =
            pad_name.split('_').collect_tuple().unwrap();
        let session_id = session_id.parse::<u32>().unwrap();
        let pt = pt.parse::<u8>().unwrap();

        let Some(session) = sessions
            .iter()
            .find(|session| session.session_id == session_id && session.pt == pt)
        else {
            eprintln!("Unknown RTP stream with session ID {session_id}, PT {pt} and SSRC {ssrc}");
            return;
        };

        let parent = recv.parent().unwrap();
        (session.on_pad_added)(parent.downcast_ref::<gst::Pipeline>().unwrap(), pad);
    });

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

    pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "rtpbin2-recv-stopping");
    pipeline.set_state(gst::State::Null).unwrap();

    // This is needed by some tracers to write their log file
    unsafe {
        gst::deinit();
    }
}
