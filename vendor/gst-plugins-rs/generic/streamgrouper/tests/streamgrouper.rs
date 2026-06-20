// Copyright (C) 2024 Igalia S.L. <aboya@igalia.com>
// Copyright (C) 2024 Comcast <aboya@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
use gst::{prelude::*, Element, GroupId};

use gst_check::Harness;
use std::sync::LazyLock;

#[allow(unused)]
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "streamgrouper-test",
        gst::DebugColorFlags::empty(),
        Some("streamgrouper test"),
    )
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gststreamgrouper::plugin_register_static().expect("gststreamgrouper streamgrouper test");
    });
}

#[test]
fn test_request_invalid_pad_name() {
    init();
    let sg = gst::ElementFactory::make("streamgrouper")
        .build()
        .expect("streamgrouper factory should exist");
    assert!(sg.request_pad_simple("invalid_name").is_none());
}

#[test]
fn test_can_change_state() {
    init();
    let sg = gst::ElementFactory::make("streamgrouper")
        .build()
        .expect("streamgrouper factory should exist");
    if let Err(error) = sg.set_state(gst::State::Playing) {
        panic!("Failed to change to PLAYING: {error:?}");
    }
    if let Err(error) = sg.set_state(gst::State::Null) {
        panic!("Failed to change to NULL: {error:?}");
    }
}

fn make_with_double_harness() -> (Element, Harness, Harness) {
    init();
    let sg = gst::ElementFactory::make("streamgrouper")
        .build()
        .expect("streamgrouper factory should exist");
    // gst_harness_add_element_full() sets the element to PLAYING, but not before sending
    // a stream-start, which means it ends up sending buffers while in NULL state.
    // streamgrouper can handle that, but let's rather test what applications should be
    // doing instead and set the element to a higher state first.
    if let Err(error) = sg.set_state(gst::State::Playing) {
        panic!("Failed to change to PLAYING: {error:?}");
    }
    let mut h1 = gst_check::Harness::with_element(&sg, Some("sink_1"), Some("src_1"));
    let mut h2 = gst_check::Harness::with_element(&sg, Some("sink_2"), Some("src_2"));
    // Consume the stream-start that harness sends internally in
    // gst_harness_add_element_full(). For some reason this is not done automatically (!?)
    while h1.try_pull_event().is_some() {}
    while h2.try_pull_event().is_some() {}
    (sg, h1, h2)
}

#[test]
fn test_push_stream_start() {
    let (_, mut h1, mut h2) = make_with_double_harness();
    let input_group_id1 = GroupId::next();
    let input_group_id2 = GroupId::next();
    h1.push_event(
        gst::event::StreamStart::builder("stream1")
            .group_id(input_group_id1)
            .build(),
    );
    h2.push_event(
        gst::event::StreamStart::builder("stream2")
            .group_id(input_group_id2)
            .build(),
    );
    let e1 = h1
        .pull_event()
        .expect("an event should have been pushed at the other end");
    let e2 = h2
        .pull_event()
        .expect("an event should have been pushed at the other end");
    assert_eq!(e1.type_(), gst::EventType::StreamStart);
    assert_eq!(e2.type_(), gst::EventType::StreamStart);
    let output_group_id1 = match e1.view() {
        gst::EventView::StreamStart(ev) => ev.group_id().expect("There must be a group id"),
        _ => panic!("unexpected event: {e1:?}"),
    };
    let output_group_id2 = match e2.view() {
        gst::EventView::StreamStart(ev) => ev.group_id().expect("There must be a group id"),
        _ => panic!("unexpected event: {e2:?}"),
    };
    assert_eq!(output_group_id1, output_group_id2);
    assert_ne!(output_group_id1, input_group_id1);
    assert_ne!(output_group_id1, input_group_id2);
}

#[test]
fn test_push_buffer() {
    let (_, mut h1, _) = make_with_double_harness();
    let segment = gst::event::Segment::new(&gst::FormattedSegment::<gst::ClockTime>::new());
    h1.push_event(segment);
    let segment_other_side = h1.pull_event().unwrap();
    assert_eq!(gst::EventType::Segment, segment_other_side.type_());
    let buffer = gst::Buffer::new();
    h1.push(buffer.clone()).unwrap();
    let buffer_other_side = h1.pull().unwrap();
    assert_eq!(
        buffer.as_ptr(),
        buffer_other_side.as_ptr(),
        "buffer should be unmodified"
    );
}

#[test]
fn test_upstream_seek() {
    let (_, mut h1, _) = make_with_double_harness();
    let seek = gst::event::Seek::new(
        1.0,
        gst::SeekFlags::FLUSH,
        gst::SeekType::Set,
        3.seconds(),
        gst::SeekType::None,
        0.seconds(),
    );
    h1.push_upstream_event(seek);
    let mut received_seek = false;
    // A reconfigure event is generated, so we'll loop to skip that.
    loop {
        let ev = match h1.try_pull_upstream_event() {
            None => break,
            Some(ev) => ev,
        };
        if let gst::EventView::Seek(seek) = ev.view() {
            let start = seek.get().3;
            let clock_time = match start {
                gst::GenericFormattedValue::Time(clock_time) => clock_time,
                _ => panic!("Invalid start: {start:?}"),
            };
            assert_eq!(Some(3.seconds()), clock_time);
            received_seek = true;
            break;
        }
    }
    assert!(received_seek);
}

#[test]
fn test_query() {
    let (_, mut h1, _) = make_with_double_harness();
    let expected_latency = 1.seconds();
    h1.set_upstream_latency(expected_latency);
    let actual_latency = h1.query_latency();
    assert_eq!(Some(expected_latency), actual_latency);
}
