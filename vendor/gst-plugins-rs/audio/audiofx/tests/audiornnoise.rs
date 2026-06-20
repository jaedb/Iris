// Copyright (C) 2020 Philippe Normand <philn@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use byte_slice_cast::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsaudiofx::plugin_register_static().expect("Failed to register rsaudiofx plugin");
    });
}

#[test]
fn test_rnnoise_silence_big_buffers() {
    init();
    let audio_info = gst_audio::AudioInfo::builder(gst_audio::AUDIO_FORMAT_F32, 48000, 2)
        .build()
        .unwrap();
    test_rnnoise(&audio_info, 4096);
}

#[test]
fn test_rnnoise_silence_small_buffers() {
    init();
    let audio_info = gst_audio::AudioInfo::builder(gst_audio::AUDIO_FORMAT_F32, 48000, 2)
        .build()
        .unwrap();
    test_rnnoise(&audio_info, 1024);
}

fn test_rnnoise(audio_info: &gst_audio::AudioInfo, buffer_size: usize) {
    let filter = gst::ElementFactory::make("audiornnoise").build().unwrap();
    let mut h = gst_check::Harness::with_element(&filter, Some("sink"), Some("src"));
    let sink_caps = audio_info.to_caps().unwrap();
    let src_caps = sink_caps.clone();
    h.set_caps(src_caps, sink_caps);
    h.play();

    let buffer = {
        let mut buffer = gst::Buffer::with_size(buffer_size * audio_info.bpf() as usize).unwrap();
        {
            let format_info = audio_info.format_info();
            let buffer_mut = buffer.get_mut().unwrap();
            let mut omap = buffer_mut.map_writable().unwrap();
            let odata = omap.as_mut_slice_of::<u8>().unwrap();
            format_info.fill_silence(odata);
        }

        buffer
    };

    let num_buffers = 5;
    let mut total_processed = 0;
    for _ in 0..num_buffers {
        let result = h.push_and_pull(buffer.clone()).unwrap();
        let map = result.into_mapped_buffer_readable().unwrap();
        let output = map.as_slice().as_slice_of::<f64>().unwrap();

        // all samples in the output buffers must value 0
        assert!(output.iter().all(|sample| *sample as u16 == 0u16));
        total_processed += output.len();
    }
    h.push_event(gst::event::Eos::new());

    let last_buffer = h.pull().unwrap();
    let map = last_buffer.into_mapped_buffer_readable().unwrap();
    let output = map.as_slice().as_slice_of::<f64>().unwrap();
    total_processed += output.len();
    // The total amount of samples pushed into the element shall be equal to the
    // total amount of samples pulled from it.
    assert_eq!(total_processed, num_buffers * buffer_size);
}
