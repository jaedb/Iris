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
        gstregex::plugin_register_static().expect("regex test");
    });
}

#[test]
fn test_replace_all() {
    init();

    let input = b"crap that mothertrapper";

    let expected_output = "trap that mothertrapper";

    let mut h = gst_check::Harness::new("regex");

    {
        let regex = h.element().expect("Could not create regex");

        let command = gst::Structure::builder("replace-all")
            .field("pattern", "crap")
            .field("replacement", "trap")
            .build();

        let commands = gst::Array::new([command]);

        regex.set_property("commands", &commands);
    }

    h.set_src_caps_str("text/x-raw, format=utf8");

    let buf = {
        let mut buf = gst::Buffer::from_mut_slice(Vec::from(&input[..]));
        let buf_ref = buf.get_mut().unwrap();
        buf_ref.set_pts(gst::ClockTime::ZERO);
        buf_ref.set_duration(2.seconds());
        buf
    };

    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));

    let buf = h.pull().expect("Couldn't pull buffer");

    assert_eq!(buf.pts(), Some(gst::ClockTime::ZERO));
    assert_eq!(buf.duration(), Some(2.seconds()));

    let map = buf.map_readable().expect("Couldn't map buffer readable");

    assert_eq!(
        std::str::from_utf8(map.as_ref()),
        std::str::from_utf8(expected_output.as_ref())
    );
}
