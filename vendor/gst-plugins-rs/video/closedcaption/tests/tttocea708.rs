// Copyright (C) 2025 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use gst::ClockTime;
use pretty_assertions::assert_eq;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().unwrap();
    });
}

fn new_timed_buffer<T: AsRef<[u8]> + Send + 'static>(
    slice: T,
    timestamp: ClockTime,
    duration: ClockTime,
) -> gst::buffer::Buffer {
    let mut buf = gst::Buffer::from_slice(slice);
    let buf_ref = buf.get_mut().unwrap();
    buf_ref.set_pts(timestamp);
    buf_ref.set_duration(duration);
    buf
}

#[test]
fn test_ttcea708_non_timed_buffer() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea708 mode=pop-on cea608-channel=1");
    h.set_src_caps_str("text/x-raw,format=utf8");
    h.set_sink_caps_str("closedcaption/x-cea-708,format=cc_data,framerate=30/1");

    let inbuf = gst::Buffer::from_slice("Hello");

    assert_eq!(h.push(inbuf), Err(gst::FlowError::Error));
}

static PADDING: [u8; 60] = [
    0xf8, 0x80, 0x80, // cea-608 field 1
    0xf9, 0x80, 0x80, // cea-608 field 2
    0xfa, 0x00, 0x00, // dtvcc
    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
    0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
    0xfa, 0x00, 0x00,
];

static POP_ON_PREAMBLE: [u8; 24] = [
    0xff, 0x0c, 0x36, 0xfe, 0x88, 0x01, 0xfe, 0x8c, 0xfe, 0xfe, 0x99, 0x18, 0xfe, 0xe4, 0x32, 0xfe,
    0x7e, 0x1f, 0xfe, 0x11, 0x92, 0xfe, 0x0e, 0x00,
];

/* Check translation of a simple string */
#[test]
fn test_tttocea708_one_timed_buffer_and_eos() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea708 mode=pop-on cea608-channel=1");
    h.set_src_caps_str("text/x-raw,format=utf8");
    h.set_sink_caps_str("closedcaption/x-cea-708,format=cc_data,framerate=30/1");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", ClockTime::SECOND, ClockTime::SECOND);

    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let mut expected0 = PADDING;
    expected0[..3].copy_from_slice(&[0xfc, 0x94, 0x20]);
    expected0[6..30].copy_from_slice(&POP_ON_PREAMBLE);
    // Hello ETX
    expected0[30..42].copy_from_slice(&[
        0xfe, 0x48, 0x65, 0xfe, 0x6c, 0x6c, 0xfe, 0x6f, 0x8b, 0xfe, 0x03, 0x03,
    ]);
    let mut expected1 = PADDING;
    expected1[..3].copy_from_slice(&[0xfc, 0x94, 0xae]);
    let mut expected2 = PADDING;
    expected2[..3].copy_from_slice(&[0xfc, 0x94, 0x70]);
    let mut expected3 = PADDING;
    expected3[..3].copy_from_slice(&[0xfc, 0xc8, 0xe5]);
    let mut expected4 = PADDING;
    expected4[..3].copy_from_slice(&[0xfc, 0xec, 0xec]);
    let mut expected5 = PADDING;
    expected5[..3].copy_from_slice(&[0xfc, 0xef, 0x80]);
    let mut expected6 = PADDING;
    expected6[..3].copy_from_slice(&[0xfc, 0x94, 0x2f]);

    let expected: [(ClockTime, ClockTime, &[u8]); 7] = [
        (1_000_000_000.nseconds(), 33_333_333.nseconds(), &expected0), /* resume_caption_loading */
        (1_033_333_333.nseconds(), 33_333_333.nseconds(), &expected1), /* erase_non_displayed_memory */
        (1_066_666_667.nseconds(), 33_333_333.nseconds(), &expected2), /* preamble */
        (1_100_000_000.nseconds(), 33_333_333.nseconds(), &expected3), /* H e */
        (1_133_333_333.nseconds(), 33_333_333.nseconds(), &expected4), /* l l */
        (1_166_666_667.nseconds(), 33_333_333.nseconds(), &expected5), /* o, nil */
        (1_200_000_000.nseconds(), 33_333_333.nseconds(), &expected6), /* end_of_caption */
    ];

    for (i, e) in expected.iter().enumerate() {
        let outbuf = h.try_pull().unwrap();

        assert_eq!(
            e.0,
            outbuf.pts().unwrap(),
            "Unexpected PTS for {}th buffer",
            i + 1
        );
        assert_eq!(
            e.1,
            outbuf.duration().unwrap(),
            "Unexpected duration for {}th buffer",
            i + 1
        );

        let data = outbuf.map_readable().unwrap();
        println!("{i}th buffer: {data:?}");
        assert_eq!(e.2, &*data);
    }

    assert_eq!(h.buffers_in_queue(), 23);

    h.push_event(gst::event::Eos::new());

    /* Check that we do receive an erase_display */
    loop {
        let outbuf = h.try_pull().unwrap();
        let data = outbuf.map_readable().unwrap();
        println!("data: {data:?}");
        if outbuf.pts().unwrap() == 2_000_000_000.nseconds() {
            let mut expected = PADDING;
            expected[..3].copy_from_slice(&[0xfc, 0x94, 0x2c]);
            expected[6..12].copy_from_slice(&[0xff, 0x42, 0x22, 0xfe, 0x88, 0x02]);
            assert_eq!(&*data, &expected);
            break;
        } else {
            assert_eq!(&*data, &PADDING);
        }
    }

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}

