// GStreamer RTP KLV Payloader / Depayloader - unit tests
//
// Copyright (C) 2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::klv::klv_utils::*;
use crate::tests::{run_test_pipeline_full, ExpectedBuffer, ExpectedPacket, Liveness, Source};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtp klv test");
    });
}

const KLV_DATA: &[u8] = include_bytes!("day-flight.klv").as_slice();

pub(crate) fn parse_klv_packets(data: &[u8]) -> anyhow::Result<Vec<&[u8]>> {
    let mut packets = vec![];

    let mut data = &data[0..];

    while !data.is_empty() {
        let size = peek_klv(data)?;
        // eprintln!("KLV unit {} of size {size}", packets.len());
        packets.push(&data[0..][..size]);
        data = &data[size..];
    }

    Ok(packets)
}

fn make_buffer(
    data: &'static [u8],
    pts: gst::ClockTime,
    duration: Option<gst::ClockTime>,
    flags: gst::BufferFlags,
) -> gst::Buffer {
    let mut buf = gst::Buffer::from_slice(data);

    let buf_ref = buf.get_mut().unwrap();
    buf_ref.set_pts(pts);
    if let Some(duration) = duration {
        buf_ref.set_duration(duration);
    }
    buf_ref.set_flags(flags);

    buf
}

// test_klv_pay_depay
//
// Check basic payloading/depayloading
//
#[test]
fn test_klv_pay_depay() {
    let klv_packets = parse_klv_packets(KLV_DATA).unwrap();

    init();

    let input_caps = gst::Caps::builder("meta/x-klv")
        .field("parsed", true)
        .build();

    let mut input_buffers = vec![];

    for (i, klv) in klv_packets.iter().enumerate() {
        input_buffers.push(make_buffer(
            klv,
            gst::ClockTime::from_seconds(i as u64),
            gst::ClockTime::NONE,
            if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            },
        ));
    }

    let mut expected_pay = vec![];

    for (i, _klv) in klv_packets.iter().enumerate() {
        let expected_flags = match i {
            0 => gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER,
            _ => gst::BufferFlags::MARKER,
        };
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_seconds(i as u64))
            .flags(expected_flags)
            .pt(96)
            .rtp_time(i as u32 * 90_000)
            .marker_bit(true)
            .build()]);
    }

    let mut expected_depay = vec![];

    for (i, _klv) in klv_packets.into_iter().enumerate() {
        let expected_flags = match i {
            0 => gst::BufferFlags::DISCONT,
            _ => gst::BufferFlags::empty(),
        };
        let expected_size = match i {
            0..=4 => 163,
            5 => 162,
            _ => unreachable!(),
        };
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_seconds(i as u64))
            .size(expected_size)
            .flags(expected_flags)
            .build()]);
    }

    let expected_output_caps = input_caps.clone();

    run_test_pipeline_full(
        Source::Buffers(input_caps, input_buffers),
        "rtpklvpay2",
        "rtpklvdepay2",
        expected_pay,
        expected_depay,
        Some(expected_output_caps),
        Liveness::NonLive,
    );
}

// test_klv_pay_depay_fragmented
//
// Check basic payloading/depayloading with fragmentated payloads
//
#[test]
fn test_klv_pay_depay_fragmented() {
    let klv_packets = parse_klv_packets(KLV_DATA).unwrap();

    init();

    let input_caps = gst::Caps::builder("meta/x-klv")
        .field("parsed", true)
        .build();

    let mut input_buffers = vec![];

    for (i, klv) in klv_packets.iter().enumerate() {
        input_buffers.push(make_buffer(
            klv,
            gst::ClockTime::from_seconds(i as u64),
            gst::ClockTime::NONE,
            if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            },
        ));
    }

    let mut expected_pay = vec![];

    for (i, _klv) in klv_packets.iter().enumerate() {
        expected_pay.push(vec![
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_seconds(i as u64))
                .flags(if i == 0 {
                    gst::BufferFlags::DISCONT
                } else {
                    gst::BufferFlags::empty()
                })
                .pt(96)
                .rtp_time(i as u32 * 90_000)
                .marker_bit(false)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_seconds(i as u64))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(i as u32 * 90_000)
                .marker_bit(false)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_seconds(i as u64))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(i as u32 * 90_000)
                .marker_bit(false)
                .build(),
            ExpectedPacket::builder()
                .pts(gst::ClockTime::from_seconds(i as u64))
                .flags(gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(i as u32 * 90_000)
                .marker_bit(true)
                .build(),
        ]);
    }

    let mut expected_depay = vec![];

    for (i, _klv) in klv_packets.into_iter().enumerate() {
        let expected_flags = match i {
            0 => gst::BufferFlags::DISCONT,
            _ => gst::BufferFlags::empty(),
        };
        let expected_size = match i {
            0..=4 => 163,
            5 => 162,
            _ => unreachable!(),
        };
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_seconds(i as u64))
            .size(expected_size)
            .flags(expected_flags)
            .build()]);
    }

    let expected_output_caps = input_caps.clone();

    run_test_pipeline_full(
        Source::Buffers(input_caps, input_buffers),
        "rtpklvpay2 mtu=60",
        "rtpklvdepay2",
        expected_pay,
        expected_depay,
        Some(expected_output_caps),
        Liveness::NonLive,
    );
}

