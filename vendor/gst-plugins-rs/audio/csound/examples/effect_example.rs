// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use std::error::Error;

const AUDIO_SRC: &str = "audiotestsrc";
const AUDIO_SINK: &str = "audioconvert ! autoaudiosink";

// This example defines two instruments, the first instrument send to the output that is at its input and accumulates the received audio samples
// into a global variable called gasig. The execution of this instrument last the first 2 seconds.
// The second instrument starts it execution at 1.8 second, This instrument creates two audio buffers with samples that are read
// from the global accumulator(gasig), then reads these buffers at a fixed delay time, creating the adelL, adelM and adelR buffers,
// also, It multiplies the audio samples in the right channel by 0.5 * kdel, being kdel a line of values starting at 0.5 at increments of 0.001.
// Finally, those buffers are mixed with the accumulator, and an audio envelop is applied(aseg) to them.
// The result is similar to an audio echo in which the buffered samples are read at different delay times and also modified in frequency(right channel),
// this creates an space effect using just one channel audio input.
const CSD: &str = "
    <CsoundSynthesizer>
    <CsOptions>
    </CsOptions>
    <CsInstruments>

    sr = 44100
    ksmps = 7

    nchnls_i = 1
    nchnls = 2

    gasig  init 0
    gidel  = 1

    instr 1

        ain in
            outs ain, ain

        vincr gasig, ain
    endin

    instr 2

        ifeedback = p4

        aseg linseg 1., p3, 0.0

        abuf2	delayr	gidel
        adelL 	deltap	.4
        adelM 	deltap	.5
            delayw	gasig + (adelL * ifeedback)

        abuf3	delayr	gidel
        kdel	line    .5, p3, .001
        adelR 	deltap  .5 * kdel
            delayw	gasig + (adelR * ifeedback)
    	outs	(adelL + adelM) * aseg, (adelR + adelM) * aseg
        clear	gasig
    endin

    </CsInstruments>
    <CsScore>

    i 1 0 2
    i 2 1.8 5 .8
    e
    </CsScore>
    </CsoundSynthesizer>";

fn create_pipeline() -> Result<gst::Pipeline, Box<dyn Error>> {
    let pipeline = gst::Pipeline::default();

    let audio_src = gst::parse::bin_from_description(AUDIO_SRC, true)?.upcast();

    let audio_sink = gst::parse::bin_from_description(AUDIO_SINK, true)?.upcast();

    let csoundfilter = gst::ElementFactory::make("csoundfilter")
        .property("csd-text", CSD)
        .build()
        .unwrap();

    pipeline.add_many([&audio_src, &csoundfilter, &audio_sink])?;

    audio_src.link_pads(Some("src"), &csoundfilter, Some("sink"))?;
    csoundfilter.link_pads(Some("src"), &audio_sink, Some("sink"))?;

    Ok(pipeline)
}

fn main_loop(pipeline: gst::Pipeline) -> Result<(), Box<dyn Error>> {
    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    msg.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    gst::init().unwrap();

    gstcsound::plugin_register_static().expect("Failed to register csound plugin");

    create_pipeline().and_then(main_loop)
}
