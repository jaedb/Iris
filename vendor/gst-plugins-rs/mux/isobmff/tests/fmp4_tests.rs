// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//

use gst::prelude::*;
use mp4_atom::{Atom, ReadAtom as _, ReadFrom as _};
use tempfile::tempdir;

use std::{fs::File, path::Path, thread};

pub mod support;
use support::{check_ftyp_output, check_mvhd_sanity, ExpectedConfiguration};

use crate::support::check_trak_sanity;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstisobmff::plugin_register_static().unwrap();
    });
}

fn to_completion(pipeline: &gst::Pipeline) {
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }

    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}

fn check_fragment_header(harness: &mut gst_check::Harness) {
    let buf = harness.pull().unwrap();
    assert_eq!(buf.flags(), gst::BufferFlags::HEADER);
}

fn check_first_fragment_header(harness: &mut gst_check::Harness) {
    let buf = harness.pull().unwrap();
    assert_eq!(
        buf.flags(),
        gst::BufferFlags::DISCONT | gst::BufferFlags::HEADER
    );
    check_fragment_header(harness)
}

fn test_buffer_flags_single_stream(cmaf: bool, set_dts: bool, caps: gst::Caps) {
    let mut h = if cmaf {
        gst_check::Harness::new("cmafmux")
    } else {
        gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"))
    };

    // 5s fragment duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h.set_src_caps(caps);
    h.play();

    let output_offset = if cmaf {
        gst::ClockTime::ZERO
    } else {
        (60 * 60 * 1000).seconds()
    };

    // Push 7 buffers of 1s each, 1st and 6 buffer without DELTA_UNIT flag
    for i in 0..7 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            if set_dts {
                buffer.set_dts(i.seconds());
            }
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 5 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    if set_dts {
        assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));
    }

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    if set_dts {
        assert_eq!(
            fragment_header.dts(),
            Some(gst::ClockTime::ZERO + output_offset)
        );
    }
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 0..5 {
        let buffer = h.pull().unwrap();
        if i == 4 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        if set_dts {
            assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        }
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    h.push_event(gst::event::Eos::new());

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds() + output_offset));
    if set_dts {
        assert_eq!(fragment_header.dts(), Some(5.seconds() + output_offset));
    }
    assert_eq!(fragment_header.duration(), Some(2.seconds()));

    for i in 5..7 {
        let buffer = h.pull().unwrap();
        if i == 6 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        if set_dts {
            assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        }
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_buffer_flags_single_h264_stream_cmaf() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    test_buffer_flags_single_stream(true, true, caps);
}

#[test]
fn test_buffer_flags_single_h264_stream_iso() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    test_buffer_flags_single_stream(false, true, caps);
}

#[test]
fn test_buffer_flags_single_vp9_stream_iso() {
    init();

    let caps = gst::Caps::builder("video/x-vp9")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("profile", "0")
        .field("chroma-format", "4:2:0")
        .field("bit-depth-luma", 8u32)
        .field("bit-depth-chroma", 8u32)
        .field("colorimetry", "bt709")
        .build();

    test_buffer_flags_single_stream(false, false, caps);
}

#[test]
fn test_buffer_flags_single_av1_stream_cmaf() {
    init();

    let caps = gst::Caps::builder("video/x-av1")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("profile", "main")
        .field("tier", "main")
        .field("level", "4.1")
        .field("chroma-format", "4:2:0")
        .field("bit-depth-luma", 8u32)
        .field("bit-depth-chroma", 8u32)
        .field("colorimetry", "bt709")
        .build();

    test_buffer_flags_single_stream(true, false, caps);
}