// test_klv_pay_depay_with_packet_loss
//
// Check basic payloading/depayloading with some packet loss
//
#[test]
fn test_klv_pay_depay_with_packet_loss() {
    let klv_packets = parse_klv_packets(KLV_DATA).unwrap();

    init();

    let input_caps = gst::Caps::builder("meta/x-klv")
        .field("parsed", true)
        .build();

    let mut input_buffers = vec![];

    for (i, klv) in klv_packets.iter().enumerate() {
        input_buffers.push(make_buffer(
            klv,
            gst::ClockTime::from_seconds(i as u64),
            gst::ClockTime::NONE,
            if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            },
        ));
    }

    let mut expected_pay = vec![];

    for (i, _klv) in klv_packets.iter().enumerate() {
        let expected_flags = match i {
            0 => gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER,
            _ => gst::BufferFlags::MARKER,
        };
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_seconds(i as u64))
            .flags(expected_flags)
            .pt(96)
            .rtp_time(i as u32 * 90_000)
            .marker_bit(true)
            .drop(i == 0 || i == 2) // Drop 1st and 3rd packet
            .build()]);
    }

    let mut expected_depay = vec![];

    // 1st and 3rd packet should have been dropped
    for (i, _klv) in klv_packets
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| i != 0 && i != 2)
    {
        println!("i = {i}");
        let expected_flags = match i {
            0 | 2 => unreachable!(),
            1 | 3 => gst::BufferFlags::DISCONT, // Depayloader will add DISCONT flag on first packet and on discontinuities
            _ => gst::BufferFlags::empty(),
        };
        let expected_size = match i {
            0..=4 => 163,
            5 => 162,
            _ => unreachable!(),
        };
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_seconds(i as u64))
            .size(expected_size)
            .flags(expected_flags)
            .build()]);
    }

    let expected_output_caps = input_caps.clone();

    run_test_pipeline_full(
        Source::Buffers(input_caps, input_buffers),
        "rtpklvpay2",
        "rtpklvdepay2",
        expected_pay,
        expected_depay,
        Some(expected_output_caps),
        Liveness::NonLive,
    );
}

