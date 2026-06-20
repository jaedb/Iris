// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
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
fn test_non_timed_buffer() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=pop-on");
    h.set_src_caps_str("text/x-raw,format=utf8");

    let inbuf = gst::Buffer::from_slice("Hello");

    assert_eq!(h.push(inbuf), Err(gst::FlowError::Error));
}

/* Check translation of a simple string */
#[test]
fn test_one_timed_buffer_and_eos() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=pop-on");
    h.set_src_caps_str("text/x-raw,format=utf8");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", ClockTime::SECOND, ClockTime::SECOND);

    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let expected: [(ClockTime, ClockTime, [u8; 2usize]); 7] = [
        (
            1_000_000_000.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x20],
        ), /* resume_caption_loading */
        (
            1_033_333_333.nseconds(),
            33_333_334.nseconds(),
            [0x94, 0xae],
        ), /* erase_non_displayed_memory */
        (
            1_066_666_667.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x70],
        ), /* preamble */
        (
            1_100_000_000.nseconds(),
            33_333_333.nseconds(),
            [0xc8, 0xe5],
        ), /* H e */
        (
            1_133_333_333.nseconds(),
            33_333_334.nseconds(),
            [0xec, 0xec],
        ), /* l l */
        (
            1_166_666_667.nseconds(),
            33_333_333.nseconds(),
            [0xef, 0x80],
        ), /* o, nil */
        (
            1_200_000_000.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x2f],
        ), /* end_of_caption */
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
        assert_eq!(e.2, &*data);
    }

    assert_eq!(h.buffers_in_queue(), 23);

    h.push_event(gst::event::Eos::new());

    /* Check that we do receive an erase_display */
    loop {
        let outbuf = h.try_pull().unwrap();
        let data = outbuf.map_readable().unwrap();
        if outbuf.pts().unwrap() == 2_200_000_000.nseconds() {
            assert_eq!(&*data, &[0x94, 0x2c]);
            break;
        } else {
            assert_eq!(&*data, &[0x80, 0x80]);
        }
    }

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}

/* Here we test that the erase_display_memory control code
 * gets inserted at the correct moment, when there's enough
 * of an interval between two buffers
 */
#[test]
fn test_erase_display_memory_non_spliced() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=pop-on");
    h.set_src_caps_str("text/x-raw,format=utf8");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", 1_000_000_000.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let inbuf = new_timed_buffer("World", 3_000_000_000.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let mut erase_display_buffers = 0;

    while h.buffers_in_queue() > 0 {
        let outbuf = h.pull().unwrap();

        if outbuf.pts().unwrap() == 2_200_000_000.nseconds() {
            let data = outbuf.map_readable().unwrap();
            assert_eq!(&*data, &[0x94, 0x2c]);
            erase_display_buffers += 1;
        }
    }

    assert_eq!(erase_display_buffers, 1);
}

/* Here we test that the erase_display_memory control code
 * gets inserted while loading the following pop-on captions
 * when there's not enough of an interval between them.
 *
 * Note that as tttocea608 introduces an offset between the
 * intended PTS and the actual display time with pop-on captions
 * (when end_of_caption is output) in order not to introduce
 * a huge latency, the clear time is also offset so that the captions
 * display as long as intended.
 */
#[test]
fn test_erase_display_memory_spliced() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=pop-on");
    h.set_src_caps_str("text/x-raw,format=utf8");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", 1_000_000_000.nseconds(), ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let inbuf = new_timed_buffer(
        "World, Lorem Ipsum",
        2_000_000_000.nseconds(),
        ClockTime::SECOND,
    );
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let mut erase_display_buffers = 0;
    let mut prev_pts: ClockTime = ClockTime::ZERO;

    while h.buffers_in_queue() > 0 {
        let outbuf = h.pull().unwrap();

        /* Check that our timestamps are strictly ascending */
        let pts = outbuf.pts().unwrap();
        assert!(pts >= prev_pts);

        if pts == 2_200_000_000.nseconds() {
            let data = outbuf.map_readable().unwrap();
            assert_eq!(&*data, &[0x94, 0x2c]);
            erase_display_buffers += 1;
        }

        prev_pts = pts;
    }

    assert_eq!(erase_display_buffers, 1);
}

/* Here we verify that the element outputs a continuous stream
 * with padding buffers
 */
