// decrypter.rs
//
// Copyright 2019 Jordan Petridis <jordan@centricular.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
// SPDX-License-Identifier: MIT

use gst::glib;
use gst::prelude::*;

use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use pretty_assertions::assert_eq;

use std::sync::LazyLock;
static SENDER_PUBLIC: LazyLock<glib::Bytes> = LazyLock::new(|| {
    let public = [
        66, 248, 199, 74, 216, 55, 228, 116, 52, 17, 147, 56, 65, 130, 134, 148, 157, 153, 235,
        171, 179, 147, 120, 71, 100, 243, 133, 120, 160, 14, 111, 65,
    ];
    glib::Bytes::from_owned(public)
});
static RECEIVER_PRIVATE: LazyLock<glib::Bytes> = LazyLock::new(|| {
    let secret = [
        54, 221, 217, 54, 94, 235, 167, 2, 187, 249, 71, 31, 59, 27, 19, 166, 78, 236, 102, 48, 29,
        142, 41, 189, 22, 146, 218, 69, 147, 165, 240, 235,
    ];
    glib::Bytes::from_owned(secret)
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstsodium::plugin_register_static().unwrap();
    });
}

#[test]
fn test_pipeline() {
    init();

    let pipeline = gst::Pipeline::builder()
        .name("sodium-decrypter-test")
        .build();

    let input_path = {
        let mut r = PathBuf::new();
        r.push(env!("CARGO_MANIFEST_DIR"));
        r.push("tests");
        r.push("encrypted_sample");
        r.set_extension("enc");
        r
    };

    let filesrc = gst::ElementFactory::make("filesrc")
        .property("location", input_path.to_str().unwrap())
        .build()
        .unwrap();

    let dec = gst::ElementFactory::make("sodiumdecrypter")
        .property("sender-key", &*SENDER_PUBLIC)
        .property("receiver-key", &*RECEIVER_PRIVATE)
        .build()
        .unwrap();

    // the typefind element here is cause the decrypter only supports
    // operating in pull mode bu the filesink wants push-mode.
    let typefind = gst::ElementFactory::make("typefind").build().unwrap();
    let sink = gst_app::AppSink::builder().build();

    pipeline
        .add_many([&filesrc, &dec, &typefind, sink.upcast_ref()])
        .expect("failed to add elements to the pipeline");
    gst::Element::link_many([&filesrc, &dec, &typefind, sink.upcast_ref()])
        .expect("failed to link the elements");

    let adapter = Arc::new(Mutex::new(gst_base::UniqueAdapter::new()));

    let adapter_clone = adapter.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;

                let mut adapter = adapter_clone.lock().unwrap();
                adapter.push(buffer.to_owned());

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
        .expect("Unable to set the pipeline to the `Playing` state");

    let expected_output = include_bytes!("sample.mp3");

    let mut adapter = adapter.lock().unwrap();
    let available = adapter.available();
    assert_eq!(available, expected_output.len());
    let output_buffer = adapter.take_buffer(available).unwrap();
    let output = output_buffer.map_readable().unwrap();
    assert_eq!(expected_output.as_ref(), output.as_ref());
}