#[test]
fn test_buffer_flags_multi_stream() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h2.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 7 buffers of 1s each, 1st and last buffer without DELTA_UNIT flag
    for i in 0..7 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 5 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h1.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );

            let ev = loop {
                let ev = h2.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h1.crank_single_clock_wait().unwrap();

    let header = h1.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 0..5 {
        for j in 0..2 {
            let buffer = h1.pull().unwrap();
            if i == 4 && j == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }

            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));

            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(2.seconds()));

    for i in 5..7 {
        for j in 0..2 {
            let buffer = h1.pull().unwrap();
            if i == 6 && j == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_live_timeout() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    h1.use_testclock();

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h2.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 7 buffers of 1s each, 1st and last buffer without DELTA_UNIT flag
    for i in 0..7 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 5 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        // Skip buffer 4th and 6th buffer (end of fragment / stream)
        if i == 4 || i == 6 {
            continue;
        } else {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(i.seconds());
                buffer.set_dts(i.seconds());
                buffer.set_duration(gst::ClockTime::SECOND);
            }
            assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
        }

        if i == 2 {
            let ev = loop {
                let ev = h1.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );

            let ev = loop {
                let ev = h2.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h1.crank_single_clock_wait().unwrap();

    let header = h1.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 0..5 {
        for j in 0..2 {
            // Skip gap events that don't result in buffers
            if j == 1 && i == 4 {
                // Advance time and crank the clock another time. This brings us at the end of the
                // EOS.
                h1.crank_single_clock_wait().unwrap();
                continue;
            }

            let buffer = h1.pull().unwrap();
            if i == 4 && j == 0 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else if i == 5 && j == 0 {
                assert_eq!(buffer.flags(), gst::BufferFlags::HEADER);
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }

            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));

            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(2.seconds()));

    for i in 5..7 {
        for j in 0..2 {
            // Skip gap events that don't result in buffers
            if j == 1 && i == 6 {
                continue;
            }

            let buffer = h1.pull().unwrap();
            if i == 6 && j == 0 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_gap_events() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    h1.use_testclock();

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h2.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 7 buffers of 1s each, 1st and last buffer without DELTA_UNIT flag
    for i in 0..7 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 5 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        // Replace buffer 3 and 6 with a gap event
        if i == 3 || i == 6 {
            let ev = gst::event::Gap::builder(i.seconds())
                .duration(gst::ClockTime::SECOND)
                .build();
            assert!(h2.push_event(ev));
        } else {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(i.seconds());
                buffer.set_dts(i.seconds());
                buffer.set_duration(gst::ClockTime::SECOND);
            }
            assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
        }

        if i == 2 {
            let ev = loop {
                let ev = h1.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );

            let ev = loop {
                let ev = h2.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h1.crank_single_clock_wait().unwrap();

    let header = h1.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 0..5 {
        for j in 0..2 {
            // Skip gap events that don't result in buffers
            if j == 1 && i == 3 {
                continue;
            }

            let buffer = h1.pull().unwrap();
            if i == 4 && j == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }

            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));

            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(5.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(2.seconds()));

    for i in 5..7 {
        for j in 0..2 {
            // Skip gap events that don't result in buffers
            if j == 1 && i == 6 {
                continue;
            }

            let buffer = h1.pull().unwrap();
            if i == 6 && j == 0 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_single_stream_short_gops() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    // 5s fragment duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 8 buffers of 1s each, 1st, 4th and 7th buffer without DELTA_UNIT flag
    for i in 0..8 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 3 && i != 6 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 || i == 7 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            let fku_time = if i == 2 { 5.seconds() } else { 8.seconds() };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(fku_time),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(3.seconds()));

    for i in 0..3 {
        let buffer = h.pull().unwrap();
        if i == 2 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    h.push_event(gst::event::Eos::new());

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(3.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(3.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 3..8 {
        let buffer = h.pull().unwrap();
        if i == 7 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_single_stream_long_gops() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    // 5s fragment duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 10 buffers of 1s each, 1st and 7th buffer without DELTA_UNIT flag
    for i in 0..10 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 6 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 || i == 7 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            let fku_time = if i == 2 { 5.seconds() } else { 11.seconds() };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(fku_time),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(6.seconds()));

    for i in 0..6 {
        let buffer = h.pull().unwrap();
        if i == 5 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    h.push_event(gst::event::Eos::new());

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(6.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(6.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(4.seconds()));

    for i in 6..10 {
        let buffer = h.pull().unwrap();
        if i == 9 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_buffer_multi_stream_short_gops() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h2.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 9 buffers of 1s each, 1st, 4th and 7th buffer without DELTA_UNIT flag
    for i in 0..9 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 3 && i != 6 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 || i == 8 {
            let ev = loop {
                let ev = h1.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            let fku_time = if i == 2 { 5.seconds() } else { 8.seconds() };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(fku_time),
                    all_headers: true,
                    count: 0
                }
            );

            let ev = loop {
                let ev = h2.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(fku_time),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    let header = h1.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(
        fragment_header.pts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(
        fragment_header.dts(),
        Some(gst::ClockTime::ZERO + output_offset)
    );
    assert_eq!(fragment_header.duration(), Some(3.seconds()));

    for i in 0..3 {
        for j in 0..2 {
            let buffer = h1.pull().unwrap();
            if i == 2 && j == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }

            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));

            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());

    let fragment_header = h1.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(3.seconds() + output_offset));
    assert_eq!(fragment_header.dts(), Some(3.seconds() + output_offset));
    assert_eq!(fragment_header.duration(), Some(6.seconds()));

    for i in 3..9 {
        for j in 0..2 {
            let buffer = h1.pull().unwrap();
            if i == 8 && j == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(i.seconds() + output_offset));
            if j == 0 {
                assert_eq!(buffer.dts(), Some(i.seconds() + output_offset));
            } else {
                assert!(buffer.dts().is_none());
            }
            assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
        }
    }

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_single_stream_manual_fragment() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // fragment duration long enough to be ignored, 1s chunk duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.hours());

    h.set_src_caps(caps);
    h.play();

    // request fragment at 4 seconds, should be created at 11th buffer
    h.element()
        .unwrap()
        .emit_by_name::<()>("split-at-running-time", &[&4.seconds()]);

    // Push 15 buffers of 0.5s each, 1st, 11th and 16th buffer without DELTA_UNIT flag
    for i in 0..20 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 10 && i != 15 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(4.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO));

    // first fragment
    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(fragment_header.dts(), Some(gst::ClockTime::ZERO));
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for buffer_idx in 0..10 {
        let buffer = h.pull().unwrap();
        if buffer_idx == 9 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(buffer_idx * 500.mseconds()));
        assert_eq!(buffer.dts(), Some(buffer_idx * 500.mseconds()));
        assert_eq!(buffer.duration(), Some(500.mseconds()));
    }

    // second manual fragment
    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds()));
    assert_eq!(fragment_header.dts(), Some(5.seconds()));
    assert_eq!(fragment_header.duration(), Some(2500.mseconds()));

    for buffer_idx in 0..5 {
        let buffer = h.pull().unwrap();
        if buffer_idx == 4 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(
            buffer.pts(),
            Some(5.seconds() + buffer_idx * 500.mseconds())
        );
        assert_eq!(
            buffer.dts(),
            Some(5.seconds() + buffer_idx * 500.mseconds())
        );
        assert_eq!(buffer.duration(), Some(500.mseconds()));
    }

    h.push_event(gst::event::Eos::new());

    // There should be the second fragment now
    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(7500.mseconds()));
    assert_eq!(fragment_header.dts(), Some(7500.mseconds()));
    assert_eq!(fragment_header.duration(), Some(2500.mseconds()));

    for buffer_idx in 0..5 {
        let buffer = h.pull().unwrap();
        if buffer_idx == 4 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(
            buffer.pts(),
            Some(7500.mseconds() + buffer_idx * 500.mseconds())
        );
        assert_eq!(
            buffer.dts(),
            Some(7500.mseconds() + buffer_idx * 500.mseconds())
        );
        assert_eq!(buffer.duration(), Some(500.mseconds()));
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_single_stream_manual_fragment() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // fragment duration long enough to be ignored, 1s chunk duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.hours());
    h.element()
        .unwrap()
        .set_property("chunk-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    // request fragment at 4 seconds, should be created at 11th buffer
    h.element()
        .unwrap()
        .emit_by_name::<()>("split-at-running-time", &[&4.seconds()]);

    // Push 15 buffers of 0.5s each, 1st and 11th buffer without DELTA_UNIT flag
    for i in 0..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 10 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(4.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO));

    // There should be 7 chunks now, and the 1st and 6th are starting a fragment.
    // Each chunk should have two buffers.
    for chunk in 0..7 {
        let chunk_header = h.pull().unwrap();
        if chunk == 0 || chunk == 5 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }
        assert_eq!(chunk_header.pts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.dts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.duration(), Some(1.seconds()));

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            if buffer_idx == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(
                buffer.pts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(
                buffer.dts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(buffer.duration(), Some(500.mseconds()));
        }
    }

    h.push_event(gst::event::Eos::new());

    // There should be the remaining chunk now, containing one 500ms buffer.
    for chunk in 7..8 {
        let chunk_header = h.pull().unwrap();
        assert_eq!(
            chunk_header.flags(),
            gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
        );
        assert_eq!(chunk_header.pts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.dts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.duration(), Some(500.mseconds()));

        for buffer_idx in 0..1 {
            let buffer = h.pull().unwrap();
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
            assert_eq!(
                buffer.pts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(
                buffer.dts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(buffer.duration(), Some(500.mseconds()));
        }
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_single_stream() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // 5s fragment duration, 1s chunk duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());
    h.element()
        .unwrap()
        .set_property("chunk-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    // Push 15 buffers of 0.5s each, 1st and 11th buffer without DELTA_UNIT flag
    for i in 0..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 10 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO));

    // There should be 7 chunks now, and the 1st and 6th are starting a fragment.
    // Each chunk should have two buffers.
    for chunk in 0..7 {
        let chunk_header = h.pull().unwrap();
        if chunk == 0 || chunk == 5 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }
        assert_eq!(chunk_header.pts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.dts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.duration(), Some(1.seconds()));

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            if buffer_idx == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(
                buffer.pts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(
                buffer.dts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(buffer.duration(), Some(500.mseconds()));
        }
    }

    h.push_event(gst::event::Eos::new());

    // There should be the remaining chunk now, containing one 500ms buffer.
    for chunk in 7..8 {
        let chunk_header = h.pull().unwrap();
        assert_eq!(
            chunk_header.flags(),
            gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
        );
        assert_eq!(chunk_header.pts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.dts(), Some(chunk * 1.seconds()));
        assert_eq!(chunk_header.duration(), Some(500.mseconds()));

        for buffer_idx in 0..1 {
            let buffer = h.pull().unwrap();
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
            assert_eq!(
                buffer.pts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(
                buffer.dts(),
                Some((chunk * 2 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(buffer.duration(), Some(500.mseconds()));
        }
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_multi_stream() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration, 1s chunk duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());
    h1.element()
        .unwrap()
        .set_property("chunk-duration", 1.seconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h2.play();

    let output_offset = (60 * 60 * 1000).seconds();

    // Push 15 buffers of 0.5s each, 1st and 11th buffer without DELTA_UNIT flag
    for i in 0..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 10 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h1.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );

            let ev = loop {
                let ev = h2.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h1.crank_single_clock_wait().unwrap();

    let header = h1.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO + output_offset));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO + output_offset));

    // There should be 7 chunks now, and the 1st and 6th are starting a fragment.
    // Each chunk should have two buffers.
    for chunk in 0..7 {
        let chunk_header = h1.pull().unwrap();
        if chunk == 0 || chunk == 5 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }
        assert_eq!(
            chunk_header.pts(),
            Some(chunk * 1.seconds() + output_offset)
        );
        assert_eq!(
            chunk_header.dts(),
            Some(chunk * 1.seconds() + output_offset)
        );
        assert_eq!(chunk_header.duration(), Some(1.seconds()));

        for buffer_idx in 0..2 {
            for stream_idx in 0..2 {
                let buffer = h1.pull().unwrap();
                if buffer_idx == 1 && stream_idx == 1 {
                    assert_eq!(
                        buffer.flags(),
                        gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                    );
                } else {
                    assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
                }
                assert_eq!(
                    buffer.pts(),
                    Some((chunk * 2 + buffer_idx) * 500.mseconds() + output_offset)
                );

                if stream_idx == 0 {
                    assert_eq!(
                        buffer.dts(),
                        Some((chunk * 2 + buffer_idx) * 500.mseconds() + output_offset)
                    );
                } else {
                    assert!(buffer.dts().is_none());
                }
                assert_eq!(buffer.duration(), Some(500.mseconds()));
            }
        }
    }

    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());

    // There should be the remaining chunk now, containing one 500ms buffer.
    for chunk in 7..8 {
        let chunk_header = h1.pull().unwrap();
        assert_eq!(
            chunk_header.flags(),
            gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
        );
        assert_eq!(
            chunk_header.pts(),
            Some(chunk * 1.seconds() + output_offset)
        );
        assert_eq!(
            chunk_header.dts(),
            Some(chunk * 1.seconds() + output_offset)
        );
        assert_eq!(chunk_header.duration(), Some(500.mseconds()));

        for buffer_idx in 0..1 {
            for stream_idx in 0..2 {
                let buffer = h1.pull().unwrap();
                if buffer_idx == 0 && stream_idx == 1 {
                    assert_eq!(
                        buffer.flags(),
                        gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                    );
                } else {
                    assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
                }

                assert_eq!(
                    buffer.pts(),
                    Some((chunk * 2 + buffer_idx) * 500.mseconds() + output_offset)
                );
                if stream_idx == 0 {
                    assert_eq!(
                        buffer.dts(),
                        Some((chunk * 2 + buffer_idx) * 500.mseconds() + output_offset)
                    );
                } else {
                    assert!(buffer.dts().is_none());
                }
                assert_eq!(buffer.duration(), Some(500.mseconds()));
            }
        }
    }

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_single_stream_gops_after_fragment_end_before_next_chunk_end() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // 5s fragment duration, 1s chunk duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());
    h.element()
        .unwrap()
        .set_property("chunk-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    // Push 15 buffers of 0.5s each, 1st and 12th buffer without DELTA_UNIT flag
    for i in 0..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 11 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let mut expected_ts = gst::ClockTime::ZERO;
    let mut num_buffers = 0;

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(expected_ts));
    assert_eq!(header.dts(), Some(expected_ts));

    // There should be 7 chunks now, and the 1st and 7th are starting a fragment.
    // Each chunk should have two buffers except for the 6th.
    for chunk in 0..7 {
        let chunk_header = h.pull().unwrap();
        if chunk == 0 || chunk == 6 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }
        assert_eq!(chunk_header.pts(), Some(expected_ts));
        assert_eq!(chunk_header.dts(), Some(expected_ts));
        if chunk == 5 {
            assert_eq!(chunk_header.duration(), Some(500.mseconds()));
        } else {
            assert_eq!(chunk_header.duration(), Some(1.seconds()));
        }

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            num_buffers += 1;
            if buffer_idx == 1 || (chunk == 5 && buffer_idx == 0) {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(expected_ts));
            assert_eq!(buffer.dts(), Some(expected_ts));
            assert_eq!(buffer.duration(), Some(500.mseconds()));

            expected_ts += 500.mseconds();

            // Only one buffer in this chunk
            if chunk == 5 && buffer_idx == 0 {
                break;
            }
        }
    }

    h.push_event(gst::event::Eos::new());

    // There should be one remaining chunk now, containing two 500ms buffer.
    for _chunk in 7..8 {
        let chunk_header = h.pull().unwrap();
        assert_eq!(
            chunk_header.flags(),
            gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
        );
        assert_eq!(chunk_header.pts(), Some(expected_ts));
        assert_eq!(chunk_header.dts(), Some(expected_ts));
        assert_eq!(chunk_header.duration(), Some(1.seconds()));

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            num_buffers += 1;
            if buffer_idx == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(expected_ts));
            assert_eq!(buffer.dts(), Some(expected_ts));
            assert_eq!(buffer.duration(), Some(500.mseconds()));
            expected_ts += 500.mseconds();
        }
    }

    assert_eq!(num_buffers, 15);

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_single_stream_gops_after_fragment_end_after_next_chunk_end() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // 5s fragment duration, 1s chunk duration
    h.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());
    h.element()
        .unwrap()
        .set_property("chunk-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    // Push 15 buffers of 0.5s each, 1st and 14th buffer without DELTA_UNIT flag
    for i in 0..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i != 0 && i != 13 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i == 2 {
            let ev = loop {
                let ev = h.pull_upstream_event().unwrap();
                if ev.type_() != gst::EventType::Reconfigure
                    && ev.type_() != gst::EventType::Latency
                {
                    break ev;
                }
            };

            assert_eq!(ev.type_(), gst::EventType::CustomUpstream);
            assert_eq!(
                gst_video::UpstreamForceKeyUnitEvent::parse(&ev).unwrap(),
                gst_video::UpstreamForceKeyUnitEvent {
                    running_time: Some(5.seconds()),
                    all_headers: true,
                    count: 0
                }
            );
        }
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let mut expected_ts = gst::ClockTime::ZERO;
    let mut num_buffers = 0;

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(expected_ts));
    assert_eq!(header.dts(), Some(expected_ts));

    // There should be 7 chunks now, and the 1st is starting a fragment.
    // Each chunk should have two buffers except for the 7th.
    for chunk in 0..7 {
        let chunk_header = h.pull().unwrap();
        if chunk == 0 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }
        assert_eq!(chunk_header.pts(), Some(expected_ts));
        assert_eq!(chunk_header.dts(), Some(expected_ts));
        if chunk == 6 {
            assert_eq!(chunk_header.duration(), Some(500.mseconds()));
        } else {
            assert_eq!(chunk_header.duration(), Some(1.seconds()));
        }

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            num_buffers += 1;
            if buffer_idx == 1 || (chunk == 6 && buffer_idx == 0) {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(expected_ts));
            assert_eq!(buffer.dts(), Some(expected_ts));
            assert_eq!(buffer.duration(), Some(500.mseconds()));

            expected_ts += 500.mseconds();

            // Only one buffer in this chunk
            if chunk == 6 && buffer_idx == 0 {
                break;
            }
        }
    }

    h.push_event(gst::event::Eos::new());

    // There should be two remaining chunks now, containing two 500ms buffers.
    // This should start a new fragment.
    for _chunk in 7..8 {
        let chunk_header = h.pull().unwrap();
        assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        assert_eq!(chunk_header.pts(), Some(expected_ts));
        assert_eq!(chunk_header.dts(), Some(expected_ts));
        assert_eq!(chunk_header.duration(), Some(1.seconds()));

        for buffer_idx in 0..2 {
            let buffer = h.pull().unwrap();
            num_buffers += 1;
            if buffer_idx == 1 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }
            assert_eq!(buffer.pts(), Some(expected_ts));
            assert_eq!(buffer.dts(), Some(expected_ts));
            assert_eq!(buffer.duration(), Some(500.mseconds()));
            expected_ts += 500.mseconds();
        }
    }

    assert_eq!(num_buffers, 15);

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_chunking_on_keyframe_single_stream() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    h.element()
        .unwrap()
        .set_property("fragment-duration", 10.seconds());
    h.element()
        .unwrap()
        .set_property_from_str("chunk-mode", "keyframe");

    h.set_src_caps(caps);
    h.play();

    for i in 0..20 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            // Buffer every 0.5ms and key frame every 2s.
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 500.mseconds());
            buffer.set_dts(i * 500.mseconds());
            buffer.set_duration(500.mseconds());
            if i % 4 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }

        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );

    // For 10s fragment duration with key frame every 2s, we expect 4 chunks.
    for chunk in 0..4 {
        let chunk_header = h.pull().unwrap();

        if chunk == 0 {
            assert_eq!(chunk_header.flags(), gst::BufferFlags::HEADER);
        } else {
            assert_eq!(
                chunk_header.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            );
        }

        assert_eq!(chunk_header.pts(), Some(chunk * 2.seconds()));
        assert_eq!(chunk_header.dts(), Some(chunk * 2.seconds()));
        assert_eq!(chunk_header.duration(), Some(2.seconds()));

        for buffer_idx in 0..4 {
            let buffer = h.pull().unwrap();

            if buffer_idx == 3 {
                assert_eq!(
                    buffer.flags(),
                    gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT
                );
            } else {
                assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
            }

            assert_eq!(
                buffer.pts(),
                Some((chunk * 4 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(
                buffer.dts(),
                Some((chunk * 4 + buffer_idx) * 500.mseconds())
            );
            assert_eq!(buffer.duration(), Some(500.mseconds()));
        }
    }

    h.push_event(gst::event::Eos::new());

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_early_eos() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    for i in 0..5 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 100.mseconds());
            buffer.set_dts(i * 100.mseconds());
            buffer.set_duration(100.mseconds());

            buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    h.push_event(gst::event::Eos::new());
    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
fn test_roundtrip_vp9_flac() {
    init();

    let pipeline = gst::parse::launch(
        r#"
        videotestsrc num-buffers=99 ! vp9enc ! vp9parse ! mux.
        audiotestsrc num-buffers=149 ! flacenc ! flacparse ! mux.
        isofmp4mux name=mux ! qtdemux name=demux
        demux.audio_0 ! queue ! flacdec ! fakesink
        demux.video_0 ! queue ! vp9dec ! fakesink
        "#,
    )
    .unwrap();
    let pipeline = pipeline.downcast().unwrap();
    to_completion(&pipeline);
}

#[track_caller]
fn test_caps_changed_verify(
    h: &mut gst_check::Harness,
    num_bufs: usize,
    caps_changed: bool,
    chunk: bool,
) {
    for i in 0..num_bufs {
        let b = h.pull().unwrap();
        match (caps_changed, i, chunk) {
            (true, 0, _) => assert_eq!(
                b.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
            ),
            (false, 0, false) | (true, 1, false) => assert_eq!(b.flags(), gst::BufferFlags::HEADER),
            (false, 0, true) | (true, 1, true) => assert_eq!(
                b.flags(),
                gst::BufferFlags::HEADER | gst::BufferFlags::DELTA_UNIT
            ),
            (false, 1, _) | (_, 2.., _) => {
                if i == num_bufs - 1 {
                    assert_eq!(
                        b.flags(),
                        gst::BufferFlags::MARKER | gst::BufferFlags::DELTA_UNIT
                    );
                } else {
                    assert_eq!(b.flags(), gst::BufferFlags::DELTA_UNIT);
                }
            }
        }
    }
}

#[track_caller]
fn test_caps_changed_buffers(
    h: &mut gst_check::Harness,
    num_bufs: u64,
    gop_size: u64,
    caps_change: u64,
    duration: u64,
    key_frame_on_caps_change: bool,
    drop_first_buffer: bool,
) {
    for i in 0..num_bufs {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * duration.mseconds());
            buffer.set_dts(i * duration.mseconds());
            buffer.set_duration(duration.mseconds());

            if i % gop_size != 0 && (i != caps_change || !key_frame_on_caps_change) {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }

        if i == 0 && drop_first_buffer {
            continue;
        }

        if i == caps_change {
            let caps = gst::Caps::builder("video/x-h264")
                .field("width", 1280i32)
                .field("height", 720i32)
                .field("framerate", gst::Fraction::new(30, 1))
                .field("stream-format", "avc")
                .field("alignment", "au")
                .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
                .build();
            h.push_event(gst::event::Caps::new(&caps));
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }
}

#[test]
#[ignore]
fn test_caps_change_at_gop_boundary() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 30, 10, 10, 100, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // Full GOP with HEADER and DISCONT due to caps change
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
#[ignore]
fn test_language_change_at_gop_boundary() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    for i in 0..30 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            if i == 10 {
                let mut tl = gst::TagList::new();
                tl.make_mut()
                    .add::<gst::tags::LanguageCode>(&"eng", gst::TagMergeMode::Append);
                let ev = gst::event::Tag::builder(tl).build();
                h.push_event(ev);
            }

            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 100.mseconds());
            buffer.set_dts(i * 100.mseconds());
            buffer.set_duration(100.mseconds());

            if i % 10 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // Full GOP with HEADER due to language change
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
#[ignore]
fn test_caps_change_at_gop_boundary_multi_stream() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    h1.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps1 = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    let caps2 = gst::Caps::builder("video/x-h264")
        .field("width", 640)
        .field("height", 480)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([4, 3, 2, 1]))
        .build();

    h1.element()
        .unwrap()
        .set_property("fragment-duration", 330.mseconds());

    h1.set_src_caps(caps1);
    h1.play();
    h2.set_src_caps(caps2);
    h2.play();

    for i in 0..21 {
        // caps change on 5th and 20th buffer
        if let Some(caps) = match i {
            5 => Some(
                gst::Caps::builder("video/x-h264")
                    .field("width", 1280i32)
                    .field("height", 720i32)
                    .field("framerate", gst::Fraction::new(30, 1))
                    .field("stream-format", "avc")
                    .field("alignment", "au")
                    .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
                    .build(),
            ),
            20 => Some(
                gst::Caps::builder("video/x-h264")
                    .field("width", 1024)
                    .field("height", 800)
                    .field("framerate", gst::Fraction::new(30, 1))
                    .field("stream-format", "avc")
                    .field("alignment", "au")
                    .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
                    .build(),
            ),
            _ => None,
        } {
            h1.push_event(gst::event::Caps::new(&caps));
        }

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 33.mseconds());
            buffer.set_dts(i * 33.mseconds());
            buffer.set_duration(33.mseconds());

            // GOP size of 5
            if i % 5 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(2).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 33.mseconds());
            buffer.set_dts(i * 33.mseconds());
            buffer.set_duration(33.mseconds());

            // GOP size of 7
            if i % 7 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i != 5 {
            continue;
        }

        // reconfigure event
        h1.try_pull_upstream_event().unwrap();
        h2.try_pull_upstream_event().unwrap();
        let ev1 = h1.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent {
                running_time: Some(330.mseconds()),
                all_headers: true,
                count: 0
            }
        );
        let ev2 = h2.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev2).unwrap()
        );
        let ev1 = h1.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent {
                running_time: Some(495.mseconds()),
                all_headers: true,
                count: 0
            }
        );
        let ev2 = h2.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev2).unwrap()
        );
        let ev1 = h1.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent {
                running_time: Some(165.mseconds()),
                all_headers: true,
                count: 0
            }
        );
        let ev2 = h2.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev2).unwrap()
        );
    }

    h1.crank_single_clock_wait().unwrap();
    // Caps change on 6th buffer, 1st stream has complete GOP but 2nd
    // stream GOP is incomplete. Still 2nd stream buffers are pushed
    // because the stream would be missing in the fragment otherwise.
    test_caps_changed_verify(&mut h1, 1 + 1 + 5 + 4, true, false);

    h1.crank_single_clock_wait().unwrap();
    // Caps change on 20th buffer, both streams have complete GOPs, 3
    // from stream #1 and the rest of the old GOP and one complete GOP
    // from stream #2.
    test_caps_changed_verify(&mut h1, 1 + 1 + 5 + 5 + 5 + 3 + 7, true, false);

    // On stream #1 a FKU event for partial GOP and next GOP and on
    // stream #2 just the next GOP FKU because no partial GOP has been
    // pushed.
    assert_eq!(h1.upstream_events_in_queue(), 2);
    assert_eq!(h2.upstream_events_in_queue(), 1);

    h1.crank_single_clock_wait().unwrap();
    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());
    test_caps_changed_verify(&mut h1, 1 + 1 + 8, true, false);

    assert_eq!(h1.buffers_in_queue(), 0);
}

