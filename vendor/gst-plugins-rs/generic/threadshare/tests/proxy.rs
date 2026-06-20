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
        gstthreadshare::plugin_register_static().expect("gstthreadshare proxy test");
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
    let proxysink = gst::ElementFactory::make("ts-proxysink")
        .name("proxysink::test1")
        .property("proxy-context", "proxy::test1_proxy")
        .build()
        .unwrap();
    let proxysrc = gst::ElementFactory::make("ts-proxysrc")
        .name("proxysrc::test1")
        .property("proxy-context", "proxy::test1_proxy")
        .property("context", "proxy::test")
        .build()
        .unwrap();
    let appsink = gst_app::AppSink::builder().build();

    pipeline
        .add_many([&fakesrc, &proxysink, &proxysrc, appsink.upcast_ref()])
        .unwrap();
    fakesrc.link(&proxysink).unwrap();
    proxysrc.link(&appsink).unwrap();

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
            MessageView::Error(err) => unreachable!("proxy::test_push {:?}", err),
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

#[test]
fn test_from_pipeline_to_pipeline() {
    init();

    let pipe_1 = gst::Pipeline::default();
    let fakesrc = gst::ElementFactory::make("fakesrc").build().unwrap();
    let pxsink = gst::ElementFactory::make("ts-proxysink")
        .name("proxysink::test2")
        .property("proxy-context", "proxy::test2_proxy")
        .build()
        .unwrap();

    let pipe_2 = gst::Pipeline::default();
    let pxsrc = gst::ElementFactory::make("ts-proxysrc")
        .name("proxysrc::test2")
        .property("proxy-context", "proxy::test2_proxy")
        .property("context", "proxy::test")
        .build()
        .unwrap();
    let fakesink = gst::ElementFactory::make("fakesink").build().unwrap();

    pipe_1.add_many([&fakesrc, &pxsink]).unwrap();
    fakesrc.link(&pxsink).unwrap();

    pipe_2.add_many([&pxsrc, &fakesink]).unwrap();
    pxsrc.link(&fakesink).unwrap();

    pipe_1.set_state(gst::State::Paused).unwrap();
    pipe_2.set_state(gst::State::Paused).unwrap();

    let _ = pipe_1.state(gst::ClockTime::NONE);
    let _ = pipe_2.state(gst::ClockTime::NONE);

    pipe_1.set_state(gst::State::Null).unwrap();

    pipe_2.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_from_pipeline_to_pipeline_and_back() {
    init();

    let pipe_1 = gst::Pipeline::default();
    let pxsrc_1 = gst::ElementFactory::make("ts-proxysrc")
        .name("proxysrc1::test3")
        .property("proxy-context", "proxy::test3_proxy1")
        .property("context", "proxy::test")
        .build()
        .unwrap();
    let pxsink_1 = gst::ElementFactory::make("ts-proxysink")
        .name("proxysink1::test3")
        .property("proxy-context", "proxy::test3_proxy2")
        .build()
        .unwrap();

    let pipe_2 = gst::Pipeline::default();
    let pxsrc_2 = gst::ElementFactory::make("ts-proxysrc")
        .name("proxysrc2::test3")
        .property("proxy-context", "proxy::test3_proxy2")
        .property("context", "proxy::test")
        .build()
        .unwrap();
    let pxsink_2 = gst::ElementFactory::make("ts-proxysink")
        .name("proxysink2::test3")
        .property("proxy-context", "proxy::test3_proxy1")
        .build()
        .unwrap();

    pipe_1.add_many([&pxsrc_1, &pxsink_1]).unwrap();
    pxsrc_1.link(&pxsink_1).unwrap();

    pipe_2.add_many([&pxsrc_2, &pxsink_2]).unwrap();
    pxsrc_2.link(&pxsink_2).unwrap();

    pipe_1.set_state(gst::State::Paused).unwrap();
    pipe_2.set_state(gst::State::Paused).unwrap();

    pipe_1.set_state(gst::State::Null).unwrap();
    pipe_2.set_state(gst::State::Null).unwrap();
}
