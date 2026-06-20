//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{event::Eos, prelude::*, Buffer, Caps, ClockTime};
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
fn test_payloader() {
    #[rustfmt::skip]
    let test_buffers: [(u64, Vec<u8>); 3] = [
        (
            0,
            vec![   // this should result in exactly 27 bytes for the RTP payload
                0b0001_0010, 0,
                0b0000_1010, 0,
                0b0011_0010, 0b0000_1100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                0b0011_0010, 0b0000_1001, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            ],
        ), (
            0,
            vec![   // these all have to go in separate packets since their IDs mismatch
                0b0011_0010, 0b0000_0100, 1, 2, 3, 4,
                0b0011_0110, 0b0010_1000, 0b0000_0101, 1, 2, 3, 4, 5,
                0b0011_0110, 0b0100_1000, 0b0000_0001, 1,
            ],
        ), (
            1_000_000_000,
            vec![
                0b0001_0010, 0,
                0b0011_0010, 0b0000_0100, 1, 2, 3, 4,
            ]
        )
    ];

    #[rustfmt::skip]
    let expected = [
        (
            false,  // marker bit
            0,      // relative RTP timestamp
            vec![   // payload bytes
                0b0011_1000,
                0b0000_0001, 0b0000_1000,
                0b0000_1101, 0b0011_0000, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                             0b0011_0000, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            ],
        ), (
            false,
            0,
            vec![
                0b0001_0000,
                0b0011_0000, 1, 2, 3, 4,
            ]
        ), (
            false,
            0,
            vec![
                0b0001_0000,
                0b0011_0100, 0b0010_1000, 1, 2, 3, 4, 5,
            ]
        ), (
            true,
            0,
            vec![
                0b0001_0000,
                0b0011_0100, 0b0100_1000, 1,
            ]
        ), (
            true, // because of EOS
            90_000,
            vec![
                0b0001_0000,
                0b0011_0000, 1, 2, 3, 4,
            ]
        )
    ];

    init();

    let mut h = Harness::new("rtpav1pay");
    {
        let pay = h.element().unwrap();
        pay.set_property(
            "mtu",
            27u32 + rtp_types::RtpPacket::MIN_RTP_PACKET_LEN as u32,
        );
    }
    h.play();

    let caps = Caps::builder("video/x-av1")
        .field("parsed", true)
        .field("stream-format", "obu-stream")
        .field("alignment", "obu")
        .build();
    h.set_src_caps(caps);

    for (pts, bytes) in &test_buffers {
        let mut buffer = Buffer::with_size(bytes.len())
            .unwrap()
            .into_mapped_buffer_writable()
            .unwrap();
        buffer.copy_from_slice(bytes);

        let mut buffer = buffer.into_buffer();
        buffer
            .get_mut()
            .unwrap()
            .set_pts(ClockTime::from_nseconds(*pts));

        h.push(buffer).unwrap();
    }
    h.push_event(Eos::new());

    let mut base_ts = None;
    for (idx, (marker, ts_offset, payload)) in expected.iter().enumerate() {
        println!("checking packet {idx}...");

        let buffer = h.pull().unwrap();
        let map = buffer.map_readable().unwrap();
        let packet = rtp_types::RtpPacket::parse(&map).unwrap();
        if base_ts.is_none() {
            base_ts = Some(packet.timestamp());
        }

        assert_eq!(packet.payload(), payload.as_slice());
        assert_eq!(packet.marker_bit(), *marker);
        assert_eq!(packet.timestamp(), base_ts.unwrap() + ts_offset);
    }
}
