// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use byte_slice_cast::*;

// This macro allows us to create a kind of dynamic CSD file,
// we need to pass in the ksmps, channels and input/output
// operations that are going to be done by Csound over input and output
// audio samples
macro_rules! CSD {
    ($ksmps:expr, $ichannels:expr, $ochannels:expr, $ins:expr, $out:expr) => {
        format!(
            "
            <CsoundSynthesizer>
            <CsOptions>
            </CsOptions>
            <CsInstruments>
            sr = 44100 ; default sample rate
            ksmps = {}
            nchnls_i = {}
            nchnls = {}
            0dbfs  = 1

            instr 1

            {} ;input
                {}	; csound output

            endin
            </CsInstruments>
            <CsScore>
            i 1 0 2
            e
            </CsScore>
            </CsoundSynthesizer>",
            $ksmps, $ichannels, $ochannels, $ins, $out
        )
    };
}

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstcsound::plugin_register_static().expect("Failed to register csound plugin");
    });
}

fn build_harness(src_caps: gst::Caps, sink_caps: gst::Caps, csd: &str) -> gst_check::Harness {
    let filter = gst::ElementFactory::make("csoundfilter")
        .property("csd-text", csd)
        .build()
        .unwrap();

    let mut h = gst_check::Harness::with_element(&filter, Some("sink"), Some("src"));

    h.set_caps(src_caps, sink_caps);
    h
}

fn duration_from_samples(num_samples: u64, rate: u64) -> Option<gst::ClockTime> {
    num_samples
        .mul_div_round(*gst::ClockTime::SECOND, rate)
        .map(gst::ClockTime::from_nseconds)
}

// This test verifies the well functioning of the EOS logic,
// we generate EOS_NUM_BUFFERS=10 buffers with EOS_NUM_SAMPLES=62 samples each one,
// for a total of 10 * 62 = 620 samples, but 620%32(ksmps)= 12 will be leftover and should be processed when
// the eos event is received, which generates another buffer, so that, the total amount of buffers that
// the harness would have at its sinkpad should be EOS_NUM_BUFFERS + 1, being the total amount of processed samples
// equals to EOS_NUM_BUFFERS * EOS_NUM_SAMPLES = 620 samples.It is important to mention that the created buffers have silenced samples(being 0),
// but csoundfilter would add 1.0 to each incoming sample.
// at the end, all of the output samples should have a value of 1.0.
const EOS_NUM_BUFFERS: usize = 10;
const EOS_NUM_SAMPLES: usize = 62;
#[test]
fn csound_filter_eos() {
    init();

    // Sets the ksmps to 32,
    // input = output channels = 1
    let ksmps: usize = 32;
    let num_channels = 1;
    let sr: i32 = 44_100;

    let caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(sr)
        .channels(num_channels)
        .build();

    let mut h = build_harness(
        caps.clone(),
        caps,
        // this score instructs Csound to add 1.0 to each input sample
        &CSD!(ksmps, num_channels, num_channels, "ain in", "out ain + 1.0"),
    );
    h.play();

    // The input buffer pts and duration
    let mut in_pts = gst::ClockTime::ZERO;
    let in_duration = duration_from_samples(EOS_NUM_SAMPLES as _, sr as _)
        .expect("duration defined because sr is > 0");
    // The number of samples that were leftover during the previous iteration
    let mut samples_offset = 0;
    // Output samples and buffers counters
    let mut num_samples: usize = 0;
    let mut num_buffers = 0;
    // The expected pts of output buffers
    let mut expected_pts = gst::ClockTime::ZERO;

    for _ in 0..EOS_NUM_BUFFERS {
        let mut buffer =
            gst::Buffer::with_size(EOS_NUM_SAMPLES * std::mem::size_of::<f64>()).unwrap();

        buffer.make_mut().set_pts(in_pts);
        buffer.make_mut().set_duration(in_duration);

        let in_samples = samples_offset + EOS_NUM_SAMPLES as u64;
        // Gets amount of samples that are going to be processed,
        // the output buffer must be in_process_samples length
        let in_process_samples = in_samples - (in_samples % ksmps as u64);

        // Push an input buffer and pull the result of processing it
        let buffer = h.push_and_pull(buffer);
        assert!(buffer.is_ok());

        let buffer = buffer.unwrap();

        // Checks output buffer timestamp and duration
        assert_eq!(
            buffer.as_ref().duration(),
            duration_from_samples(in_process_samples, sr as _)
        );
        assert_eq!(buffer.as_ref().pts(), Some(expected_pts));

        // Get the number of samples that were not processed
        samples_offset = in_samples % ksmps as u64;
        // Calculates the next output buffer timestamp
        expected_pts = in_pts
            + duration_from_samples(EOS_NUM_SAMPLES as u64 - samples_offset, sr as _)
                .expect("duration defined because sr is > 0");
        // Calculates the next input buffer timestamp
        in_pts += in_duration;

        let map = buffer.into_mapped_buffer_readable().unwrap();
        let output = map.as_slice().as_slice_of::<f64>().unwrap();

        // all samples in the output buffers must value 1
        assert!(output.iter().all(|sample| *sample as u16 == 1u16));

        num_samples += output.len();
        num_buffers += 1;
    }

    h.push_event(gst::event::Eos::new());

    // pull the buffer produced after the EOS event
    let buffer = h.pull().unwrap();

    let samples_at_eos = (EOS_NUM_BUFFERS * EOS_NUM_SAMPLES) % ksmps;
    assert_eq!(
        buffer.as_ref().pts(),
        in_pts.opt_sub(duration_from_samples(samples_at_eos as _, sr as _))
    );

    let map = buffer.into_mapped_buffer_readable().unwrap();
    let output = map.as_slice().as_slice_of::<f64>().unwrap();
    num_samples += output.len();
    num_buffers += 1;

    assert_eq!(output.len(), samples_at_eos);
    assert!(output.iter().all(|sample| *sample as u16 == 1u16));

    // All the generated samples should have been processed at this point
    assert_eq!(num_samples, EOS_NUM_SAMPLES * EOS_NUM_BUFFERS);
    assert_eq!(num_buffers, EOS_NUM_BUFFERS + 1);
}