#[test]
fn test_caps_change_at_gop_boundary_chunked_multi_stream() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    h1.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps1 = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    let caps2 = gst::Caps::builder("video/x-h264")
        .field("width", 640)
        .field("height", 480)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([4, 3, 2, 1]))
        .build();

    h1.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());
    h1.element()
        .unwrap()
        .set_property("chunk-duration", 250.mseconds());

    h1.set_src_caps(caps1);
    h1.play();
    h2.set_src_caps(caps2);
    h2.play();

    for i in 0..19 {
        // caps change on 10th and 20th buffer
        if let Some(caps) = match i {
            10 => Some(
                gst::Caps::builder("video/x-h264")
                    .field("width", 1280i32)
                    .field("height", 720i32)
                    .field("framerate", gst::Fraction::new(30, 1))
                    .field("stream-format", "avc")
                    .field("alignment", "au")
                    .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
                    .build(),
            ),
            _ => None,
        } {
            h1.push_event(gst::event::Caps::new(&caps));
        }

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 33.mseconds());
            buffer.set_dts(i * 33.mseconds());
            buffer.set_duration(33.mseconds());

            // GOP size of 5
            if i % 5 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(2).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 33.mseconds());
            buffer.set_dts(i * 33.mseconds());
            buffer.set_duration(33.mseconds());

            // GOP size of 7
            if i % 7 != 0 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i != 5 {
            continue;
        }

        // reconfigure event
        h1.try_pull_upstream_event().unwrap();
        h2.try_pull_upstream_event().unwrap();
        let ev1 = h1.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent {
                running_time: Some(1.seconds()),
                all_headers: true,
                count: 0
            }
        );
        let ev2 = h2.pull_upstream_event().unwrap();
        assert_eq!(
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
            gst_video::UpstreamForceKeyUnitEvent::parse(&ev2).unwrap()
        );
    }

    h1.crank_single_clock_wait().unwrap();
    // Fragment start chunk
    test_caps_changed_verify(&mut h1, 1 + 1 + 8 + 8, true, false);

    h1.crank_single_clock_wait().unwrap();
    // Early end of chunk due to caps change
    test_caps_changed_verify(&mut h1, 1 + 2 + 1, false, true);

    // Signalling new keyunit for next fragment
    let ev1 = h1.pull_upstream_event().unwrap();
    assert_eq!(
        gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
        gst_video::UpstreamForceKeyUnitEvent {
            running_time: Some(1330.mseconds()),
            all_headers: true,
            count: 0
        }
    );

    // Signalling new keyunit on stream with caps change
    let ev1 = h1.pull_upstream_event().unwrap();
    assert_eq!(
        gst_video::UpstreamForceKeyUnitEvent::parse(&ev1).unwrap(),
        gst_video::UpstreamForceKeyUnitEvent {
            running_time: Some(330.mseconds()),
            all_headers: true,
            count: 0
        }
    );

    h1.crank_single_clock_wait().unwrap();
    // The first chunk of the new fragment
    test_caps_changed_verify(&mut h1, 1 + 1 + 8 + 9, true, false);

    h1.crank_single_clock_wait().unwrap();
    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());
    // The final chunk from EOS
    test_caps_changed_verify(&mut h1, 1 + 1 + 1, false, true);

    assert_eq!(h1.buffers_in_queue(), 0);
}

