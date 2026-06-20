// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(clippy::single_match)]

use gst::prelude::*;
use gst::EventView;
use pretty_assertions::assert_eq;
use rand::{Rng, SeedableRng};
use std::path::PathBuf;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().expect("mccparse test");
    });
}

/// Randomized test passing buffers of arbitrary sizes to the parser
#[test]
fn test_parse() {
    init();
    let mut data = include_bytes!("captions-test_708.mcc").as_ref();

    let mut rnd = if let Ok(seed) = std::env::var("MCC_PARSE_TEST_SEED") {
        rand::rngs::SmallRng::seed_from_u64(
            seed.parse::<u64>()
                .expect("MCC_PARSE_TEST_SEED has to contain a 64 bit integer seed"),
        )
    } else {
        let seed = rand::random::<u64>();
        println!("seed {seed}");
        rand::rngs::SmallRng::seed_from_u64(seed)
    };

    let mut h = gst_check::Harness::new("mccparse");

    h.set_src_caps_str("application/x-mcc, version=(int) 1");

    let mut input_len = 0;
    let mut output_len = 0;
    let mut checksum = 0u32;
    let mut expected_timecode = None;

    while !data.is_empty() {
        let l = if data.len() == 1 {
            1
        } else {
            rnd.random_range(1..=data.len())
        };
        let buf = gst::Buffer::from_mut_slice(Vec::from(&data[0..l]));
        input_len += buf.size();
        assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));
        while let Some(buf) = h.try_pull() {
            output_len += buf.size();
            checksum = checksum.wrapping_add(
                buf.map_readable()
                    .unwrap()
                    .iter()
                    .fold(0u32, |s, v| s.wrapping_add(*v as u32)),
            );

            let tc_meta = buf
                .meta::<gst_video::VideoTimeCodeMeta>()
                .expect("No timecode meta");
            if let Some(ref timecode) = expected_timecode {
                assert_eq!(&tc_meta.tc(), timecode);
            } else {
                expected_timecode = Some(tc_meta.tc());
            }
            if let Some(ref mut tc) = expected_timecode {
                tc.increment_frame();
            }
        }
        data = &data[l..];
    }

    h.push_event(gst::event::Eos::new());
    while let Some(buf) = h.try_pull() {
        output_len += buf.size();
        checksum = checksum.wrapping_add(
            buf.map_readable()
                .unwrap()
                .iter()
                .fold(0u32, |s, v| s.wrapping_add(*v as u32)),
        );

        let tc_meta = buf
            .meta::<gst_video::VideoTimeCodeMeta>()
            .expect("No timecode meta");
        if let Some(ref timecode) = expected_timecode {
            assert_eq!(&tc_meta.tc(), timecode);
        } else {
            expected_timecode = Some(tc_meta.tc());
        }
        if let Some(ref mut tc) = expected_timecode {
            tc.increment_frame();
        }
    }

    assert!(expected_timecode.is_some());
    assert_eq!(input_len, 28_818);
    assert_eq!(output_len, 42_383);
    assert_eq!(checksum, 3_988_480);

    let caps = h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");
    assert_eq!(
        caps,
        gst::Caps::builder("closedcaption/x-cea-708")
            .field("format", "cdp")
            .field("framerate", gst::Fraction::new(30000, 1001))
            .build()
    );
}

#[test]
fn test_pull() {
    init();

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/captions-test_708.mcc");

    let mut h = gst_check::Harness::new_parse(&format!("filesrc location={path:?} ! mccparse"));

    h.play();

    /* Let's first pull until EOS */
    loop {
        let mut done = false;

        while h.events_in_queue() != 0 {
            let event = h.pull_event();

            if let Ok(event) = event {
                match event.view() {
                    EventView::Eos(_) => {
                        done = true;
                        break;
                    }
                    _ => (),
                }
            }
        }

        while h.buffers_in_queue() != 0 {
            let _ = h.pull();
        }

        if done {
            break;
        }
    }

    /* Now seek and check that we receive buffers with appropriate PTS */
    h.push_upstream_event(gst::event::Seek::new(
        1.0,
        gst::SeekFlags::FLUSH,
        gst::SeekType::Set,
        gst::ClockTime::SECOND,
        gst::SeekType::Set,
        2.seconds(),
    ));

    loop {
        let mut done = false;

        while h.buffers_in_queue() != 0 {
            if let Ok(buffer) = h.pull() {
                let pts = buffer.pts().unwrap();
                assert!(pts > gst::ClockTime::SECOND && pts < 2.seconds());
            }
        }

        while h.events_in_queue() != 0 {
            let event = h.pull_event();

            if let Ok(event) = event {
                match event.view() {
                    EventView::Eos(_) => {
                        done = true;
                        break;
                    }
                    _ => (),
                }
            }
        }

        if done {
            break;
        }
    }
}
