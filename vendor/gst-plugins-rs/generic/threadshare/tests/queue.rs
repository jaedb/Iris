// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use std::sync::{Arc, Mutex};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare queue test");
    });
}

#[test]
fn test_push() {
    init();

    let pipeline = gst::Pipeline::default();
    let fakesrc = gst::ElementFactory::make("fakesrc")
        .property("num-buffers", 3i32)
        .build()
        .unwrap();
    let queue = gst::ElementFactory::make("ts-queue").build().unwrap();
    let appsink = gst_app::AppSink::builder().build();

    pipeline
        .add_many([&fakesrc, &queue, appsink.upcast_ref()])
        .unwrap();
    fakesrc.link(&queue).unwrap();
    queue.link(&appsink).unwrap();

    let samples = Arc::new(Mutex::new(Vec::new()));

    let samples_clone = samples.clone();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().unwrap();

                samples_clone.lock().unwrap().push(sample);

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(5.seconds()) {
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

    assert!(eos);
    let samples = samples.lock().unwrap();
    assert_eq!(samples.len(), 3);

    for sample in samples.iter() {
        assert!(sample.buffer().is_some());
    }

    pipeline.set_state(gst::State::Null).unwrap();
}
