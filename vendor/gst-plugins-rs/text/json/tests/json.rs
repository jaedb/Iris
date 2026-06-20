// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(clippy::single_match)]

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstjson::plugin_register_static().expect("json test");
    });
}

#[test]
fn test_enc() {
    init();

    let input = "{\"foo\":42}";

    let mut h = gst_check::Harness::new("jsongstenc");

    h.set_src_caps_str("application/x-json, format=test");

    let buf = {
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(input));
        let buf_ref = buf.get_mut().unwrap();
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf_ref.set_duration(2.seconds());
        buf
    };

    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));

    let buf = h.pull().expect("Couldn't pull buffer");
    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        Ok("{\"Header\":{\"format\":\"test\"}}\n"),
    );

    let buf = h.pull().expect("Couldn't pull buffer");
    assert_eq!(buf.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(buf.duration(), Some(2.seconds()));
    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        Ok("{\"Buffer\":{\"pts\":0,\"duration\":2000000000,\"data\":{\"foo\":42}}}\n")
    );
}

#[test]
fn test_parse() {
    init();

    let input = [
        "{\"Header\":{\"format\":\"test\"}}\n",
        "{\"Buffer\":{\"pts\":0,\"duration\":2000000000,\"data\":{\"foo\":42}}}\n",
    ];

    let mut h = gst_check::Harness::new("jsongstparse");

    h.set_src_caps_str("text/x-raw");

    for input in &input {
        let buf = gst::Buffer::from_mut_slice(Vec::from(&input[..]));
        assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));
    }

    while h.events_in_queue() > 0 {
        use gst::EventView;

        let ev = h.pull_event().unwrap();

        match ev.view() {
            EventView::Caps(ev) => {
                assert!(ev.caps().is_strictly_equal(
                    &gst::Caps::builder("application/x-json")
                        .field("format", "test")
                        .build()
                ));
            }
            _ => (),
        }
    }

    let buf = h.pull().expect("Couldn't pull buffer");
    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(buf.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(buf.duration(), Some(2.seconds()));
    assert_eq!(std::str::from_utf8(map.as_ref()), Ok("{\"foo\":42}"));
}
