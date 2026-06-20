use gst::prelude::*;
use std::fmt::Display;

pub const QUIC_DATAGRAM_PROBE: &str = "quic-datagram-probe";
pub const QUIC_STREAM_ID: &str = "quic-stream-id";
pub const QUIC_STREAM_OPEN: &str = "quic-stream-open";
pub const QUIC_STREAM_PRIORITY: &str = "quic-stream-priority";
pub const QUIC_STREAM_TYPE: &str = "quic-stream-type";

pub const QUIC_STREAM_CLOSE_CUSTOMDOWNSTREAM_EVENT: &str = "GstQuinnQuicStreamClose";

#[derive(Debug)]
pub enum QuicStreamType {
    BIDI, /* Bi-directional stream */
    UNI,  /* Uni-directional stream */
}

impl Display for QuicStreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                QuicStreamType::BIDI => "bidi",
                QuicStreamType::UNI => "uni",
            }
        )
    }
}

fn query_datagram() -> gst::query::Custom<gst::Query> {
    let s = gst::Structure::builder(QUIC_DATAGRAM_PROBE).build();
    gst::query::Custom::new(s)
}

fn query_open_new_stream(
    stream_type: QuicStreamType,
    priority: i32,
) -> gst::query::Custom<gst::Query> {
    let s = gst::Structure::builder(QUIC_STREAM_OPEN)
        .field(QUIC_STREAM_TYPE, stream_type.to_string())
        .field(QUIC_STREAM_PRIORITY, priority)
        .build();
    gst::query::Custom::new(s)
}

pub fn request_datagram(srcpad: &gst::Pad) -> bool {
    let mut query = query_datagram();

    srcpad.peer_query(&mut query)
}

pub fn request_stream(srcpad: &gst::Pad, priority: i32) -> Option<u64> {
    // We do not support bi-directional streams yet
    let mut query = query_open_new_stream(QuicStreamType::UNI, priority);

    if srcpad.peer_query(&mut query) {
        if let Some(s) = query.structure() {
            s.get::<u64>(QUIC_STREAM_ID).ok()
        } else {
            None
        }
    } else {
        None
    }
}

pub fn close_stream(srcpad: &gst::Pad, stream_id: u64) -> bool {
    let s = gst::Structure::builder(QUIC_STREAM_CLOSE_CUSTOMDOWNSTREAM_EVENT)
        .field(QUIC_STREAM_ID, stream_id)
        .build();
    let event = gst::event::CustomDownstream::new(s);

    srcpad.push_event(event)
}
