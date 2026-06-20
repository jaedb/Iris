// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrspng::plugin_register_static().expect("Failed to register rspng plugin");
    });
}

#[test]
fn test_png_encode_gray() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Gray8, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_png_encode(&video_info);
}

#[test]
fn test_png_encode_gray16() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Gray16Be, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_png_encode(&video_info);
}

#[test]
fn test_png_encode_rgb() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_png_encode(&video_info);
}

#[test]
fn test_png_encode_rgba() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_png_encode(&video_info);
}

fn test_png_encode(video_info: &gst_video::VideoInfo) {
    let mut h = gst_check::Harness::new("rspngenc");
    h.set_src_caps(video_info.to_caps().unwrap());
    h.play();

    for pts in 0..5 {
        let buffer = {
            let mut buffer = gst::Buffer::with_size(video_info.size()).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(pts.seconds());
            }
            let mut vframe =
                gst_video::VideoFrame::from_buffer_writable(buffer, video_info).unwrap();
            for v in vframe.plane_data_mut(0).unwrap() {
                *v = 128;
            }
            vframe.into_buffer()
        };
        h.push(buffer.clone()).unwrap();
    }
    h.push_event(gst::event::Eos::new());

    (0..5).for_each(|_| {
        let buffer = h.pull().unwrap();
        assert!(!buffer.flags().contains(gst::BufferFlags::DELTA_UNIT))
    });
}