// In this test, we generate UNDERFLOW_NUM_BUFFERS buffers with UNDERFLOW_NUM_SAMPLES samples each one, however,
// Csound is waiting for UNDERFLOW_NUM_SAMPLES * 2 samples per buffer at its input, so that,
// internally, the output will be only generated when enough data is available.
// It happens, after every 2 * UNDERFLOW_NUM_BUFFERS input buffers, after processing, we should have UNDERFLOW_NUM_BUFFERS/2
// output buffers containing UNDERFLOW_NUM_SAMPLES * 2 samples.
const UNDERFLOW_NUM_BUFFERS: usize = 200;
const UNDERFLOW_NUM_SAMPLES: usize = 2;
#[test]
fn csound_filter_underflow() {
    init();

    let ksmps: usize = UNDERFLOW_NUM_SAMPLES * 2;
    let num_channels = 1;
    let sr: i32 = 44_100;

    let caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(sr)
        .channels(num_channels)
        .build();

    let mut h = build_harness(
        caps.clone(),
        caps,
        &CSD!(ksmps, num_channels, num_channels, "ain in", "out ain"),
    );
    h.play();

    // Input buffers timestamp
    let mut in_pts = gst::ClockTime::ZERO;
    let in_samples_duration = duration_from_samples(UNDERFLOW_NUM_SAMPLES as _, sr as _)
        .expect("duration defined because sr is > 0");

    for _ in 0..UNDERFLOW_NUM_BUFFERS {
        let mut buffer =
            gst::Buffer::with_size(UNDERFLOW_NUM_SAMPLES * std::mem::size_of::<f64>()).unwrap();

        buffer.make_mut().set_pts(in_pts);
        buffer.make_mut().set_duration(in_samples_duration);

        in_pts += in_samples_duration;

        assert!(h.push(buffer).is_ok());
    }

    h.push_event(gst::event::Eos::new());

    // From here we check our output data
    let mut num_buffers = 0;
    let mut num_samples = 0;

    let expected_duration = duration_from_samples(UNDERFLOW_NUM_SAMPLES as u64 * 2, sr as _)
        .expect("duration defined because sr is > 0");
    let expected_buffers = UNDERFLOW_NUM_BUFFERS / 2;
    let mut expected_pts = gst::ClockTime::ZERO;

    for _ in 0..expected_buffers {
        let buffer = h.pull().unwrap();
        let samples = buffer.size() / std::mem::size_of::<f64>();

        assert_eq!(buffer.as_ref().pts(), Some(expected_pts));
        assert_eq!(buffer.as_ref().duration(), Some(expected_duration));
        assert_eq!(samples, UNDERFLOW_NUM_SAMPLES * 2);
        // Output data is produced after 2 input buffers
        // so that, the next output buffer's PTS should be
        // equal to the last PTS plus the duration of 2 input buffers
        expected_pts += 2 * in_samples_duration;

        num_buffers += 1;
        num_samples += samples;
    }

    assert_eq!(num_buffers, UNDERFLOW_NUM_BUFFERS / 2);
    assert_eq!(num_samples, UNDERFLOW_NUM_SAMPLES * UNDERFLOW_NUM_BUFFERS);
}