#[test]
fn test_output_gaps() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=pop-on");
    h.set_src_caps_str("text/x-raw,format=utf8");

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
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= ClockTime::SECOND {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_eq!(&*data, &[0x80, 0x80]);
    }

    /* Hello */
    loop {
        let outbuf = h.pull().unwrap();
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 1_233_333_333.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_ne!(&*data, &[0x80, 0x80]);
    }

    /* Padding */
    loop {
        let outbuf = h.pull().unwrap();
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 3_000_000_000.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        if outbuf.pts().unwrap() == 2_200_000_000.nseconds() {
            /* Erase display one second after Hello */
            assert_eq!(&*data, &[0x94, 0x2C]);
        } else {
            assert_eq!(&*data, &[0x80, 0x80]);
        }
    }

    /* World */
    loop {
        let outbuf = h.pull().unwrap();
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 3_233_333_333.nseconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_ne!(&*data, &[0x80, 0x80]);
    }

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}

#[test]
fn test_one_timed_buffer_and_eos_roll_up2() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=roll-up2");
    h.set_src_caps_str("text/x-raw,format=utf8");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello", ClockTime::SECOND, ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let inbuf = new_timed_buffer("World", 2.seconds(), 1.nseconds());
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let expected: [(ClockTime, ClockTime, [u8; 2usize]); 5] = [
        (
            1_000_000_000.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x25],
        ), /* roll_up_2 */
        (
            1_033_333_333.nseconds(),
            33_333_334.nseconds(),
            [0x94, 0x70],
        ), /* preamble */
        (
            1_066_666_667.nseconds(),
            33_333_333.nseconds(),
            [0xc8, 0xe5],
        ), /* H e */
        (
            1_100_000_000.nseconds(),
            33_333_333.nseconds(),
            [0xec, 0xec],
        ), /* l l */
        (
            1_133_333_333.nseconds(),
            33_333_334.nseconds(),
            [0xef, 0x80],
        ), /* o nil */
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
        assert_eq!(e.2, &*data);
    }

    /* Padding */
    loop {
        let outbuf = h.pull().unwrap();
        if outbuf.pts().unwrap() + outbuf.duration().unwrap() >= 2.seconds() {
            break;
        }

        let data = outbuf.map_readable().unwrap();
        assert_eq!(&*data, &[0x80, 0x80]);
    }

    let expected: [(ClockTime, ClockTime, [u8; 2usize]); 3] = [
        (2_000_000_000.nseconds(), ClockTime::ZERO, [0x20, 0x57]), /* SPACE W */
        (2_000_000_000.nseconds(), ClockTime::ZERO, [0xef, 0xf2]), /* o r */
        (2_000_000_000.nseconds(), ClockTime::ZERO, [0xec, 0x64]), /* l d */
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
        assert_eq!(e.2, &*data);
    }

    assert_eq!(h.buffers_in_queue(), 0);

    h.push_event(gst::event::Eos::new());

    assert_eq!(h.events_in_queue(), 1);

    let event = h.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Eos);
}

/* Here we test that tttocea608 introduces carriage returns in
 * judicious places and avoids to break words without rhyme or
 * reason.
 */
#[test]
fn test_word_wrap_roll_up() {
    init();

    let mut h = gst_check::Harness::new_parse("tttocea608 mode=roll-up2 origin-column=24");
    h.set_src_caps_str("text/x-raw,format=utf8");

    while h.events_in_queue() != 0 {
        let _event = h.pull_event().unwrap();
    }

    let inbuf = new_timed_buffer("Hello World", ClockTime::SECOND, ClockTime::SECOND);
    assert_eq!(h.push(inbuf), Ok(gst::FlowSuccess::Ok));

    let expected: [(ClockTime, ClockTime, [u8; 2usize]); 11] = [
        (
            1_000_000_000.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x25],
        ), /* roll_up_2 */
        (
            1_033_333_333.nseconds(),
            33_333_334.nseconds(),
            [0x94, 0x7c],
        ), /* preamble */
        (
            1_066_666_667.nseconds(),
            33_333_333.nseconds(),
            [0xc8, 0xe5],
        ), /* H e */
        (
            1_100_000_000.nseconds(),
            33_333_333.nseconds(),
            [0xec, 0xec],
        ), /* l l */
        (
            1_133_333_333.nseconds(),
            33_333_334.nseconds(),
            [0xef, 0x20],
        ), /* o SPACE */
        (
            1_166_666_667.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0xad],
        ), /* carriage return */
        (
            1_200_000_000.nseconds(),
            33_333_333.nseconds(),
            [0x94, 0x25],
        ), /* roll_up_2 */
        (
            1_233_333_333.nseconds(),
            33_333_334.nseconds(),
            [0x94, 0x7c],
        ), /* preamble */
        (
            1_266_666_667.nseconds(),
            33_333_333.nseconds(),
            [0x57, 0xef],
        ), /* W o */
        (
            1_300_000_000.nseconds(),
            33_333_333.nseconds(),
            [0xf2, 0xec],
        ), /* r l */
        (
            1_333_333_333.nseconds(),
            33_333_334.nseconds(),
            [0x64, 0x80],
        ), /* d nil */
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
        assert_eq!(e.2, &*data);
    }
}