#[test]
fn test_caps_change_at_gop_boundary_compatible() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1280i32)
        .field("height", 720i32)
        .field("framerate", gst::Fraction::new(10, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 30, 10, 10, 100, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // Full GOP with HEADER but no DISCONT because compatible caps
    // change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
fn test_caps_change_at_gop_boundary_not_allowed() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "rewrite");

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 30, 10, 10, 100, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // Full GOP with HEADER but no DISCONT because caps change not
    // allowed from header-update-modex
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
#[ignore]
fn test_caps_change_within_gop() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 20, 10, 5, 100, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);

    h.crank_single_clock_wait().unwrap();
    // Reduced GOP with HEADER and DISCONT due to caps change
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
fn test_caps_change_within_gop_start_without_key() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 20, 10, 5, 100, true, true);

    // Same as test_caps_change_within_gop() but without the first
    // fragment since all frames are dropped due to missing key frame

    h.crank_single_clock_wait().unwrap();
    // Reduced GOP with HEADER and DISCONT due to caps change
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Full GOP with HEADER but no DISCONT because no caps change
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
#[ignore]
fn test_caps_change_within_gop_chunked() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());
    h.element()
        .unwrap()
        .set_property("chunk-duration", 300.mseconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 22, 10, 5, 30, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);

    h.crank_single_clock_wait().unwrap();
    // Fragment with HEADER and DISCONT due to caps change
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // Reduced chunk due to GOP end inbetween
    test_caps_changed_verify(&mut h, 1 + 5, false, true);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Everything left until EOS
    test_caps_changed_verify(&mut h, 1 + 2, false, true);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
