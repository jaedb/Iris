// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use pretty_assertions::assert_eq;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrswebp::plugin_register_static().expect("webpenc-rs test");
    });
}

#[test]
fn test_decode() {
    init();
    let data = include_bytes!("animated.webp").as_ref();
    let mut h = gst_check::Harness::new("rswebpdec");

    h.set_src_caps_str("image/webp");

    let buf = gst::Buffer::from_slice(data);
    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));
    h.push_event(gst::event::Eos::new());

    let mut expected_timestamp: Option<gst::ClockTime> = Some(gst::ClockTime::ZERO);
    let mut count = 0;
    let expected_duration: Option<gst::ClockTime> = Some(40_000_000.nseconds());

    while let Some(buf) = h.try_pull() {
        assert_eq!(buf.pts(), expected_timestamp);
        assert_eq!(buf.duration(), expected_duration);

        expected_timestamp = expected_timestamp.opt_add(expected_duration);
        count += 1;
    }

    assert_eq!(count, 10);

    let caps = h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");
    assert_eq!(
        caps,
        gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, 400, 400)
            .fps((0, 1))
            .build()
            .unwrap()
            .to_caps()
            .unwrap()
    );
}
