// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019 Jordan Petridis <jordan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use pretty_assertions::assert_eq;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().unwrap();
    });
}

/// Encode a single raw CEA608 packet and compare the output
#[test]
fn test_encode_single_packet() {
    init();

    let input = [148, 44];
    let expected_output = b"Scenarist_SCC V1.0\r\n\r\n11:12:13;14\t942c\r\n\r\n";

    let mut h = gst_check::Harness::new("sccenc");
    h.set_src_caps_str("closedcaption/x-cea-608, format=raw, framerate=(fraction)30000/1001");
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
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(&input[..]));
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
        std::str::from_utf8(expected_output.as_ref())
    );
}

/// Encode a multiple raw CEA608 packets and compare the output
#[test]
fn test_encode_multiple_packets() {
    init();

    let input1 = [148, 44];
    let input2 = [
        148, 32, 148, 32, 148, 174, 148, 174, 148, 84, 148, 84, 16, 174, 16, 174, 70, 242, 239,
        109, 32, 206, 229, 247, 32, 217, 239, 242, 107, 44, 148, 242, 148, 242, 16, 174, 16, 174,
        244, 104, 233, 115, 32, 233, 115, 32, 196, 229, 109, 239, 227, 242, 97, 227, 121, 32, 206,
        239, 247, 161, 148, 47, 148, 47,
    ];

    let expected_output1 = b"Scenarist_SCC V1.0\r\n\r\n00:00:00;00\t942c 942c\r\n\r\n";
    let expected_output2 = b"00:00:14;01\t9420 9420 94ae 94ae 9454 9454 10ae 10ae 46f2 ef6d 20ce e5f7 20d9 eff2 6b2c 94f2\r\n\r\n";
    let expected_output3 = b"00:00:14;17\t94f2 10ae 10ae f468 e973 20e9 7320 c4e5 6def e3f2 61e3 7920 ceef f7a1 942f 942f\r\n\r\n";

    let mut h = gst_check::Harness::new("sccenc");
    h.set_src_caps_str("closedcaption/x-cea-608, format=raw, framerate=(fraction)30000/1001");
    let tc1 = gst_video::ValidVideoTimeCode::new(
        gst::Fraction::new(30000, 1001),
        None,
        gst_video::VideoTimeCodeFlags::DROP_FRAME,
        0,
        0,
        0,
        0,
        0,
    )
    .unwrap();

    let tc2 = gst_video::ValidVideoTimeCode::new(
        gst::Fraction::new(30000, 1001),
        None,
        gst_video::VideoTimeCodeFlags::DROP_FRAME,
        0,
        0,
        14,
        1,
        0,
    )
    .unwrap();

    let buf1 = {
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(&input1[..]));
        let buf_ref = buf.get_mut().unwrap();
        gst_video::VideoTimeCodeMeta::add(buf_ref, &tc1);
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf
    };

    let buf2 = {
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(&input1[..]));
        let buf_ref = buf.get_mut().unwrap();
        let mut tc = tc1.clone();
        tc.increment_frame();
        gst_video::VideoTimeCodeMeta::add(buf_ref, &tc);
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf
    };

    let mut t = tc2.clone();
    let mut buffers = input2
        .chunks(2)
        .map(move |bytes| {
            let mut buf = gst::Buffer::from_mut_slice(Vec::from(bytes));
            let buf_ref = buf.get_mut().unwrap();
            gst_video::VideoTimeCodeMeta::add(buf_ref, &t);
            t.increment_frame();
            buf
        })
        .collect::<Vec<gst::Buffer>>();
    buffers.insert(0, buf1);
    buffers.insert(1, buf2);

    buffers.iter().for_each(|buf| {
        assert_eq!(h.push(buf.clone()), Ok(gst::FlowSuccess::Ok));
    });
    h.push_event(gst::event::Eos::new());

    // Pull 1
    let buf = h.pull().expect("Couldn't pull buffer");

    let timecode = buf
        .meta::<gst_video::VideoTimeCodeMeta>()
        .expect("No timecode for buffer")
        .tc();
    assert_eq!(timecode, tc1);

    let pts = buf.pts().unwrap();
    assert_eq!(pts, gst::ClockTime::ZERO);

    let map = buf.map_readable().expect("Couldn't map buffer readable");

    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        std::str::from_utf8(expected_output1.as_ref())
    );

    // Pull 2
    let buf = h.pull().expect("Couldn't pull buffer");
    let timecode = buf
        .meta::<gst_video::VideoTimeCodeMeta>()
        .expect("No timecode for buffer")
        .tc();
    assert_eq!(timecode, tc2);

    // let pts = buf.get_pts().unwrap();
    // assert_eq!(pts, gst::ClockTime::ZERO);

    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        std::str::from_utf8(expected_output2.as_ref())
    );

    let tc3 = gst_video::ValidVideoTimeCode::new(
        gst::Fraction::new(30000, 1001),
        None,
        gst_video::VideoTimeCodeFlags::DROP_FRAME,
        0,
        0,
        14,
        17,
        0,
    )
    .unwrap();

    // Pull 3
    let buf = h.pull().expect("Couldn't pull buffer");
    let timecode = buf
        .meta::<gst_video::VideoTimeCodeMeta>()
        .expect("No timecode for buffer")
        .tc();
    assert_eq!(timecode, tc3);

    // let pts = buf.get_pts().unwrap();
    // assert_eq!(pts, gst::ClockTime::ZERO);

    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        std::str::from_utf8(expected_output3.as_ref())
    );
}
