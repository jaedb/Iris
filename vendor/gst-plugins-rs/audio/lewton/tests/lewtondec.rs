// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstlewton::plugin_register_static().expect("lewton test");
    });
}

#[test]
fn test_with_streamheader() {
    run_test(false);
}

#[test]
fn test_with_inline_headers() {
    run_test(true);
}

fn run_test(inline_headers: bool) {
    let data = include_bytes!("test.vorbis");
    let packet_sizes = [30, 99, 3189, 43, 20, 56, 56, 21, 20, 22, 21, 22, 22, 43];
    let packet_offsets = packet_sizes
        .iter()
        .scan(0, |state, &size| {
            *state += size;
            Some(*state)
        })
        .collect::<Vec<usize>>();
    let decoded_samples = [0usize, 128, 576, 1472, 128, 128, 128, 128, 128, 128, 128];

    init();

    let mut h = gst_check::Harness::new("lewtondec");
    h.play();

    if inline_headers {
        let caps = gst::Caps::builder("audio/x-vorbis").build();
        h.set_src_caps(caps);
    } else {
        let caps = gst::Caps::builder("audio/x-vorbis")
            .field(
                "streamheader",
                gst::Array::new([
                    gst::Buffer::from_slice(&data[0..packet_offsets[0]]),
                    gst::Buffer::from_slice(&data[packet_offsets[0]..packet_offsets[1]]),
                    gst::Buffer::from_slice(&data[packet_offsets[1]..packet_offsets[2]]),
                ]),
            )
            .build();
        h.set_src_caps(caps);
    }

    let packet_offsets_iter = std::iter::once(&0).chain(packet_offsets.iter());
    let skip = if inline_headers { 0 } else { 3 };

    for (offset_start, offset_end) in packet_offsets_iter
        .clone()
        .skip(skip)
        .zip(packet_offsets_iter.clone().skip(skip + 1))
    {
        let buffer = gst::Buffer::from_slice(&data[*offset_start..*offset_end]);
        h.push(buffer).unwrap();
    }

    h.push_event(gst::event::Eos::new());

    for samples in &decoded_samples {
        if *samples == 0 {
            continue;
        }
        let buffer = h.pull().unwrap();
        assert_eq!(buffer.size(), 4 * samples);
    }

    let caps = h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");
    assert_eq!(
        caps,
        gst_audio::AudioCapsBuilder::new_interleaved()
            .format(gst_audio::AUDIO_FORMAT_F32)
            .rate(44_100)
            .channels(1)
            .build()
    );
}
