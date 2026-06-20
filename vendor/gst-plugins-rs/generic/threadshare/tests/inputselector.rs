// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
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
        gstthreadshare::plugin_register_static().expect("gstthreadshare inputselector test");
    });
}

#[test]
fn test_active_pad() {
    init();

    let is = gst::ElementFactory::make("ts-input-selector")
        .build()
        .unwrap();

    let mut h1 = gst_check::Harness::with_element(&is, Some("sink_%u"), Some("src"));
    let mut h2 = gst_check::Harness::with_element(&is, Some("sink_%u"), None);

    let active_pad = is.property::<Option<gst::Pad>>("active-pad");
    assert_eq!(active_pad, h1.srcpad().unwrap().peer());

    is.set_property("active-pad", h2.srcpad().unwrap().peer());
    let active_pad = is.property::<Option<gst::Pad>>("active-pad");
    assert_eq!(active_pad, h2.srcpad().unwrap().peer());

    h1.set_src_caps_str("foo/bar");
    h2.set_src_caps_str("foo/bar");

    h1.play();

    /* Push buffer on inactive pad, we should not receive anything */
    let buf = gst::Buffer::new();
    assert_eq!(h1.push(buf), Ok(gst::FlowSuccess::Ok));
    assert_eq!(h1.buffers_received(), 0);

    /* Buffers pushed on the active pad should be received */
    let buf = gst::Buffer::new();
    assert_eq!(h2.push(buf), Ok(gst::FlowSuccess::Ok));
    assert_eq!(h1.buffers_received(), 1);

    assert_eq!(h1.events_received(), 3);

    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::StreamStart);
    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Caps);
    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Segment);

    /* Push another buffer on the active pad, there should be no new events */
    let buf = gst::Buffer::new();
    assert_eq!(h2.push(buf), Ok(gst::FlowSuccess::Ok));
    assert_eq!(h1.buffers_received(), 2);
    assert_eq!(h1.events_received(), 3);

    /* Switch the active pad and push a buffer, we should receive stream-start, segment and caps
     * again */
    let buf = gst::Buffer::new();
    is.set_property("active-pad", h1.srcpad().unwrap().peer());
    assert_eq!(h1.push(buf), Ok(gst::FlowSuccess::Ok));
    assert_eq!(h1.buffers_received(), 3);
    assert_eq!(h1.events_received(), 6);
    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::StreamStart);
    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Caps);
    let event = h1.pull_event().unwrap();
    assert_eq!(event.type_(), gst::EventType::Segment);

    let _ = is.set_state(gst::State::Null);
}
