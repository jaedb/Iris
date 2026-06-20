// Copyright (C) 2020 Markus Ebner <info@ebner-markus.de>
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
        gstgif::plugin_register_static().expect("gif test");
    });
}

#[test]
fn test_encode_rgba() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_encode(&video_info);
}
#[test]
fn test_encode_rgb() {
    init();

    let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    test_encode(&video_info);
}

fn test_encode(video_info: &gst_video::VideoInfo) {
    let mut h = gst_check::Harness::new("gifenc");
    h.set_src_caps(video_info.to_caps().unwrap());

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

    for _ in 0..6 {
        // last frame is the GIF trailer
        let buffer = h.pull().unwrap();
        // Currently, every frame should be a full frame
        assert!(!buffer.flags().contains(gst::BufferFlags::DELTA_UNIT))
    }
}

#[test]
fn test_no_frame_in_no_frame_out() {
    init();

    let mut h = gst_check::Harness::new("gifenc");

    // Start with 30fps
    let video_info_rgb = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    h.set_src_caps(video_info_rgb.to_caps().unwrap());

    // Change framerate to 60fps - this should NOT reset the encoder
    let video_info_argb = gst_video::VideoInfo::builder(gst_video::VideoFormat::Argb, 160, 120)
        .fps((60, 1))
        .build()
        .unwrap();
    h.set_src_caps(video_info_argb.to_caps().unwrap());

    h.push_event(gst::event::Eos::new());

    // Should get all frames + trailer
    let mut frame_count = 0;
    while let Some(_buffer) = h.try_pull() {
        frame_count += 1;
    }

    assert_eq!(
        frame_count, 0,
        "Ensure no frame in results in no frame out, got {} frames",
        frame_count
    );
}

#[test]
fn test_framerate_change_no_reset() {
    init();

    let mut h = gst_check::Harness::new("gifenc");

    // Start with 30fps
    let video_info_30fps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 160, 120)
        .fps((30, 1))
        .build()
        .unwrap();
    h.set_src_caps(video_info_30fps.to_caps().unwrap());

    // Push a few frames
    for pts in 0..3 {
        let buffer = {
            let mut buffer = gst::Buffer::with_size(video_info_30fps.size()).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(pts.seconds());
            }
            let mut vframe =
                gst_video::VideoFrame::from_buffer_writable(buffer, &video_info_30fps).unwrap();
            for v in vframe.plane_data_mut(0).unwrap() {
                *v = 128;
            }
            vframe.into_buffer()
        };
        h.push(buffer).unwrap();
    }

    // Change framerate to 60fps - this should NOT reset the encoder
    let video_info_60fps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 160, 120)
        .fps((60, 1))
        .build()
        .unwrap();
    h.set_src_caps(video_info_60fps.to_caps().unwrap());

    // Push more frames with the new framerate
    for pts in 3..6 {
        let buffer = {
            let mut buffer = gst::Buffer::with_size(video_info_60fps.size()).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(pts.seconds());
            }
            let mut vframe =
                gst_video::VideoFrame::from_buffer_writable(buffer, &video_info_60fps).unwrap();
            for v in vframe.plane_data_mut(0).unwrap() {
                *v = 192; // Different color to distinguish frames
            }
            vframe.into_buffer()
        };
        h.push(buffer).unwrap();
    }

    h.push_event(gst::event::Eos::new());

    // Should get all frames + trailer
    let mut frame_count = 0;
    while let Some(_buffer) = h.try_pull() {
        frame_count += 1;
    }

    // We expect 6 video frames + 1 trailer = 7 total
    // If the encoder was incorrectly reset, we might get additional trailer frames
    assert_eq!(
        frame_count, 7,
        "Expected 6 video frames + 1 trailer, got {} frames",
        frame_count
    );
}