#[ignore]
fn test_caps_change_within_gop_no_key() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 22, 10, 5, 100, false, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);

    h.crank_single_clock_wait().unwrap();
    // Reduced GOP with HEADER and DISCONT due to caps change
    test_caps_changed_verify(&mut h, 1 + 1 + 5, true, false);
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    h.crank_single_clock_wait().unwrap();
    h.push_event(gst::event::Eos::new());
    // Everything left until EOS
    test_caps_changed_verify(&mut h, 1 + 2, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
fn test_caps_change_before_first_frame() {
    init();

    let mut h = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));

    h.element()
        .unwrap()
        .set_property_from_str("header-update-mode", "caps");

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::from_slice([1, 2, 3, 4]))
        .build();

    h.element()
        .unwrap()
        .set_property("fragment-duration", 1.seconds());

    h.set_src_caps(caps);
    h.play();

    test_caps_changed_buffers(&mut h, 22, 10, 0, 100, true, false);

    h.crank_single_clock_wait().unwrap();
    // Initial fragment with HEADER and DISCONT
    test_caps_changed_verify(&mut h, 1 + 1 + 10, true, false);

    h.crank_single_clock_wait().unwrap();
    // 2nd fragment with HEADER
    test_caps_changed_verify(&mut h, 1 + 10, false, false);

    assert_eq!(h.buffers_in_queue(), 0);
}

