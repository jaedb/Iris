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
fn test_vp8() {
    init();

    // Generates encoded frames of sizes 1915 (key), 110, 103, 100, 100
    let src = "videotestsrc num-buffers=5 pattern=smpte100 ! video/x-raw,format=I420,width=1280,height=720,framerate=25/1 ! vp8enc target-bitrate=4000000";
    let pay = "rtpvp8pay2 picture-id-mode=7-bit";
    let depay = "rtpvp8depay2";

    let expected_pay = vec![
        vec![
            // First frame is split into two packets
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::DISCONT)
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(1400)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(0)
                .marker_bit(true)
                .size(545)
                .build(),
        ],
        // Second and following frames
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(3_600)
            .marker_bit(true)
            .size(125)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(7_200)
            .marker_bit(true)
            .size(118)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(120))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(10_800)
            .marker_bit(true)
            .size(115)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(160))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(14_400)
            .marker_bit(true)
            .size(115)
            .build()],
    ];

    let expected_depay = vec![
        // One buffer per frame
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(1915)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(110)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(103)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(120))
            .size(100)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(160))
            .size(100)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
    ];

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn test_vp8_small_mtu() {
    init();

    // Generates encoded frames of sizes 1915 (key), 110, 103, 100, 100
    let src = "videotestsrc num-buffers=5 pattern=smpte100 ! video/x-raw,format=I420,width=1280,height=720,framerate=25/1 ! vp8enc target-bitrate=4000000";
    let pay = "rtpvp8pay2 mtu=800 picture-id-mode=15-bit";
    let depay = "rtpvp8depay2";

    let expected_pay = vec![
        vec![
            // First frame is split into three packets
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::DISCONT)
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(800)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(800)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(0)
                .marker_bit(true)
                .size(363)
                .build(),
        ],
        // Second and following frames
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(3_600)
            .marker_bit(true)
            .size(126)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(7_200)
            .marker_bit(true)
            .size(119)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(120))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(10_800)
            .marker_bit(true)
            .size(116)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(160))
            .flags(gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(14_400)
            .marker_bit(true)
            .size(116)
            .build()],
    ];

    let expected_depay = vec![
        // One buffer per frame
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(1915)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(110)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(103)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(120))
            .size(100)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(160))
            .size(100)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
    ];

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn test_vp8_partitions() {
    init();

    // Generates encoded frames of sizes 1927 (key), 122, 115, 112, 112
    let src = "videotestsrc num-buffers=5 pattern=smpte100 ! video/x-raw,format=I420,width=1280,height=720,framerate=25/1 ! vp8enc token-partitions=4 target-bitrate=4000000";
    let pay = "rtpvp8pay2 mtu=800 fragmentation-mode=every-partition picture-id-mode=15-bit";
    let depay = "rtpvp8depay2";

    let expected_pay = vec![
        vec![
            // First frame is split into seven packets (3 first partition, 4 for the other partitions)
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::DISCONT)
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(800)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(800)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(190)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(187)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(28)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(0)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(0))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(0)
                .marker_bit(true)
                .size(17)
                .build(),
        ],
        // Second and following frames are all split into 5 packets, one per partition
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(3_600)
                .marker_bit(false)
                .size(129)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(3_600)
                .marker_bit(false)
                .size(22)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(3_600)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(3_600)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(40))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(3_600)
                .marker_bit(true)
                .size(17)
                .build(),
        ],
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(80))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(7_200)
                .marker_bit(false)
                .size(125)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(80))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(7_200)
                .marker_bit(false)
                .size(19)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(80))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(7_200)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(80))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(7_200)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(80))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(7_200)
                .marker_bit(true)
                .size(17)
                .build(),
        ],
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(120))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(10_800)
                .marker_bit(false)
                .size(124)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(120))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(10_800)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(120))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(10_800)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(120))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(10_800)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(120))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(10_800)
                .marker_bit(true)
                .size(17)
                .build(),
        ],
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(160))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(14_400)
                .marker_bit(false)
                .size(124)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(160))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(14_400)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(160))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(14_400)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(160))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(14_400)
                .marker_bit(false)
                .size(17)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(160))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(14_400)
                .marker_bit(true)
                .size(17)
                .build(),
        ],
    ];

    let expected_depay = vec![
        // One buffer per frame
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(1927)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(122)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(115)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(120))
            .size(112)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(160))
            .size(112)
            .flags(gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT)
            .build()],
    ];

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}