/* Here we verify that the element outputs a continuous stream
 * with padding buffers
 */
#[test]
fn test_tttocea708_output_gaps() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea708 mode=pop-on cea608-channel=1");
    h.set_src_caps_str("text/x-raw,format=utf8");
    h.set_sink_caps_str("closedcaption/x-cea-708,format=cc_data,framerate=30/1");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", 1_000_000_000.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let inbuf = new_timed_buffer("World", 3_000_000_000.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    h.push_event(gst::event::Eos::new());

    /* Padding */
    loop {
        let outbuf = h.pull().unwrap();
        println!("pts {}", outbuf.pts().unwrap().display());
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= ClockTime::SECOND {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_eq!(&*data, &PADDING);
    }

    /* Hello */
    loop {
        let outbuf = h.pull().unwrap();
        println!("pts {}", outbuf.pts().unwrap().display());
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 1_233_333_333.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_ne!(&*data, &PADDING);
    }

    /* Padding */
    loop {
        let outbuf = h.pull().unwrap();
        println!("pts {}", outbuf.pts().unwrap().display());
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 3_000_000_000.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        if outbuf.pts().unwrap() == 2_000_000_000.nseconds() {
            /* Erase 708 display one second after Hello */
            let mut expected = PADDING;
            let clear_window = [0xff, 0x42, 0x22, 0xfe, 0x88, 0x02];
            expected[6..12].copy_from_slice(&clear_window);
            assert_eq!(&*data, &expected);
        } else if outbuf.pts().unwrap() == 2_200_000_000.nseconds() {
            /* Erase 608 display one second after Hello */
            let mut expected = PADDING;
            let clear_window = [0xfc, 0x94, 0x2C];
            expected[0..3].copy_from_slice(&clear_window);
            assert_eq!(&*data, &expected);
        } else {
            assert_eq!(&*data, &PADDING);
        }
    }

    /* World */
    loop {
        let outbuf = h.pull().unwrap();
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 3_233_333_333.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_ne!(&*data, &PADDING);
    }

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}

/* Here we verify that the element does not crash on large input buffers */
#[test]
fn test_tttocea708_large_input() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea708 mode=roll-up");
    h.set_src_caps_str("text/x-raw,format=utf8");
    h.set_sink_caps_str("closedcaption/x-cea-708,format=cc_data,framerate=60/1");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer(" This is going to be a very long#& buffer that will exceed the output length of a single DTVCCPacket.#&", 0.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    h.push_event(gst::event::Eos::new());

    loop {
        let outbuf = h.pull().unwrap();
        println!("pts {}", outbuf.pts().unwrap().display());
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 200_000_000.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        println!("data {:?}", &*data);
        assert_ne!(&*data, &PADDING);
    }

    /* Padding */
    loop {
        let outbuf = h.pull().unwrap();
        println!("pts {}", outbuf.pts().unwrap().display());
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 1_000_000_000.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_eq!(&*data, &PADDING);
    }

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}