// Verifies that the caps negotiation is properly done, by pushing buffers whose caps
// are the same as the one configured in csound, into the harness sink pad. Csoundfilter is expecting 2 channels audio
// at a sample rate of 44100.
// the output caps configured in the harness are not fixated but when the caps negotiation ends,
// those caps must be fixated according to the csound output format which is defined once the csd file is compiled
#[test]
fn csound_filter_caps_negotiation() {
    init();

    let ksmps = 4;
    let ichannels = 2;
    let ochannels = 1;
    let sr: i32 = 44_100;

    let src_caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(sr)
        .channels(ichannels)
        .build();

    // Define the output caps which would be fixated
    // at the end of the caps negotiation
    let sink_caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate_range(1..=48000)
        .channels_range(1..=2)
        .build();

    // build the harness setting its src and sink caps,
    // also passing the csd score to the filter element
    let mut h = build_harness(
        src_caps,
        sink_caps,
        // creates a csd score that defines the input and output formats on the csound side
        // the output fomart would be 1 channel audio samples at 44100
        &CSD!(ksmps, ichannels, ochannels, "ain, ain2 ins", "out ain"),
    );

    h.play();
    assert!(h.push(gst::Buffer::with_size(2048).unwrap()).is_ok());

    h.push_event(gst::event::Eos::new());

    let buffer = h.pull().unwrap();

    // Pushing a buffer without a timestamp should produce a no timestamp output
    assert!(buffer.as_ref().pts().is_none());
    // But It should have a duration
    assert_eq!(
        buffer.as_ref().duration(),
        duration_from_samples(1024 / std::mem::size_of::<f64>() as u64, sr as u64)
    );

    // get the negotiated harness sink caps
    let harness_sink_caps = h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");

    // our expected caps at the harness sinkpad
    let expected_caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(44_100)
        .channels(ochannels)
        .build();

    assert_eq!(harness_sink_caps, expected_caps);
}

// Similar to caps negotiation, but in this case, we configure a fixated caps in the harness sinkpad,
// such caps are incompatible with the csoundfilter and  it leads to an error during the caps negotiation,
// because there is not a common intersection between both caps.
#[test]
fn csound_filter_caps_negotiation_fail() {
    init();

    let ksmps = 4;
    let ichannels = 2;
    let ochannels = 1;

    let src_caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(44_100)
        .channels(ichannels)
        .build();

    // instead of having a range for channels/rate fields
    // we fixate them  to 2 and 48_000 respectively, which would cause the negotiation error
    let sink_caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AUDIO_FORMAT_F64)
        .rate(48_000)
        .channels(ichannels)
        .build();

    let mut h = build_harness(
        src_caps,
        sink_caps,
        // creates a csd score that defines the input and output formats on the csound side
        // the output fomart would be 1 channel audio samples at 44100
        &CSD!(ksmps, ichannels, ochannels, "ain, ain2 ins", "out ain"),
    );

    h.play();

    let buffer = gst::Buffer::with_size(2048).unwrap();
    assert!(h.push(buffer).is_err());

    h.push_event(gst::event::Eos::new());

    // The harness sinkpad end up not having defined caps
    // so, the get_current_caps should be None
    let current_caps = h.sinkpad().expect("harness has no sinkpad").current_caps();

    assert!(current_caps.is_none());
}