#[test]
fn test_cmaf_manual_split() {
    init();

    let caps = gst::Caps::builder("video/x-h264")
        .field("width", 1920i32)
        .field("height", 1080i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .field("stream-format", "avc")
        .field("alignment", "au")
        .field("codec_data", gst::Buffer::with_size(1).unwrap())
        .build();

    let mut h = gst_check::Harness::new("cmafmux");

    // 5s fragment duration
    // manual-split mode
    h.element()
        .unwrap()
        .set_properties(&[("fragment-duration", &5.seconds()), ("manual-split", &true)]);

    h.set_src_caps(caps);
    h.play();

    // Push 7 buffers of 1s each, 1st and 6 buffer without DELTA_UNIT flag
    for i in 0..7 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 0 && i != 5 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            } else if i == 5 {
                h.push_event(
                    gst::event::CustomDownstream::builder(
                        gst::Structure::builder("FMP4MuxSplitNow").build(),
                    )
                    .build(),
                );
            }
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the first fragment
    h.crank_single_clock_wait().unwrap();

    let header = h.pull().unwrap();
    assert_eq!(
        header.flags(),
        gst::BufferFlags::HEADER | gst::BufferFlags::DISCONT
    );
    assert_eq!(header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(header.dts(), Some(gst::ClockTime::ZERO));

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(fragment_header.dts(), Some(gst::ClockTime::ZERO));
    assert_eq!(fragment_header.duration(), Some(5.seconds()));

    for i in 0..5 {
        let buffer = h.pull().unwrap();
        if i == 4 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds()));
        assert_eq!(buffer.dts(), Some(i.seconds()));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    h.push_event(gst::event::Eos::new());

    let fragment_header = h.pull().unwrap();
    assert_eq!(fragment_header.flags(), gst::BufferFlags::HEADER);
    assert_eq!(fragment_header.pts(), Some(5.seconds()));
    assert_eq!(fragment_header.dts(), Some(5.seconds()));
    assert_eq!(fragment_header.duration(), Some(2.seconds()));

    for i in 5..7 {
        let buffer = h.pull().unwrap();
        if i == 6 {
            assert_eq!(
                buffer.flags(),
                gst::BufferFlags::DELTA_UNIT | gst::BufferFlags::MARKER
            );
        } else {
            assert_eq!(buffer.flags(), gst::BufferFlags::DELTA_UNIT);
        }
        assert_eq!(buffer.pts(), Some(i.seconds()));
        assert_eq!(buffer.dts(), Some(i.seconds()));
        assert_eq!(buffer.duration(), Some(gst::ClockTime::SECOND));
    }

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_multi_stream_late_key_frame() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h2.play();

    // 5 frames fit into the fragment +1 otherwise the fragment is
    // just extended, then next frame is key-frame outside fragment
    for i in 0..8 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 6 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the 1st fragment
    h1.crank_single_clock_wait().unwrap();

    // global and fragment header and 5 from from the audio stream (no
    // video) because of the missing key frame
    check_first_fragment_header(&mut h1);
    for _ in 2..7 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);

    // 5 frames fit into the fragment, +2 to add key-frame and trigger
    // check.
    for i in 8..15 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 13 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the 2nd fragment
    h1.crank_single_clock_wait().unwrap();

    // fragment header + 8 audio and 7 (GOP size) video
    check_fragment_header(&mut h1);
    for _ in 1..16 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);
}

#[test]
fn test_multi_stream_late_key_frame_skips_fragment() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h2.play();

    // 2x5 frames fit into the fragment +1 otherwise the fragment is
    // just extended, then next frame is key-frame outside fragment
    // plus one to trigger drain
    for i in 0..13 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 11 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the 1st fragment
    h1.crank_single_clock_wait().unwrap();

    // global and fragment header and 5 from from the audio stream (no
    // video) because of the missing key frame
    check_first_fragment_header(&mut h1);
    for _ in 2..7 {
        let _ = h1.pull().unwrap();
    }
    // fragment header and 5 from from the audio stream (no video)
    // because of the missing key frame
    check_fragment_header(&mut h1);
    for _ in 1..6 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);

    // 5 frames fit into the fragment, +1 to add key-frame and 1 for
    // trigger
    for i in 13..20 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 18 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the next
    // fragment
    h1.crank_single_clock_wait().unwrap();

    // fragment header +7 audio and 7 (GOP size) video
    check_fragment_header(&mut h1);
    for _ in 1..16 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);
}

#[test]
fn test_multi_stream_late_key_frame_skips_two_fragments() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 5.seconds());

    h1.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h2.play();

    for i in 0..18 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 16 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the 1st fragment
    h1.crank_single_clock_wait().unwrap();

    // global and fragment header and 5 from from the audio stream (no
    // video) because of the missing key frame
    check_first_fragment_header(&mut h1);
    for _ in 2..7 {
        let _ = h1.pull().unwrap();
    }
    // fragment header and 5 from from the audio stream (no video)
    // because of the missing key frame
    check_fragment_header(&mut h1);
    for _ in 1..6 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 6);

    // fragment header and 5 from from the audio stream (no video)
    // because of the missing key frame
    check_fragment_header(&mut h1);
    for _ in 1..6 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);

    // 5 frames fit into the fragment, +1 to add key-frame and 1 for
    // trigger
    for i in 18..25 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i.seconds());
            buffer.set_dts(i.seconds());
            buffer.set_duration(gst::ClockTime::SECOND);
            if i != 23 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock: this should bring us to the end of the next
    // fragment
    h1.crank_single_clock_wait().unwrap();

    // fragment header +7 audio and 7 (GOP size) video
    check_fragment_header(&mut h1);
    for _ in 1..16 {
        let _ = h1.pull().unwrap();
    }
    assert_eq!(h1.buffers_in_queue(), 0);
}

#[test]
fn test_multi_stream_late_2nd_stream() {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);

    // 5s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", 2.seconds());

    h1.set_src_caps(
        gst::Caps::builder("audio/mpeg")
            .field("mpegversion", 4i32)
            .field("channels", 1i32)
            .field("rate", 44100i32)
            .field("stream-format", "raw")
            .field("base-profile", "lc")
            .field("profile", "lc")
            .field("level", "2")
            .field(
                "codec_data",
                gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
            )
            .build(),
    );
    h1.play();

    h2.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h2.play();

    for i in 0..12 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts((i * 500).mseconds());
            buffer.set_dts((i * 500).mseconds());
            buffer.set_duration(500.mseconds());
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));

        // Only audio for 1st fragment
        if i < 4 {
            // Fire some GAP events to keep GstAggregator running
            if i == 0 || i == 2 {
                let ev = gst::event::Gap::builder((i * 500).mseconds())
                    .duration(1.seconds())
                    .build();
                assert!(h2.push_event(ev));
            }
            continue;
        }

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts((i * 500).mseconds());
            buffer.set_dts((i * 500).mseconds());
            buffer.set_duration(500.mseconds());
            if i != 4 && i != 8 {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }
        assert_eq!(h2.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // 1st buffer with 4 x audio buffers only and 2 headers
    h1.crank_single_clock_wait().unwrap();
    check_first_fragment_header(&mut h1);
    for _ in 2..6 {
        let _ = h1.pull().unwrap();
    }

    // 2nd fragment contains 4 x audio and 4 x video buffers + header
    h1.crank_single_clock_wait().unwrap();
    check_fragment_header(&mut h1);
    for _ in 1..9 {
        let _ = h1.pull().unwrap();
    }

    // Get the remaining 4 x audio plus 4 x video with header
    h1.push_event(gst::event::Eos::new());
    h2.push_event(gst::event::Eos::new());
    check_fragment_header(&mut h1);
    for _ in 1..9 {
        let _ = h1.pull().unwrap();
    }

    assert_eq!(h1.buffers_in_queue(), 0);
}

