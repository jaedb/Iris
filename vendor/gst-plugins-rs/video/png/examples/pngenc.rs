// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::prelude::*;

const ENCODE_PIPELINE: &str = "videotestsrc is-live=false num-buffers=1 ! videoconvert ! video/x-raw, format=RGB, width=160, height=120 !
    rspngenc compression-level=2 filter=4 ! filesink location=frame.png";

fn main() {
    gst::init().unwrap();
    gstrspng::plugin_register_static().expect("Failed to register gif plugin");

    let pipeline = gst::parse::launch(ENCODE_PIPELINE).unwrap();
    let bus = pipeline.bus().unwrap();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Failed to set pipeline state to playing");

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Failed to set pipeline state to null");
}
