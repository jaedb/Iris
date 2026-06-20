// Copyright (C) 2022 Philippe Normand <philn@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsvideofx::plugin_register_static().expect("Failed to register rsvideofx plugin");
    });
}

#[test]
fn test_red_color() {
    init();
    let pipeline = gst::Pipeline::default();

    let src = gst::ElementFactory::make("videotestsrc")
        .property_from_str("pattern", "red")
        .property("num-buffers", 2i32)
        .build()
        .unwrap();

    let filter = gst::ElementFactory::make("colordetect").build().unwrap();
    let sink = gst::ElementFactory::make("fakevideosink").build().unwrap();

    pipeline
        .add_many([&src, &filter, &sink])
        .expect("failed to add elements to the pipeline");
    gst::Element::link_many([&src, &filter, &sink]).expect("failed to link the elements");

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    let mut detected_color: Option<String> = None;
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Element(elt) => {
                if let Some(s) = elt.structure() {
                    if s.name() == "colordetect" {
                        // The video source emits 2 red frames, but we should
                        // receive only one message because the dominant color
                        // doesn't change.
                        assert_eq!(detected_color.as_deref(), None);
                        detected_color = Some(s.get::<String>("dominant-color").unwrap());
                    }
                }
            }
            MessageView::Eos(..) => break,
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();

    assert_eq!(detected_color.as_deref(), Some("red"));
}