// test_klv_pay_depay_fragmented_with_packet_loss
//
// Check basic payloading/depayloading with fragmentated payloads with packet loss
//
#[test]
fn test_klv_pay_depay_fragmented_with_packet_loss() {
    init();

    fn run_klv_pay_depay_fragmented_with_packet_loss_with_drop_mask(
        drop_mask: u32,
        initial_seqnum: Option<u16>,
    ) {
        let klv_packets = parse_klv_packets(KLV_DATA).unwrap();

        let input_caps = gst::Caps::builder("meta/x-klv")
            .field("parsed", true)
            .build();

        let mut input_buffers = vec![];

        for (i, klv) in klv_packets.iter().enumerate() {
            input_buffers.push(make_buffer(
                klv,
                gst::ClockTime::from_seconds(i as u64),
                gst::ClockTime::NONE,
                if i == 0 {
                    gst::BufferFlags::DISCONT
                } else {
                    gst::BufferFlags::empty()
                },
            ));
        }

        let mut expected_pay = vec![];

        for (i, _klv) in klv_packets.iter().enumerate() {
            let packet_mask = (drop_mask >> (4 * i)) & 0b1111;

            expected_pay.push(vec![
                ExpectedPacket::builder()
                    .pts(gst::ClockTime::from_seconds(i as u64))
                    .flags(if i == 0 {
                        gst::BufferFlags::DISCONT
                    } else {
                        gst::BufferFlags::empty()
                    })
                    .pt(96)
                    .rtp_time(i as u32 * 90_000)
                    .marker_bit(false)
                    .drop((packet_mask & 0b0001) == 0b0001)
                    .build(),
                ExpectedPacket::builder()
                    .pts(gst::ClockTime::from_seconds(i as u64))
                    .flags(gst::BufferFlags::empty())
                    .pt(96)
                    .rtp_time(i as u32 * 90_000)
                    .marker_bit(false)
                    .drop((packet_mask & 0b0010) == 0b0010)
                    .build(),
                ExpectedPacket::builder()
                    .pts(gst::ClockTime::from_seconds(i as u64))
                    .flags(gst::BufferFlags::empty())
                    .pt(96)
                    .rtp_time(i as u32 * 90_000)
                    .marker_bit(false)
                    .drop((packet_mask & 0b0100) == 0b0100)
                    .build(),
                ExpectedPacket::builder()
                    .pts(gst::ClockTime::from_seconds(i as u64))
                    .flags(gst::BufferFlags::MARKER)
                    .pt(96)
                    .rtp_time(i as u32 * 90_000)
                    .marker_bit(true)
                    .drop((packet_mask & 0b1000) == 0b1000)
                    .build(),
            ]);
        }

        let mut expected_depay = vec![];

        for (i, _klv) in klv_packets.iter().enumerate() {
            let packet_mask = (drop_mask >> (4 * i)) & 0b1111;

            // Expect discont on first packet and if previous packet got dropped
            let expected_flags = if i == 0 || (drop_mask >> (4 * (i - 1))) & 0b1111 != 0b0000 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            };
            let expected_size = match i {
                0..=4 => 163,
                5 => 162,
                _ => unreachable!(),
            };
            // If any of the fragments got dropped, we can't reconstruct the original payload
            if packet_mask == 0b0000 {
                expected_depay.push(vec![ExpectedBuffer::builder()
                    .pts(gst::ClockTime::from_seconds(i as u64))
                    .size(expected_size)
                    .flags(expected_flags)
                    .build()]);
            }
        }

        let expected_output_caps = input_caps.clone();

        let payloader = if let Some(seqnum_offset) = initial_seqnum {
            format!("rtpklvpay2 mtu=60 seqnum-offset={seqnum_offset}")
        } else {
            "rtpklvpay2 mtu=60".to_string()
        };

        run_test_pipeline_full(
            Source::Buffers(input_caps.clone(), input_buffers.clone()),
            &payloader,
            "rtpklvdepay2",
            expected_pay,
            expected_depay,
            Some(expected_output_caps),
            Liveness::NonLive,
        );
    }

    // Test case for https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/merge_requests/1584
    // where the base class got confused because the lost packet was just at the seqnum wraparound
    run_klv_pay_depay_fragmented_with_packet_loss_with_drop_mask(0b100000000000000, Some(65520));

    // Now run with different drop patterns.. can't test all 2^24 combinations.
    // 24 = 6 KLV units * 4 fragments/unit
    // (163 bytes per unit + 4*12 bytes rtp headers with 60 mtu size = 3.5 packets per unit)
    let masks = [
        0b0000_0100_1100_0000_0000,
        0b0001_1111_1100_0000_1000,
        0b0010_1000_0101_0101_0000,
        0b0011_0000_1110_1010_1010,
        0b0011_0010_0000_0000_0010,
        0b0011_0010_0000_1011_0111,
        0b0011_1011_1111_0000_1000,
        0b0011_1111_1001_0101_0000,
    ];

    for start_mask in masks {
        for mask in (start_mask..start_mask + 8000).step_by(0b010101) {
            run_klv_pay_depay_fragmented_with_packet_loss_with_drop_mask(mask, None);
            run_klv_pay_depay_fragmented_with_packet_loss_with_drop_mask(mask, Some(65520));
        }
    }
}
