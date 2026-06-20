// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
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

#[test]
fn test_parse() {
    init();
    let data = include_bytes!("dn2018-1217.scc").as_ref();

    let mut h = gst_check::Harness::new_parse("sccparse ! cea608tott");
    h.set_src_caps_str("application/x-scc");
    h.set_sink_caps_str("text/x-raw");

    let buf = gst::Buffer::from_mut_slice(Vec::from(data));
    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));

    // Check the first 10 output buffers
    let expected: [(ClockTime, ClockTime, &'static str); 10] = [
        (
            15_048_366_666.nseconds(),
            3_236_566_667.nseconds(),
            "From New York,\r\nthis is Democracy Now!",
        ),
        (
            18_985_633_333.nseconds(),
            1_234_566_667.nseconds(),
            "Yes, I’m supporting\r\nDonald Trump.",
        ),
        (
            20_220_200_000.nseconds(),
            2_168_833_333.nseconds(),
            "I’m doing so as enthusiastically\r\nas I can,",
        ),
        (
            22_389_033_333.nseconds(),
            2_235_566_667.nseconds(),
            "even the fact I think\r\nhe’s a terrible human being.",
        ),
        (
            24_624_600_000.nseconds(),
            2_102_100_000.nseconds(),
            "But the choice on the other side\r\nis just as bad.",
        ),
        (
            26_726_700_000.nseconds(),
            2_268_933_333.nseconds(),
            "Trump is a\r\n\"terrible human being.\"",
        ),
        (
            29_696_333_333.nseconds(),
            2_202_200_000.nseconds(),
            "Those are the words\r\nof Mick Mulvaney,",
        ),
        (
            32_432_400_000.nseconds(),
            3_236_566_666.nseconds(),
            "the man Trump has chosen\r\nto be his new chief of staff.",
        ),
        (
            36_302_933_333.nseconds(),
            2_635_966_667.nseconds(),
            "We’ll speak with former\r\npresidential candidate",
        ),
        (
            38_938_900_000.nseconds(),
            2_369_033_333.nseconds(),
            "Ralph Nader\r\nabout Mulvaney,",
        ),
    ];

    for (i, e) in expected.iter().enumerate() {
        let buf = h.try_pull().unwrap();

        assert_eq!(
            e.0,
            buf.pts().unwrap(),
            "Unexpected PTS for {}th buffer",
            i + 1
        );
        assert_eq!(
            e.1,
            buf.duration().unwrap(),
            "Unexpected duration for {}th buffer",
            i + 1
        );

        let data = buf.map_readable().unwrap();
        let s = std::str::from_utf8(&data)
            .unwrap_or_else(|_| panic!("Non-UTF8 data for {}th buffer", i + 1));
        assert_eq!(e.2, s, "Unexpected data for {}th buffer", i + 1);
    }

    let caps = h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");
    assert_eq!(
        caps,
        gst::Caps::builder("text/x-raw")
            .field("format", "utf8")
            .build()
    );
}
