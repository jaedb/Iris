// Copyright (C) 2019 Ruben Gonzalez <rgonzalez@fluendo.com>
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
        gstclaxon::plugin_register_static().expect("claxon test");
    });
}

#[test]
fn test_mono_s16() {
    let data = include_bytes!("test_mono_s16.flac");
    // 4 fLaC header, 38 streaminfo_header, 66 other header, 18 data
    let packet_sizes = [4, 38, 66, 18];
    let decoded_samples = [0usize, 0usize, 0usize, 2];

    let caps = do_test(data, &packet_sizes, &decoded_samples);

    assert_eq!(
        caps,
        gst_audio::AudioCapsBuilder::new_interleaved()
            .format(gst_audio::AUDIO_FORMAT_S16)
            .rate(44_100)
            .channels(1)
            .build()
    );
}

#[test]
fn test_stereo_s32() {
    let data = include_bytes!("test_stereo_s32.flac");
    // 4 fLaC header, 38 streaminfo_header, 17465 data
    let packet_sizes = [4, 38, 17465];
    let decoded_samples = [0usize, 0usize, 8192];

    let caps = do_test(data, &packet_sizes, &decoded_samples);

    assert_eq!(
        caps,
        gst_audio::AudioCapsBuilder::new_interleaved()
            .format(gst_audio::AUDIO_FORMAT_S2432)
            .rate(44100)
            .channels(2)
            .channel_mask(0x3)
            .build()
    );
}

fn do_test(data: &'static [u8], packet_sizes: &[usize], decoded_samples: &[usize]) -> gst::Caps {
    let packet_offsets = packet_sizes
        .iter()
        .scan(0, |state, &size| {
            *state += size;
            Some(*state)
        })
        .collect::<Vec<usize>>();

    init();

    let mut h = gst_check::Harness::new("claxondec");
    h.play();

    let caps = gst::Caps::builder("audio/x-flac")
        .field("framed", true)
        .build();
    h.set_src_caps(caps);

    let packet_offsets_iter = std::iter::once(&0).chain(packet_offsets.iter());
    let skip = 0;

    for (offset_start, offset_end) in packet_offsets_iter
        .clone()
        .skip(skip)
        .zip(packet_offsets_iter.clone().skip(skip + 1))
    {
        let buffer = gst::Buffer::from_slice(&data[*offset_start..*offset_end]);
        h.push(buffer).unwrap();
    }

    h.push_event(gst::event::Eos::new());

    for samples in decoded_samples {
        if *samples == 0 {
            continue;
        }
        let buffer = h.pull().unwrap();
        assert_eq!(buffer.size(), 4 * samples);
    }

    h.sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps")
}
