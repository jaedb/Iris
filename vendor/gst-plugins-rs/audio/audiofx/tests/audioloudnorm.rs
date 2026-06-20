// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use byte_slice_cast::*;

use std::sync::{Arc, Mutex};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsaudiofx::plugin_register_static().expect("Failed to register rsaudiofx plugin");
    });
}

fn run_test(
    first_input: &str,
    second_input: Option<&str>,
    num_buffers: u32,
    samples_per_buffer: u32,
    channels: u32,
    expected_loudness: f64,
) {
    init();

    let format = if cfg!(target_endian = "little") {
        format!("audio/x-raw,format=F64LE,rate=192000,channels={channels}")
    } else {
        format!("audio/x-raw,format=F64BE,rate=192000,channels={channels}")
    };

    let pipeline = if let Some(second_input) = second_input {
        gst::parse::launch(&format!(
        "audiotestsrc {first_input} num-buffers={num_buffers} samplesperbuffer={samples_per_buffer} ! {format} ! audiomixer name=mixer output-buffer-duration={output_buffer_duration} ! {format} ! audioloudnorm ! appsink name=sink  audiotestsrc {second_input} num-buffers={num_buffers} samplesperbuffer={samples_per_buffer} ! {format} ! mixer.",
        first_input = first_input,
        second_input = second_input,
        num_buffers = num_buffers,
        samples_per_buffer = samples_per_buffer,
        output_buffer_duration = samples_per_buffer as u64 * *gst::ClockTime::SECOND / 192_000,
        format = format,
        ))
    } else {
        gst::parse::launch(&format!(
        "audiotestsrc {first_input} num-buffers={num_buffers} samplesperbuffer={samples_per_buffer} ! {format} ! audioloudnorm ! appsink name=sink",
        ))
    }
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();

    sink.set_sync(false);
    let caps = gst_audio::AudioInfo::builder(gst_audio::AUDIO_FORMAT_F64, 192_000, channels)
        .build()
        .unwrap()
        .to_caps()
        .unwrap();
    sink.set_caps(Some(&caps));

    let samples = Arc::new(Mutex::new(Vec::new()));

    let samples_clone = samples.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().unwrap();

                let mut samples = samples_clone.lock().unwrap();
                samples.push(sample);
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(..) => unreachable!(),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();

    assert!(eos);
    let samples = samples.lock().unwrap();

    let mut r128 = ebur128::EbuR128::new(
        channels,
        192_000,
        ebur128::Mode::I | ebur128::Mode::SAMPLE_PEAK,
    )
    .unwrap();

    let mut num_samples = 0;
    let mut expected_ts = gst::ClockTime::ZERO;
    for sample in samples.iter() {
        use std::cmp::Ordering;

        let buf = sample.buffer().unwrap();

        let ts = buf.pts().expect("undefined pts");
        match ts.cmp(&expected_ts) {
            Ordering::Greater => {
                assert!(
                    ts - expected_ts <= gst::ClockTime::NSECOND,
                    "TS is {ts} instead of {expected_ts}"
                );
            }
            Ordering::Less => {
                assert!(
                    expected_ts - ts <= gst::ClockTime::NSECOND,
                    "TS is {ts} instead of {expected_ts}"
                );
            }
            Ordering::Equal => (),
        }

        let map = buf.map_readable().unwrap();
        let data = map.as_slice_of::<f64>().unwrap();

        num_samples += data.len() / channels as usize;
        r128.add_frames_f64(data).unwrap();

        expected_ts += (data.len() as u64 / channels as u64).seconds() / 192_000;
    }

    assert_eq!(
        num_samples,
        num_buffers as usize * samples_per_buffer as usize
    );

    let loudness = r128.loudness_global().unwrap();

    if expected_loudness.classify() == std::num::FpCategory::Infinite && expected_loudness < 0.0 {
        assert!(
            loudness.classify() == std::num::FpCategory::Infinite && loudness < 0.0,
            "Loudness is {loudness} instead of {expected_loudness}",
        );
    } else {
        assert!(
            f64::abs(loudness - expected_loudness) < 1.0,
            "Loudness is {loudness} instead of {expected_loudness}",
        );
    }

    for c in 0..channels {
        let peak = 20.0 * f64::log10(r128.sample_peak(c).unwrap());
        assert!(peak <= -2.0, "Peak {c} for channel {peak} is above -2.0",);
    }
}

#[test]
fn basic() {
    run_test("wave=sine", None, 1000, 1920, 1, -24.0);
}

#[test]
fn basic_white_noise() {
    run_test("wave=white-noise", None, 1000, 1920, 1, -24.0);
}

#[test]
fn remaining_at_eos() {
    run_test("wave=sine", None, 1000, 1024, 1, -24.0);
}

#[test]
fn short_input() {
    run_test("wave=sine", None, 100, 1024, 1, -24.0);
}

#[test]
fn basic_two_channels() {
    run_test("wave=sine", None, 1000, 1920, 2, -24.0);
}

#[test]
fn silence() {
    run_test("wave=silence", None, 1000, 1024, 1, f64::NEG_INFINITY);
}

#[test]
fn quiet() {
    // -6dB
    run_test("wave=sine volume=0.5", None, 1000, 1024, 1, -24.0);
}

#[test]
fn very_quiet() {
    // -20dB
    run_test("wave=sine volume=0.1", None, 1000, 1024, 1, -24.0);
}

#[test]
fn very_very_quiet() {
    // -40dB
    run_test("wave=sine volume=0.01", None, 1000, 1024, 1, -24.0);
}

#[test]
fn below_threshold() {
    // -70dB
    run_test(
        "wave=sine volume=0.00045",
        None,
        1000,
        1024,
        1,
        f64::NEG_INFINITY,
    );
}

#[test]
fn limiter() {
    run_test(
        "wave=sine volume=0.05",
        Some("wave=ticks sine-periods-per-tick=1 tick-interval=4000000000"),
        1000,
        1024,
        1,
        -24.0,
    );
}

#[test]
fn limiter_on_first_frame() {
    run_test(
        "wave=sine volume=0.05",
        Some("wave=ticks sine-periods-per-tick=10 tick-interval=4000000000"),
        1000,
        1024,
        1,
        -24.0,
    );
}