fn test_late_key_frame_sparse(offset: u64, multi_stream: bool, gap_buffer: bool) {
    init();

    let mut h1 = gst_check::Harness::with_padnames("isofmp4mux", Some("sink_0"), Some("src"));
    let mut h2 = if multi_stream {
        let h = gst_check::Harness::with_element(&h1.element().unwrap(), Some("sink_1"), None);
        Some(h)
    } else {
        None
    };

    let frag_duration = 2000;
    let buffer_duration = 500;

    // 2s fragment duration
    h1.element()
        .unwrap()
        .set_property("fragment-duration", frag_duration.mseconds());

    h1.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    h1.play();

    if let Some(ref mut h) = h2 {
        h.set_src_caps(
            gst::Caps::builder("audio/mpeg")
                .field("mpegversion", 4i32)
                .field("channels", 1i32)
                .field("rate", 44100i32)
                .field("stream-format", "raw")
                .field("base-profile", "lc")
                .field("profile", "lc")
                .field("level", "2")
                .field(
                    "codec_data",
                    gst::Buffer::from_slice([0x12, 0x08, 0x56, 0xe5, 0x00]),
                )
                .build(),
        );
        h.play();
    }

    let mut pts = 0;

    // calculate fragment buffers from headers + video buffers + audio
    // buffers
    let buffers_per_frag = frag_duration / buffer_duration;
    let skip = offset / buffer_duration - 1;
    let mut skipped = skip;

    // first fragment with 2 headers, subsequent fragments have only one
    let (frag_1_size, frag_2_size, frag_3_size) =
        match (offset < frag_duration, multi_stream, gap_buffer) {
            // First fragment standard, both video and audio filled
            // and two headers, next fragment with one header and as
            // many audio buffers as video buffers are skipped, 3rd
            // fragment standard with audio and video again
            (true, true, true) => (
                2 + 2 * buffers_per_frag,
                1 + skip + 1,
                Some(1 + 2 * buffers_per_frag),
            ),
            (false, false, true) => (2 + buffers_per_frag, 1 + buffers_per_frag, None),
            // Here -1 because the fragment is too large and then
            // fmp4mux does not wait for the audio buffer when the
            // video buffer already arrived to close the first GOP
            (false, true, false) => (2 + 3 * buffers_per_frag, 1 + 2 * buffers_per_frag, None),
            // The fragment is extended until the GOP is closed, means
            // 1.5 x GOP (6) audio buffers and 4 (GOP) video buffers
            // plus 2 headers for first fragment, then a full fragment
            // afterwards plus header.
            (true, true, false) => (2 + 2 * buffers_per_frag + 2, 1 + 2 * buffers_per_frag, None),
            (false, true, true) => (
                2 + 2 * buffers_per_frag,
                1 + buffers_per_frag,
                Some(1 + 2 * buffers_per_frag),
            ),
            (_, _, _) => (2 + buffers_per_frag, 1 + buffers_per_frag, None),
        };

    let n_bufs = 3 * buffers_per_frag + 3;
    // Make sure that the test is not blocking on buffer push() while
    // waiting for output to be pulled but this is not possible
    // because the clock is waiting until advanced, this situation may
    // happen in GstAggregator
    let clk = h1.testclock().unwrap();

    // 0 is idle, 1 is crank, 2 is quit
    let crank_trigger = std::sync::Arc::new((std::sync::Mutex::new(0), std::sync::Condvar::new()));
    let crank_trigger2 = std::sync::Arc::clone(&crank_trigger);
    let fwd_clock = thread::spawn(move || {
        let (lock, cond) = &*crank_trigger;
        loop {
            let mut state = lock.lock().unwrap();
            if *state == 0 {
                state = cond.wait(state).unwrap();
            }
            match *state {
                1 => {
                    *state = 0;
                    drop(state);
                    if !gap_buffer {
                        clk.wait_for_next_pending_id();
                        clk.crank();
                    }
                }
                2 => break,
                _ => unreachable!(),
            }
        }
    });

    for i in 0..n_bufs {
        {
            let (lock, cond) = &*crank_trigger2;
            let mut state = lock.lock().unwrap();
            *state = 1;
            cond.notify_one();
        }

        if let Some(ref mut h) = h2 {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_duration(buffer_duration.mseconds());
                buffer.set_pts((buffer_duration * i).mseconds());
            }
            assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
        }

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_duration(buffer_duration.mseconds());

            match i - (skip - skipped) {
                0 | 5 | 9 | 13 => buffer.set_pts(pts.mseconds()),
                1..=3 | 6..=8 | 10..=12 | 14..=16 => {
                    buffer.set_pts(pts.mseconds());
                    buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
                }
                4 => {
                    if gap_buffer {
                        let ev = gst::event::Gap::builder(pts.mseconds())
                            .duration(buffer_duration.mseconds())
                            .build();
                        assert!(h1.push_event(ev));
                    }
                    pts += buffer_duration;
                    skipped = skipped.saturating_sub(1);
                    continue;
                }
                _ => unreachable!(),
            }

            buffer.set_dts(buffer.pts());
        }

        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));
        pts += buffer_duration;
    }

    {
        let (lock, cond) = &*crank_trigger2;
        let mut state = lock.lock().unwrap();
        *state = 2;
        cond.notify_one();
    }
    fwd_clock.join().unwrap();

    h1.crank_single_clock_wait().unwrap();
    check_first_fragment_header(&mut h1);
    for _ in 2..frag_1_size {
        let _ = h1.pull().unwrap();
    }

    h1.crank_single_clock_wait().unwrap();
    check_fragment_header(&mut h1);
    for _ in 1..frag_2_size {
        let _ = h1.pull().unwrap();
    }

    if let Some(frag_size) = frag_3_size {
        h1.crank_single_clock_wait().unwrap();
        check_fragment_header(&mut h1);
        for _ in 1..frag_size {
            let _ = h1.pull().unwrap();
        }
    }

    h1.push_event(gst::event::Eos::new());
    if let Some(mut h) = h2 {
        h.push_event(gst::event::Eos::new());
    }
}

#[test]
fn test_single_stream_late_key_frame_sparse() {
    test_late_key_frame_sparse(1_000, false, false)
}

#[test]
fn test_single_stream_late_key_frame_sparse_gap() {
    test_late_key_frame_sparse(1_000, false, true)
}

#[test]
fn test_multi_stream_late_key_frame_sparse() {
    test_late_key_frame_sparse(1_000, true, false)
}

#[test]
fn test_multi_stream_late_key_frame_sparse_gap() {
    test_late_key_frame_sparse(1_000, true, true)
}

#[test]
fn test_single_stream_late_key_frame_sparse_on_frag_boundary() {
    test_late_key_frame_sparse(2_000, false, false)
}

#[test]
fn test_single_stream_late_key_frame_sparse_on_frag_boundary_gap() {
    test_late_key_frame_sparse(2_000, false, true)
}

#[test]
fn test_multi_stream_late_key_frame_sparse_on_frag_boundary() {
    test_late_key_frame_sparse(2_000, true, false)
}

#[test]
fn test_multi_stream_late_key_frame_sparse_on_frag_boundary_gap() {
    test_late_key_frame_sparse(2_000, true, true)
}

fn check_mvex_sanity(maybe_mvex: &Option<mp4_atom::Mvex>) {
    assert!(maybe_mvex.as_ref().is_some_and(|mvex| {
        assert!(mvex.mehd.is_none());
        assert_eq!(mvex.trex.len(), 1);
        let trex0 = &mvex.trex[0];
        assert_eq!(trex0.track_id, 1);
        assert_eq!(trex0.default_sample_description_index, 1);
        assert_eq!(trex0.default_sample_duration, 0);
        assert_eq!(trex0.default_sample_flags, 0);
        assert_eq!(trex0.default_sample_size, 0);
        true
    }));
    // TODO: see if there is anything generic about the stco entries we could check
}

