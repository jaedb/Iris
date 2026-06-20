//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{event::Eos, Caps};
use gst_check::Harness;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpav1 test");
    });
}

#[test]
fn test_depayloader() {
    #[rustfmt::skip]
    let test_packets: [(Vec<u8>, gst::ClockTime, bool, u32); 4] = [
        ( // simple packet, complete TU
            vec![       // RTP payload
                0b0001_1000,
                0b0011_0000, 1, 2, 3, 4, 5, 6,
            ],
            gst::ClockTime::from_seconds(0),
            true,       // marker bit
            100_000,    // timestamp
        ), ( // 2 OBUs, last is fragmented
            vec![
                0b0110_0000,
                0b0000_0110, 0b0011_0000, 1, 2, 3, 4, 5,
                             0b0011_0000, 1, 2, 3,
            ],
            gst::ClockTime::from_seconds(1),
            false,
            190_000,
        ), ( // continuation of the last OBU
            vec![
                0b1100_0000,
                0b0000_0100, 4, 5, 6, 7,
            ],
            gst::ClockTime::from_seconds(1),
            false,
            190_000,
        ), ( // finishing the OBU fragment
            vec![
                0b1001_0000,
                8, 9, 10,
            ],
            gst::ClockTime::from_seconds(1),
            true,
            190_000,
        )
    ];

    #[rustfmt::skip]
    let expected: [(gst::ClockTime, Vec<u8>); 3] = [
        (
            gst::ClockTime::from_seconds(0),
            vec![0b0001_0010, 0, 0b0011_0010, 0b0000_0110, 1, 2, 3, 4, 5, 6],
        ),
        (
            gst::ClockTime::from_seconds(1),
            vec![0b0001_0010, 0, 0b0011_0010, 0b0000_0101, 1, 2, 3, 4, 5],
        ),
        (
            gst::ClockTime::from_seconds(1),
            vec![0b0011_0010, 0b0000_1010, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        ),
    ];

    init();

    let mut h = Harness::new("rtpav1depay");
    h.play();

    let caps = Caps::builder("application/x-rtp")
        .field("media", "video")
        .field("payload", 96)
        .field("clock-rate", 90000)
        .field("encoding-name", "AV1")
        .build();
    h.set_src_caps(caps);

    for (idx, (bytes, pts, marker, timestamp)) in test_packets.iter().enumerate() {
        let builder = rtp_types::RtpPacketBuilder::new()
            .marker_bit(*marker)
            .timestamp(*timestamp)
            .payload_type(96)
            .sequence_number(idx as u16)
            .payload(bytes.as_slice());
        let buf = builder.write_vec().unwrap();
        let mut buf = gst::Buffer::from_mut_slice(buf);
        {
            buf.get_mut().unwrap().set_pts(*pts);
        }

        h.push(buf).unwrap();
    }
    h.push_event(Eos::new());

    for (idx, (pts, ex)) in expected.iter().enumerate() {
        println!("checking buffer {idx}...");

        let buffer = h.pull().unwrap();
        assert_eq!(buffer.pts(), Some(*pts));
        let actual = buffer.into_mapped_buffer_readable().unwrap();
        assert_eq!(actual.as_slice(), ex.as_slice());
    }
}
