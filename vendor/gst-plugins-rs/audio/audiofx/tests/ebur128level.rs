// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(clippy::single_match)]

use gst::glib;
use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsaudiofx::plugin_register_static().expect("Failed to register rsaudiofx plugin");
    });
}

#[test]
fn test_ebur128level_s16_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::Interleaved,
        gst_audio::AUDIO_FORMAT_S16,
    );
}

#[test]
fn test_ebur128level_s32_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::Interleaved,
        gst_audio::AUDIO_FORMAT_S32,
    );
}

#[test]
fn test_ebur128level_f32_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::Interleaved,
        gst_audio::AUDIO_FORMAT_F32,
    );
}

#[test]
fn test_ebur128level_f64_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::Interleaved,
        gst_audio::AUDIO_FORMAT_F64,
    );
}

#[test]
fn test_ebur128level_s16_non_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::NonInterleaved,
        gst_audio::AUDIO_FORMAT_S16,
    );
}

#[test]
fn test_ebur128level_s32_non_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::NonInterleaved,
        gst_audio::AUDIO_FORMAT_S32,
    );
}

#[test]
fn test_ebur128level_f32_non_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::NonInterleaved,
        gst_audio::AUDIO_FORMAT_F32,
    );
}

#[test]
fn test_ebur128level_f64_non_interleaved() {
    init();
    run_test(
        gst_audio::AudioLayout::NonInterleaved,
        gst_audio::AUDIO_FORMAT_F64,
    );
}

fn run_test(layout: gst_audio::AudioLayout, format: gst_audio::AudioFormat) {
    let mut h = gst_check::Harness::new_parse(&format!(
        "audiotestsrc num-buffers=5 samplesperbuffer=48000 ! \
         audioconvert ! \
         audio/x-raw,layout={},format={},channels=2,rate=48000 ! \
         ebur128level interval=500000000",
        match layout {
            gst_audio::AudioLayout::Interleaved => "interleaved",
            gst_audio::AudioLayout::NonInterleaved => "non-interleaved",
            _ => unimplemented!(),
        },
        format.to_str()
    ));
    let bus = gst::Bus::new();
    h.element().unwrap().set_bus(Some(&bus));
    h.play();

    // Pull all buffers until EOS
    let mut num_buffers = 0;
    while let Some(_buffer) = h.pull_until_eos().unwrap() {
        num_buffers += 1;
    }
    assert_eq!(num_buffers, 5);

    let mut num_msgs = 0;
    while let Some(msg) = bus.pop() {
        match msg.view() {
            gst::MessageView::Element(msg) => {
                let s = msg.structure().unwrap();
                if s.name() == "ebur128-level" {
                    num_msgs += 1;
                    let timestamp = s.get::<u64>("timestamp").unwrap();
                    let running_time = s.get::<u64>("running-time").unwrap();
                    let stream_time = s.get::<u64>("stream-time").unwrap();
                    assert_eq!(timestamp, num_msgs * 500 * *gst::ClockTime::MSECOND);
                    assert_eq!(running_time, num_msgs * 500 * *gst::ClockTime::MSECOND);
                    assert_eq!(stream_time, num_msgs * 500 * *gst::ClockTime::MSECOND);

                    // Check if all these exist
                    let _momentary_loudness = s.get::<f64>("momentary-loudness").unwrap();
                    let _shortterm_loudness = s.get::<f64>("shortterm-loudness").unwrap();
                    let _global_loudness = s.get::<f64>("global-loudness").unwrap();
                    let _relative_threshold = s.get::<f64>("relative-threshold").unwrap();
                    let _loudness_range = s.get::<f64>("loudness-range").unwrap();
                    let sample_peak = s.get::<gst::ArrayRef>("sample-peak").unwrap();
                    assert_eq!(sample_peak.as_slice().len(), 2);
                    assert_eq!(sample_peak.as_slice()[0].type_(), glib::Type::F64);
                    let true_peak = s.get::<gst::ArrayRef>("true-peak").unwrap();
                    assert_eq!(true_peak.as_slice().len(), 2);
                    assert_eq!(true_peak.as_slice()[0].type_(), glib::Type::F64);
                }
            }
            _ => (),
        }
    }

    assert_eq!(num_msgs, 10);
}
