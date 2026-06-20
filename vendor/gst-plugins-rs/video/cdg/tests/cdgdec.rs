// Copyright (C) 2019 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::prelude::*;
use std::path::PathBuf;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstcdg::plugin_register_static().expect("cdgdec tests");
    });
}

#[test]
fn test_cdgdec() {
    init();

    let pipeline = gst::Pipeline::with_name("cdgdec-test");

    let input_path = {
        let mut r = PathBuf::new();
        r.push(env!("CARGO_MANIFEST_DIR"));
        r.push("tests");
        r.push("BrotherJohn");
        r.set_extension("cdg");
        r
    };

    // Ensure we are in push mode so 'blocksize' prop is used
    let filesrc = gst::ElementFactory::make("pushfilesrc")
        .property("location", input_path.to_str().unwrap())
        .build()
        .unwrap();
    {
        let child_proxy = filesrc.dynamic_cast_ref::<gst::ChildProxy>().unwrap();
        child_proxy.set_child_property("real-filesrc::num-buffers", 1);
        let blocksize: u32 = 24; // One CDG instruction
        child_proxy.set_child_property("real-filesrc::blocksize", blocksize);
    }

    let parse = gst::ElementFactory::make("cdgparse").build().unwrap();
    let dec = gst::ElementFactory::make("cdgdec").build().unwrap();
    let sink = gst_app::AppSink::builder().build();

    pipeline
        .add_many([&filesrc, &parse, &dec, sink.upcast_ref()])
        .expect("failed to add elements to the pipeline");
    gst::Element::link_many([&filesrc, &parse, &dec, sink.upcast_ref()])
        .expect("failed to link the elements");

    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

                // First frame fully blue
                map.as_slice()
                    .chunks_exact(4)
                    .for_each(|p| assert_eq!(p, [0, 0, 136, 255]));

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                eprintln!("Debugging information: {:?}", err.debug());
                unreachable!();
            }
            MessageView::Eos(..) => break,
            _ => (),
        }
    }

    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
