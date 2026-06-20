// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use pretty_assertions::assert_eq;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().expect("mccenc test");
    });
}

/// Encode a single CDP packet and compare the output
#[test]
fn test_encode() {
    init();

    let input = [
        0x96, 0x69, 0x52, 0x4f, 0x67, 0x00, 0x00, 0x72, 0xf4, 0xfc, 0x80, 0x80, 0xfd, 0x80, 0x80,
        0xff, 0x02, 0x22, 0xfe, 0x8c, 0xff, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
        0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
        0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
        0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0x73, 0x91, 0x81, 0x65, 0x6e, 0x67,
        0x81, 0x7f, 0xff, 0x74, 0x00, 0x00, 0x1c,
    ];

    let expected_output = format!("File Format=MacCaption_MCC V1.0\r\n\
\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
// Computer Prompting and Captioning Company\r\n\
// Ancillary Data Packet Transfer File\r\n\
//\r\n\
// Permission to generate this format is granted provided that\r\n\
//   1. This ANC Transfer file format is used on an as-is basis and no warranty is given, and\r\n\
//   2. This entire descriptive information text is included in a generated .mcc file.\r\n\
//\r\n\
// General file format:\r\n\
//   HH:MM:SS:FF(tab)[Hexadecimal ANC data in groups of 2 characters]\r\n\
//     Hexadecimal data starts with the Ancillary Data Packet DID (Data ID defined in S291M)\r\n\
//       and concludes with the Check Sum following the User Data Words.\r\n\
//     Each time code line must contain at most one complete ancillary data packet.\r\n\
//     To transfer additional ANC Data successive lines may contain identical time code.\r\n\
//     Time Code Rate=[24, 25, 30, 30DF, 50, 60]\r\n\
//\r\n\
//   ANC data bytes may be represented by one ASCII character according to the following schema:\r\n\
//     G  FAh 00h 00h\r\n\
//     H  2 x (FAh 00h 00h)\r\n\
//     I  3 x (FAh 00h 00h)\r\n\
//     J  4 x (FAh 00h 00h)\r\n\
//     K  5 x (FAh 00h 00h)\r\n\
//     L  6 x (FAh 00h 00h)\r\n\
//     M  7 x (FAh 00h 00h)\r\n\
//     N  8 x (FAh 00h 00h)\r\n\
//     O  9 x (FAh 00h 00h)\r\n\
//     P  FBh 80h 80h\r\n\
//     Q  FCh 80h 80h\r\n\
//     R  FDh 80h 80h\r\n\
//     S  96h 69h\r\n\
//     T  61h 01h\r\n\
//     U  E1h 00h 00h 00h\r\n\
//     Z  00h\r\n\
//\r\n\
///////////////////////////////////////////////////////////////////////////////////\r\n\
\r\n\
UUID=14720C04-857D-40E2-86FC-F080DE44CE74\r\n\
Creation Program=GStreamer MCC Encoder {}\r\n\
Creation Date=Thursday, December 27, 2018\r\n\
Creation Time=17:34:47\r\n\
Time Code Rate=30DF\r\n\
\r\n\
11:12:13;14	T52S524F67ZZ72F4QRFF0222FE8CFFOM739181656E67817FFF74ZZ1CZ\r\n", env!("CARGO_PKG_VERSION"));

    let mut h = gst_check::Harness::new("mccenc");
    {
        let enc = h.element().expect("could not create encoder");
        enc.set_property("uuid", "14720C04-857D-40E2-86FC-F080DE44CE74");
        enc.set_property(
            "creation-date",
            glib::DateTime::from_utc(2018, 12, 27, 17, 34, 47.0).unwrap(),
        );
    }

    h.set_src_caps_str(
        "closedcaption/x-cea-708, format=(string)cdp, framerate=(fraction)30000/1001",
    );

    let tc = gst_video::ValidVideoTimeCode::new(
        gst::Fraction::new(30000, 1001),
        None,
        gst_video::VideoTimeCodeFlags::DROP_FRAME,
        11,
        12,
        13,
        14,
        0,
    )
    .unwrap();

    let buf = {
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(input));
        let buf_ref = buf.get_mut().unwrap();
        gst_video::VideoTimeCodeMeta::add(buf_ref, &tc);
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf
    };

    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));
    h.push_event(gst::event::Eos::new());

    let buf = h.pull().expect("Couldn't pull buffer");
    let timecode = buf
        .meta::<gst_video::VideoTimeCodeMeta>()
        .expect("No timecode for buffer")
        .tc();
    assert_eq!(timecode, tc);

    let pts = buf.pts().unwrap();
    assert_eq!(pts, gst::ClockTime::ZERO);

    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        Ok(expected_output.as_str()),
    );
}
