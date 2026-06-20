// Copyright (C) 2022 LTN Global Communications, Inc.
// Contact: Jan Alexander Steffens (heftig) <jan.steffens@ltnglobal.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstlivesync::plugin_register_static().expect("Failed to register livesync plugin");
    });
}

const DURATION: gst::ClockTime = gst::ClockTime::from_mseconds(100);
const LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(200);
const SEGMENT_OFFSET: gst::ClockTime = gst::ClockTime::from_seconds(60 * 60 * 1000);

fn crank_pull(harness: &mut gst_check::Harness) -> gst::Buffer {
    harness.crank_single_clock_wait().unwrap();
    harness.pull().unwrap()
}

#[track_caller]
fn assert_buf(
    buf: &gst::BufferRef,
    offset: u64,
    pts: gst::ClockTime,
    duration: gst::ClockTime,
    flags: gst::BufferFlags,
) {
    assert_eq!(buf.offset(), offset, "Bad offset");
    assert_eq!(buf.pts(), Some(pts), "Bad PTS");
    assert_eq!(buf.duration(), Some(duration), "Bad duration");
    assert_eq!(
        buf.flags() - gst::BufferFlags::TAG_MEMORY,
        flags,
        "Bad flags",
    );
}

#[track_caller]
fn assert_crank_pull(
    harness: &mut gst_check::Harness,
    offset_per_buffer: u64,
    src_buffer_number: u64,
    sink_buffer_number: u64,
    flags: gst::BufferFlags,
    singlesegment: bool,
) {
    let pts = if singlesegment {
        LATENCY + DURATION * sink_buffer_number + SEGMENT_OFFSET
    } else {
        DURATION * sink_buffer_number
    };
    assert_buf(
        &crank_pull(harness),
        offset_per_buffer * src_buffer_number,
        pts,
        DURATION,
        flags,
    );
}

#[test]
fn test_video_singlesegment() {
    test_video(true);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/328
#[ignore]
fn test_audio_singlesegment() {
    test_audio(true);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/357
#[ignore]
fn test_video_nonsinglesegment() {
    test_video(false);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/328
#[ignore]
fn test_audio_nonsinglesegment() {
    test_audio(false);
}

fn test_video(singlesegment: bool) {
    init();

    let mut h = gst_check::Harness::new("livesync");
    h.add_src_parse(
        r"videotestsrc is-live=1
          ! capsfilter caps=video/x-raw,framerate=10/1
        ",
        true,
    );

    let element = h.element().unwrap();
    element.set_property("latency", LATENCY);
    element.set_property("single-segment", singlesegment);

    test_livesync(&mut h, 1, singlesegment);
}

fn test_audio(singlesegment: bool) {
    init();

    let mut h = gst_check::Harness::new("livesync");
    h.add_src_parse(
        r"audiotestsrc is-live=1 samplesperbuffer=4800
          ! capsfilter caps=audio/x-raw,rate=48000
        ",
        true,
    );

    let element = h.element().unwrap();
    element.set_property("latency", LATENCY);
    element.set_property("single-segment", singlesegment);

    test_livesync(&mut h, 4800, singlesegment);
}

fn test_livesync(h: &mut gst_check::Harness, o: u64, singlesegment: bool) {
    // Normal operation ------------------------------

    // Push frames 0-1, pull frame 0
    h.push_from_src().unwrap();
    h.push_from_src().unwrap();
    assert_eq!(h.pull_event().unwrap().type_(), gst::EventType::StreamStart);
    // Caps are only output once waiting for the first buffer has finished
    h.crank_single_clock_wait().unwrap();
    assert_eq!(h.pull_event().unwrap().type_(), gst::EventType::Caps);
    assert_eq!(h.pull_event().unwrap().type_(), gst::EventType::Segment);
    assert_crank_pull(h, o, 0, 0, gst::BufferFlags::DISCONT, singlesegment);

    // Push frames 2-10, pull frames 1-9
    for i in 1..=9 {
        h.push_from_src().unwrap();
        assert_crank_pull(h, o, i, i, gst::BufferFlags::empty(), singlesegment);
    }

    // Pull frame 10
    assert_crank_pull(h, o, 10, 10, gst::BufferFlags::empty(), singlesegment);

    // Bridging gap ----------------------------------

    // Pull frames 11-19
    for i in 11..=19 {
        assert_crank_pull(h, o, 10, i, gst::BufferFlags::GAP, singlesegment);
    }

    // Push frames 11-19
    for _ in 11..=19 {
        h.push_from_src().unwrap();
    }

    // Normal operation ------------------------------

    // Push frames 20-21, pull frame 20
    for _ in 1..=2 {
        let mut src_h = h.src_harness_mut().unwrap();
        src_h.crank_single_clock_wait().unwrap();
        let mut buf = src_h.pull().unwrap();
        let buf_mut = buf.make_mut();
        buf_mut.set_flags(gst::BufferFlags::MARKER);
        h.push(buf).unwrap();
    }
    assert_crank_pull(h, o, 10, 20, gst::BufferFlags::GAP, singlesegment);

    // Push frame 22, pull frame 21
    h.push_from_src().unwrap();
    assert_crank_pull(
        h,
        o,
        21,
        21,
        gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER,
        singlesegment,
    );

    // Push frames 23-30, pull frames 22-29
    for i in 22..=29 {
        h.push_from_src().unwrap();
        assert_crank_pull(h, o, i, i, gst::BufferFlags::empty(), singlesegment);
    }

    // EOS -------------------------------------------
    assert!(h.push_event(gst::event::Eos::new()));

    // Pull frame 30
    assert_crank_pull(h, o, 30, 30, gst::BufferFlags::empty(), singlesegment);

    assert_eq!(h.pull_event().unwrap().type_(), gst::EventType::Eos);
    assert_eq!(h.try_pull(), None);
}
