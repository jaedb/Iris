// Copyright (C) 2024 Thibault Saunier <tsaunier@igalia.com>
#![allow(clippy::single_match)]

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstskia::plugin_register_static().expect("skia test");
    });
}

#[test]
fn test_simple() {
    init();

    let mut h = gst_check::Harness::with_padnames("skiacompositor", Some("sink_0"), Some("src"));

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, 160, 120)
        .fps((10, 1))
        .build()
        .unwrap();
    let caps = video_info.to_caps().unwrap();

    h.set_sink_caps(caps.clone());
    h.set_src_caps(caps);

    for pts in 0..5 {
        let buffer = {
            let mut buffer = gst::Buffer::with_size(video_info.size()).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(pts.seconds());
            }
            let mut vframe =
                gst_video::VideoFrame::from_buffer_writable(buffer, &video_info).unwrap();
            for v in vframe.plane_data_mut(0).unwrap() {
                *v = 128;
            }
            vframe.into_buffer()
        };
        h.push(buffer.clone()).unwrap();
    }
    h.push_event(gst::event::Eos::new());

    for i in 0..6 {
        let buffer = h.pull().unwrap();
        assert_eq!(
            buffer.pts(),
            Some(gst::ClockTime::from_seconds_f64(1. / 10. * i as f64))
        )
    }
}
