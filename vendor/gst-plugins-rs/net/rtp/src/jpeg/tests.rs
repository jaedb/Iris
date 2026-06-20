//
// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::tests::{run_test_pipeline, ExpectedBuffer, ExpectedPacket, Source};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpvp8 test");
    });
}

#[test]
fn test_jpeg() {
    init();

    // Generates encoded frames of size 5409 bytes
    let src = "videotestsrc num-buffers=2 pattern=black ! video/x-raw,format=I420,width=640,height=480,framerate=25/1 ! jpegenc ! jpegparse";
    let pay = "rtpjpegpay2";
    let depay = "rtpjpegdepay2";

    let expected_pay = vec![
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::DISCONT)
                .pt(26)
                .rtp_time(0)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(26)
                .rtp_time(0)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(26)
                .rtp_time(0)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::MARKER)
                .pt(26)
                .rtp_time(0)
                .marker_bit(true)
                .size(684)
                .build(),
        ],
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(26)
                .rtp_time(3600)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(26)
                .rtp_time(3600)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(26)
                .rtp_time(3600)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::MARKER)
                .pt(26)
                .rtp_time(3600)
                .marker_bit(true)
                .size(684)
                .build(),
        ],
    ];

    let expected_depay = vec![
        // One buffer per frame
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(5409)
            .flags(gst::BufferFlags::DISCONT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(5409)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}