#[test]
fn test_pull_range() {
    init();

    let pipeline = gst::Pipeline::builder()
        .name("sodium-decrypter-pull-range-test")
        .build();
    let input_path = {
        let mut r = PathBuf::new();
        r.push(env!("CARGO_MANIFEST_DIR"));
        r.push("tests");
        r.push("encrypted_sample");
        r.set_extension("enc");
        r
    };

    let filesrc = gst::ElementFactory::make("filesrc")
        .property("location", input_path.to_str().unwrap())
        .build()
        .unwrap();

    let dec = gst::ElementFactory::make("sodiumdecrypter")
        .property("sender-key", &*SENDER_PUBLIC)
        .property("receiver-key", &*RECEIVER_PRIVATE)
        .build()
        .unwrap();

    pipeline
        .add_many([&filesrc, &dec])
        .expect("failed to add elements to the pipeline");
    gst::Element::link_many([&filesrc, &dec]).expect("failed to link the elements");

    // Activate in the pad in pull mode
    pipeline
        .set_state(gst::State::Ready)
        .expect("Unable to set the pipeline to the `Playing` state");
    let srcpad = dec.static_pad("src").unwrap();
    srcpad.activate_mode(gst::PadMode::Pull, true).unwrap();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Test that the decryptor is seekable
    let mut q = gst::query::Seeking::new(gst::Format::Bytes);
    srcpad.query(&mut q);

    // get the seeking capabilities
    let (seekable, start, stop) = q.result();
    assert!(seekable);
    assert_eq!(start, 0.bytes().into());
    assert_eq!(stop, 6043.bytes().into());

    // do pulls
    let expected_array_1 = [
        255, 251, 192, 196, 0, 0, 13, 160, 37, 86, 116, 240, 0, 42, 73, 33, 43, 63, 61, 16, 128, 5,
        53, 37, 220, 28, 225, 35, 16, 243, 140, 220, 4, 192, 2, 64, 14, 3, 144, 203, 67, 208, 244,
        61, 70, 175, 103, 127, 28, 0,
    ];
    let buf1 = srcpad.range(0, 50).unwrap();
    assert_eq!(buf1.size(), 50);
    let map1 = buf1.map_readable().unwrap();
    assert_eq!(&map1[..], &expected_array_1[..]);

    let expected_array_2 = [
        255, 251, 192, 196, 0, 0, 13, 160, 37, 86, 116, 240, 0, 42, 73, 33, 43, 63, 61, 16, 128, 5,
        53, 37, 220, 28, 225, 35, 16, 243, 140, 220, 4, 192, 2, 64, 14, 3, 144, 203, 67, 208, 244,
        61, 70, 175, 103, 127, 28, 0, 0, 0, 0, 12, 60, 60, 60, 122, 0, 0, 0, 12, 195, 195, 195,
        207, 192, 0, 0, 3, 113, 195, 199, 255, 255, 254, 97, 225, 225, 231, 160, 0, 0, 49, 24, 120,
        120, 121, 232, 0, 0, 12, 252, 195, 195, 199, 128, 0, 0, 0,
    ];
    let buf2 = srcpad.range(0, 100).unwrap();
    assert_eq!(buf2.size(), 100);
    let map2 = buf2.map_readable().unwrap();
    assert_eq!(&map2[..], &expected_array_2[..]);

    // compare the first 50 bytes of the two slices
    // they should match
    assert_eq!(&map1[..], &map2[..map1.len()]);

    // request in the middle of a block
    let buf = srcpad.range(853, 100).unwrap();
    // result size doesn't include the block macs,
    assert_eq!(buf.size(), 100);

    // read till eos, this also will pull multiple blocks
    let buf = srcpad.range(853, 42000).unwrap();
    // 6031 (size of file) - 883 (requersted offset) - headers size - (numbler of blcks * block mac)
    assert_eq!(buf.size(), 5054);

    // read 0 bytes from the start
    let buf = srcpad.range(0, 0).unwrap();
    assert_eq!(buf.size(), 0);

    // read 0 bytes somewhere in the middle
    let buf = srcpad.range(4242, 0).unwrap();
    assert_eq!(buf.size(), 0);

    // read 0 bytes to eos
    let res = srcpad.range(6003, 0);
    assert_eq!(res, Err(gst::FlowError::Eos));

    // read 100 bytes at eos
    let res = srcpad.range(6003, 100);
    assert_eq!(res, Err(gst::FlowError::Eos));

    // read 100 bytes way past eos
    let res = srcpad.range(424_242, 100);
    assert_eq!(res, Err(gst::FlowError::Eos));

    // read 10 bytes at eos -1, should return a single byte
    let buf = srcpad.range(5906, 10).unwrap();
    assert_eq!(buf.size(), 1);

    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Playing` state");
}

#[test]
fn test_state_changes() {
    init();

    // NullToReady without keys provided
    {
        let dec = gst::ElementFactory::make("sodiumdecrypter")
            .build()
            .unwrap();
        assert!(dec.change_state(gst::StateChange::NullToReady).is_err());

        // Set only receiver key
        let dec = gst::ElementFactory::make("sodiumdecrypter")
            .property("receiver-key", &*RECEIVER_PRIVATE)
            .build()
            .unwrap();
        assert!(dec.change_state(gst::StateChange::NullToReady).is_err());

        // Set only sender key
        let dec = gst::ElementFactory::make("sodiumdecrypter")
            .property("sender-key", &*SENDER_PUBLIC)
            .build()
            .unwrap();
        assert!(dec.change_state(gst::StateChange::NullToReady).is_err());
    }

    // NullToReady, no nonce provided
    {
        let dec = gst::ElementFactory::make("sodiumdecrypter")
            .property("sender-key", &*SENDER_PUBLIC)
            .property("receiver-key", &*RECEIVER_PRIVATE)
            .build()
            .unwrap();
        assert!(dec.change_state(gst::StateChange::NullToReady).is_ok());
    }

    // ReadyToNull
    {
        let dec = gst::ElementFactory::make("sodiumdecrypter")
            .property("sender-key", &*SENDER_PUBLIC)
            .property("receiver-key", &*RECEIVER_PRIVATE)
            .build()
            .unwrap();
        assert!(dec.change_state(gst::StateChange::NullToReady).is_ok());
    }
}
