//
// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
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
        crate::plugin_register_static().expect("rtp linear audio test");
    });
}

#[test]
fn test_l16() {
    init();

    let src =
        "audiotestsrc num-buffers=5 samplesperbuffer=480 ! capsfilter caps=audio/x-raw,rate=48000";
    let pay = "rtpL16pay2";
    let depay = "rtpL16depay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(10))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(480)
            .marker_bit(false)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(2 * 480)
            .marker_bit(false)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(30))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(3 * 480)
            .marker_bit(false)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(4 * 480)
            .marker_bit(false)
            .size(972)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(960)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(10))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(30))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn test_l16_marker_bit() {
    init();

    let info = gst_audio::AudioInfo::builder(gst_audio::AudioFormat::S16be, 48000, 1)
        .build()
        .unwrap();
    let caps = info.to_caps().unwrap();

    let mut buffers = vec![];
    for i in 0..5 {
        let mut buffer = gst::Buffer::from_mut_slice([0u8; 960]);
        let buffer_ref = buffer.get_mut().unwrap();
        buffer_ref.set_pts(i * gst::ClockTime::from_mseconds(10));
        if i == 3 {
            buffer_ref.set_flags(gst::BufferFlags::RESYNC);
        }
        buffers.push(buffer);
    }

    let pay = "rtpL16pay2";
    let depay = "rtpL16depay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(10))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(480)
            .marker_bit(false)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(2 * 480)
            .marker_bit(false)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(30))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(3 * 480)
            .marker_bit(true)
            .size(972)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(4 * 480)
            .marker_bit(false)
            .size(972)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(960)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(10))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(30))
            .size(960)
            .flags(gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(960)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(caps, buffers),
        pay,
        depay,
        expected_pay,
        expected_depay,
    );
}
