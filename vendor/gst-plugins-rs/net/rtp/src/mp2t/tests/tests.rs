// GStreamer RTP MPEG-TS Payloader / Depayloader - unit tests
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
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
        crate::plugin_register_static().expect("rtpmp2t test");
    });
}

// gst-launch-1.0 videotestsrc num-buffers=3 \
//  ! video/x-raw,format=I420,width=80,height=60,framerate=25/1 \
//  ! x264enc tune=zerolatency \
//  ! mpegtsmux \
//  ! filesink location=videotestsrc-80x60-h264.ts
//
// - 15 packets with timestamp  0ms
// -  4 packets with timestamp 40ms
// -  4 packets with timestamp 80ms
//
const MPEGTS_DATA: &[u8] = include_bytes!("videotestsrc-80x60-h264.ts").as_slice();

// PACKET_SIZE = 188
fn make_mp2t_buffer(
    packet_number: usize,
    n_packets: usize,
    pts: gst::ClockTime,
    flags: gst::BufferFlags,
) -> gst::Buffer {
    const PACKET_SIZE: usize = 188;

    let mut ts_packet = MPEGTS_DATA[(packet_number * PACKET_SIZE)..].to_vec();

    assert!(ts_packet.starts_with(&[0x47]));

    ts_packet.truncate(n_packets * PACKET_SIZE);

    // Add filler packets if needed
    while ts_packet.len() < (n_packets * PACKET_SIZE) {
        ts_packet.extend_from_slice(&[0x47, 0x1f, 0xff, 0x10]);

        while ts_packet.len() % PACKET_SIZE != 0 {
            ts_packet.extend_from_slice(&[0, 0, 0, 0]);
        }
    }

    let mut buf = gst::Buffer::from_mut_slice(ts_packet);

    if let Some(buf_ref) = buf.get_mut() {
        buf_ref.set_pts(pts);
        buf_ref.set_flags(flags);
    }

    buf
}

#[test]
fn test_mp2t_pay_depay_single_ts_packets() {
    init();

    // mpegtsmux would first push these caps and then update with a streamheader
    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 188i32)
        .build();

    let mut input_buffers = Vec::with_capacity(24);

    // No DISCONT flag on first buffer for some reason..
    // - 15 packets with timestamp  0ms
    // -  4 packets with timestamp 40ms
    // -  4 packets with timestamp 80ms
    for i in 0..23 {
        input_buffers.push(make_mp2t_buffer(
            0,
            1,
            match i {
                0..=14 => gst::ClockTime::ZERO,
                15..=18 => gst::ClockTime::from_mseconds(40),
                19..=23 => gst::ClockTime::from_mseconds(80),
                _ => unreachable!(),
            },
            if i == 0 {
                gst::BufferFlags::empty()
            } else {
                gst::BufferFlags::DELTA_UNIT
            },
        ));
    }

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(33)
            .rtp_time(0)
            .marker_bit(true)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(7200) // 80ms, 0.080 * 90000
            .marker_bit(false)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(376)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2",
        "rtpmp2tdepay2",
        expected_pay,
        expected_depay,
    );
}

#[test]
fn test_mp2t_pay_depay_7ts_packets() {
    init();

    // mpegtsmux would first push these caps and then update with a streamheader
    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 188i32)
        .build();

    let input_buffers = vec![
        // This time we're not feeding single ts packets, but 7 per buffer (like alignment=7)
        make_mp2t_buffer(0, 7, gst::ClockTime::ZERO, gst::BufferFlags::empty()),
        make_mp2t_buffer(7, 7, gst::ClockTime::ZERO, gst::BufferFlags::DELTA_UNIT),
        make_mp2t_buffer(14, 7, gst::ClockTime::ZERO, gst::BufferFlags::DELTA_UNIT),
        make_mp2t_buffer(
            21,
            7,
            gst::ClockTime::from_mseconds(80),
            gst::BufferFlags::DELTA_UNIT,
        ),
    ];

    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(33)
            .rtp_time(0)
            .marker_bit(true)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(7200) // 80ms, 0.080 * 90000
            .marker_bit(false)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(1316) // includes padding packets
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2",
        "rtpmp2tdepay2",
        expected_pay,
        expected_depay,
    );
}

#[test]
fn test_mp2t_pay_depay_7ts_packets_mtu_split() {
    init();

    // mpegtsmux would first push these caps and then update with a streamheader
    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 188i32)
        .build();

    let input_buffers = vec![
        // This time we're not feeding single ts packets, but 7 per buffer (like alignment=7)
        make_mp2t_buffer(0, 7, gst::ClockTime::ZERO, gst::BufferFlags::empty()),
    ];

    let expected_pay = vec![vec![
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(33)
            .rtp_time(0)
            .marker_bit(true)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
        ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build(),
    ]];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(188)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2 mtu=300",
        "rtpmp2tdepay2",
        expected_pay,
        expected_depay,
    );
}