fn check_frag_file_structure(
    location: &Path,
    expected_major_brand: mp4_atom::FourCC,
    expected_minor_version: u32,
    expected_compatible_brands: Vec<mp4_atom::FourCC>,
    expected_config: ExpectedConfiguration,
) {
    let mut required_top_level_boxes: Vec<mp4_atom::FourCC> = vec![
        b"ftyp".into(),
        b"moov".into(),
        b"styp".into(),
        b"moof".into(),
        b"mdat".into(),
    ];

    let mut input = File::open(location).unwrap();
    while let Ok(header) = mp4_atom::Header::read_from(&mut input) {
        println!("header.kind: {:?}", &header.kind);
        assert!(required_top_level_boxes.contains(&header.kind));
        let pos = required_top_level_boxes
            .iter()
            .position(|&fourcc| fourcc == header.kind)
            .unwrap_or_else(|| panic!("expected to find a matching fourcc {:?}", header.kind));
        required_top_level_boxes.remove(pos);
        match header.kind {
            mp4_atom::Ftyp::KIND => {
                let ftyp = mp4_atom::Ftyp::read_atom(&header, &mut input).unwrap();
                check_ftyp_output(
                    expected_major_brand,
                    expected_minor_version,
                    &expected_compatible_brands,
                    ftyp,
                );
            }
            mp4_atom::Moov::KIND => {
                let moov = mp4_atom::Moov::read_atom(&header, &mut input).unwrap();
                assert!(moov.meta.is_none());
                assert!(moov.udta.is_none());
                check_mvex_sanity(&moov.mvex);
                check_trak_sanity(&moov.trak, &expected_config);
                check_mvhd_sanity(&moov.mvhd, &expected_config);
            }
            mp4_atom::Styp::KIND => {
                let styp = mp4_atom::Styp::read_atom(&header, &mut input).unwrap();
                assert_eq!(styp.major_brand, expected_major_brand);
                // TODO: check the rest
            }
            mp4_atom::Moof::KIND => {
                let moof = mp4_atom::Moof::read_atom(&header, &mut input).unwrap();
                assert_eq!(moof.mfhd.sequence_number, 1);
                assert_eq!(moof.traf.len(), 1);
                let traf0 = &moof.traf[0];
                assert!(traf0
                    .tfdt
                    .as_ref()
                    .is_some_and(|tfdt| { tfdt.base_media_decode_time == 0 }));
                assert_eq!(traf0.tfhd.track_id, 1);
                assert!(traf0.tfhd.base_data_offset.is_none());
                if expected_config.is_audio {
                    // TODO: work out why this isn't always Some()
                    // assert!(traf0.tfhd.default_sample_duration.is_some());
                } else {
                    assert_eq!(traf0.tfhd.default_sample_duration, Some(100));
                }
                assert!(traf0.tfhd.default_sample_size.is_none());
                assert!(traf0.tfhd.sample_description_index.is_none());
                if expected_config.is_audio {
                    assert_eq!(traf0.tfhd.default_sample_flags, Some(0x02800000));
                } else {
                    assert_eq!(traf0.tfhd.default_sample_flags, Some(0x01010000));
                }
                assert_eq!(traf0.trun.len(), 1);
                let trun = traf0.trun.first().unwrap();
                if expected_config.is_audio {
                    // This offset and count is a bit arbitrary, since its codec specific
                    assert!(trun.data_offset.is_some_and(|offset| offset >= 112));
                    assert!(trun.entries.len() >= 3);
                } else {
                    assert_eq!(trun.data_offset, Some(184));
                    assert_eq!(trun.entries.len(), 10);
                }
                // TODO: this could check the entries.
            }
            mp4_atom::Mdat::KIND => {
                let mdat = mp4_atom::Mdat::read_atom(&header, &mut input).unwrap();
                assert!(!mdat.data.is_empty());
            }
            _ => {
                panic!("Unexpected top level box: {:?}", header.kind);
            }
        }
    }
    assert!(
        required_top_level_boxes.is_empty(),
        "expected all top level boxes to be found, but these were missed: {required_top_level_boxes:?}"
    );
}

#[test]
fn test_fmux_boxes() {
    init();

    let video_enc = "x264enc";
    let filename = format!("frag_{video_enc}.mp4").to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "videotestsrc num-buffers=10 ! {video_enc} ! isofmp4mux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        panic!("could not build encoding pipeline")
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");

    check_frag_file_structure(
        location,
        b"iso6".into(),
        0,
        vec![b"avc1".into(), b"iso6".into()],
        ExpectedConfiguration {
            has_stss: true,
            is_fragmented: true,
            ..Default::default()
        },
    );
}

#[test]
fn test_cmaf_fmux_boxes() {
    init();

    let video_enc = "x264enc";
    let filename = format!("cmaf_{video_enc}.mp4").to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "videotestsrc num-buffers=10 ! {video_enc} ! cmafmux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        panic!("could not build encoding pipeline")
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");

    check_frag_file_structure(
        location,
        b"cmf2".into(),
        0,
        vec![
            b"avc1".into(),
            b"cmf2".into(),
            b"iso6".into(),
            b"cmfc".into(),
        ],
        ExpectedConfiguration {
            has_stss: true,
            has_taic: false,
            is_fragmented: true,
            ..Default::default()
        },
    );
}

#[test]
fn test_dash_fmux_boxes() {
    init();

    let video_enc = "x264enc";
    let filename = format!("dash_{video_enc}.mp4").to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "videotestsrc num-buffers=10 ! {video_enc} ! dashmp4mux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        panic!("could not build encoding pipeline")
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");

    check_frag_file_structure(
        location,
        b"msdh".into(),
        0,
        vec![
            b"avc1".into(),
            b"iso6".into(),
            b"dums".into(),
            b"msdh".into(),
        ],
        ExpectedConfiguration {
            has_stss: true,
            is_fragmented: true,
            ..Default::default()
        },
    );
}

#[test]
fn test_flac_fmux_boxes() {
    init();

    let filename = "flac_fmp4.mp4".to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "audiotestsrc num-buffers=10 ! flacenc ! flacparse ! isofmp4mux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        println!("could not build encoding pipeline");
        return;
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    let s = location.to_str().unwrap();
    println!("s = {s:?}");
    check_frag_file_structure(
        location,
        b"iso6".into(),
        0,
        vec![b"iso6".into()],
        ExpectedConfiguration {
            is_audio: true,
            is_fragmented: true,
            audio_channel_count: 1,
            audio_sample_rate: 44100.into(),
            audio_sample_size: 8,
            ..Default::default()
        },
    );
}

#[test]
fn test_ac3_fmux_boxes() {
    init();

    let filename = "ac3_fmp4.mp4".to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "audiotestsrc num-buffers=10 ! audio/x-raw,channels=2 ! avenc_ac3 bitrate=192000 ! isofmp4mux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        println!("could not build encoding pipeline");
        return;
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    let s = location.to_str().unwrap();
    println!("s = {s:?}");
    check_frag_file_structure(
        location,
        b"iso6".into(),
        0,
        vec![b"dby1".into(), b"iso6".into()],
        ExpectedConfiguration {
            is_audio: true,
            is_fragmented: true,
            audio_channel_count: 2,
            audio_sample_rate: 44100.into(),
            audio_sample_size: 16,
            ..Default::default()
        },
    );
}

#[test]
fn test_eac3_fmux_boxes() {
    init();

    let filename = "eac3_fmp4.mp4".to_string();
    let temp_dir = tempdir().unwrap();
    let temp_file_path = temp_dir.path().join(filename);
    let location = temp_file_path.as_path();
    let pipeline_text = format!(
        "audiotestsrc num-buffers=10 ! audio/x-raw,channels=2 ! avenc_eac3 bitrate=192000 ! isofmp4mux ! filesink location={location:?}"
    );

    let Ok(pipeline) = gst::parse::launch(&pipeline_text) else {
        println!("could not build encoding pipeline");
        return;
    };
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");
    for msg in pipeline.bus().unwrap().iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                panic!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    let s = location.to_str().unwrap();
    println!("s = {s:?}");
    check_frag_file_structure(
        location,
        b"iso6".into(),
        0,
        vec![b"dby1".into(), b"iso6".into()],
        ExpectedConfiguration {
            is_audio: true,
            is_fragmented: true,
            audio_channel_count: 2,
            audio_sample_rate: 44100.into(),
            audio_sample_size: 16,
            ..Default::default()
        },
    );
}
