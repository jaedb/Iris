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
        crate::plugin_register_static().expect("rtpamr test");
    });
}

// 6 encoded frames of 32 bytes / 20ms
static AMR_NB_DATA: &[u8] = include_bytes!("test.amrnb");

fn get_amr_nb_data() -> (gst::Caps, Vec<gst::Buffer>) {
    let caps = gst::Caps::builder("audio/AMR")
        .field("rate", 8_000i32)
        .field("channels", 1i32)
        .build();

    let buffers = AMR_NB_DATA
        .chunks_exact(32)
        .enumerate()
        .map(|(idx, c)| {
            let mut buf = gst::Buffer::from_slice(c);

            {
                let buf = buf.get_mut().unwrap();
                buf.set_pts(idx as u64 * gst::ClockTime::from_mseconds(20));
                buf.set_duration(gst::ClockTime::from_mseconds(20));
                if idx == 0 {
                    buf.set_flags(gst::BufferFlags::DISCONT);
                }
            }

            buf
        })
        .collect();

    (caps, buffers)
}

// 4 encoded frames of 18 bytes / 20ms
static AMR_WB_DATA: &[u8] = include_bytes!("test.amrwb");

fn get_amr_wb_data() -> (gst::Caps, Vec<gst::Buffer>) {
    let caps = gst::Caps::builder("audio/AMR-WB")
        .field("rate", 16_000i32)
        .field("channels", 1i32)
        .build();

    let buffers = AMR_WB_DATA
        .chunks_exact(18)
        .enumerate()
        .map(|(idx, c)| {
            let mut buf = gst::Buffer::from_slice(c);

            {
                let buf = buf.get_mut().unwrap();
                buf.set_pts(idx as u64 * gst::ClockTime::from_mseconds(20));
                buf.set_duration(gst::ClockTime::from_mseconds(20));
                if idx == 0 {
                    buf.set_flags(gst::BufferFlags::DISCONT);
                }
            }

            buf
        })
        .collect();

    (caps, buffers)
}

#[test]
fn test_amr_nb() {
    init();

    let (caps, buffers) = get_amr_nb_data();
    let pay = "rtpamrpay2 aggregate-mode=zero-latency";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(160)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(320)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(480)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(100))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(800)
            .marker_bit(false)
            .size(45)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(32)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(100))
            .size(32)
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

#[test]
fn test_amr_nb_bit_packed() {
    init();

    let (caps, buffers) = get_amr_nb_data();
    let pay = "rtpamrpay2 aggregate-mode=zero-latency ! application/x-rtp,octet-align=(string)0";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(160)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(320)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(480)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(45)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(100))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(800)
            .marker_bit(false)
            .size(45)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(32)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(32)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(100))
            .size(32)
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

#[test]
fn test_amr_nb_aggregate() {
    init();

    let (caps, buffers) = get_amr_nb_data();
    let pay = "rtpamrpay2 max-ptime=40000000";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(77)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(320)
            .marker_bit(false)
            .size(77)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(77)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(64)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(64)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(64)
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

#[test]
fn test_amr_wb() {
    init();

    let (caps, buffers) = get_amr_wb_data();
    let pay = "rtpamrpay2 aggregate-mode=zero-latency";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(31)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(320)
            .marker_bit(false)
            .size(31)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(31)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960)
            .marker_bit(false)
            .size(31)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(18)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(18)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(18)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .size(18)
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

#[test]
fn test_amr_wb_bit_packed() {
    init();

    let (caps, buffers) = get_amr_wb_data();
    let pay = "rtpamrpay2 aggregate-mode=zero-latency ! application/x-rtp,octet-align=(string)0";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(30)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(320)
            .marker_bit(false)
            .size(30)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(30)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960)
            .marker_bit(false)
            .size(30)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(18)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(18)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(18)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(60))
            .size(18)
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

#[test]
fn test_amr_wb_aggregate() {
    init();

    let (caps, buffers) = get_amr_wb_data();
    let pay = "rtpamrpay2 max-ptime=40000000";
    let depay = "rtpamrdepay2";

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .size(49)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(640)
            .marker_bit(false)
            .size(49)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(0))
            .size(36)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(36)
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