#[test]
fn test_mp2t_pay_depay_au_ts_packets() {
    init();

    // mpegtsmux would first push these caps and then update with a streamheader
    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 188i32)
        .build();

    let input_buffers = vec![
        // This time we're not feeding single ts packets, but chunk per timestamp/AU
        make_mp2t_buffer(0, 15, gst::ClockTime::ZERO, gst::BufferFlags::empty()),
        make_mp2t_buffer(
            15,
            4,
            gst::ClockTime::from_mseconds(40),
            gst::BufferFlags::DELTA_UNIT,
        ),
        make_mp2t_buffer(
            19,
            4,
            gst::ClockTime::from_mseconds(80),
            gst::BufferFlags::DELTA_UNIT,
        ),
    ];

    let expected_pay = vec![
        // Since the first input buffer contains 15 ts packets, the payloader can push out
        // two rtp packets of 7 ts packets immediately, with 1 ts packet queued for later.
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::ZERO)
                .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
                .pt(33)
                .rtp_time(0)
                .marker_bit(true)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::ZERO)
                .flags(gst::BufferFlags::empty())
                .pt(33)
                .rtp_time(0)
                .marker_bit(false)
                .build(),
        ],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(0)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .rtp_time(7200) // 80ms, 0.080 * 90000
            .marker_bit(false)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(1316)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .size(376)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2",
        "rtpmp2tdepay2",
        expected_pay,
        expected_depay,
    );
}

// gst-launch-1.0 videotestsrc num-buffers=3 is-live=true \
//  ! video/x-raw,format=I420,width=80,height=60,framerate=25/1 \
//  ! x264enc tune=zerolatency \
//  ! mpegtsmux m2ts-mode=true \
//  ! filesink location=videotestsrc-80x60-h264.m2ts \
//  && truncate -s 3648 videotestsrc-80x60-h264.m2ts
//
// Same as above, just with 192-byte packets, and it seems like we only get 19 ts packets
const M2TS_DATA: &[u8] = include_bytes!("videotestsrc-80x60-h264.m2ts").as_slice();

#[test]
fn test_mp2t_pay_depay_m2ts_variant() {
    init();

    // mpegtsmux would first push these caps and then update with a streamheader
    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 192i32)
        .build();

    // Send as single input buffer, we just want to make sure the
    // depayloader can automatically detect the packet size.
    let mut input_buf = gst::Buffer::from_slice(M2TS_DATA);

    if let Some(buf_ref) = input_buf.get_mut() {
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf_ref.set_flags(gst::BufferFlags::empty());
    }

    let input_buffers = vec![input_buf];

    let expected_pay = vec![
        // Since the first input buffer contains 21 ts packets, the payloader can push out
        // 3 rtp packets immediately, with 2 ts packets queued for later.
        vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::ZERO)
                .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
                .pt(33)
                .rtp_time(0)
                .marker_bit(true)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::ZERO)
                .flags(gst::BufferFlags::empty())
                .pt(33)
                .rtp_time(0)
                .marker_bit(false)
                .build(),
        ],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(80))
            .flags(gst::BufferFlags::empty())
            .pt(33)
            .pts(gst::ClockTime::ZERO)
            .marker_bit(false)
            .build()],
    ];

    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(7 * 192)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(7 * 192)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(5 * 192)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2",
        "rtpmp2tdepay2",
        expected_pay,
        expected_depay,
    );
}

#[test]
fn test_mp2t_pay_depay_single_packet() {
    let inputs = [(188u8, MPEGTS_DATA), (192u8, M2TS_DATA)];

    init();

    for (packet_size, data) in inputs {
        // mpegtsmux would first push these caps and then update with a streamheader
        let input_caps = gst::Caps::builder("video/mpegts")
            .field("systemstream", true)
            .field("packetsize", packet_size as i32)
            .build();

        let packet_size = packet_size as usize;

        // Send a single TS packet as input buffer, to make sure the
        // depayloader can still automatically detect the packet size.
        let mut input_buf = gst::Buffer::from_slice(&data[0..][..packet_size]);

        if let Some(buf_ref) = input_buf.get_mut() {
            buf_ref.set_pts(gst::ClockTime::ZERO);
            buf_ref.set_flags(gst::BufferFlags::empty());
        }

        let input_buffers = vec![input_buf];

        let expected_pay = vec![vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(33)
            .rtp_time(0)
            .marker_bit(true)
            .build()]];

        let expected_depay = vec![vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(packet_size)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()]];

        run_test_pipeline(
            Source::Buffers(input_caps, input_buffers),
            "rtpmp2tpay2",
            "rtpmp2tdepay2",
            expected_pay,
            expected_depay,
        );
    }
}

#[test]
fn test_mp2t_depay_skip_bytes() {
    init();

    let input_caps = gst::Caps::builder("video/mpegts")
        .field("systemstream", true)
        .field("packetsize", 192i32)
        .build();

    // Send a single 192-byte TS packet as input buffer, and strip off the first 4 bytes
    // (extra timestamp), which should yield a normal 188-byte TS packet. Need to send
    // a single one since skip-first-bytes only applies to the whole payload buffer,
    // not each individual TS packet, so it wouldn't work right if there were multiple
    // TS packets (because it's made for stripping off other things, not TS packet prefixes).
    let mut input_buf = gst::Buffer::from_slice(&M2TS_DATA[0..][..192]);

    if let Some(buf_ref) = input_buf.get_mut() {
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf_ref.set_flags(gst::BufferFlags::empty());
    }

    let input_buffers = vec![input_buf];

    let expected_pay = vec![vec![ExpectedPacket::builder()
        .pts(gst::ClockTime::ZERO)
        .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
        .pt(33)
        .rtp_time(0)
        .marker_bit(true)
        .build()]];

    let expected_depay = vec![vec![ExpectedBuffer::builder()
        .pts(gst::ClockTime::ZERO)
        .size(192 - 4)
        .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
        .build()]];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpmp2tpay2",
        "rtpmp2tdepay2 skip-first-bytes=4",
        expected_pay,
        expected_depay,
    );
}
