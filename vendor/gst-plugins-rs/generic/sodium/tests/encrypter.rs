// encrypter.rs
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

use std::sync::LazyLock;

use pretty_assertions::assert_eq;

static RECEIVER_PUBLIC: LazyLock<glib::Bytes> = LazyLock::new(|| {
    let public = [
        28, 95, 33, 124, 28, 103, 80, 78, 7, 28, 234, 40, 226, 179, 253, 166, 169, 64, 78, 5, 57,
        92, 151, 179, 221, 89, 68, 70, 44, 225, 219, 19,
    ];

    glib::Bytes::from_owned(public)
});
static SENDER_PRIVATE: LazyLock<glib::Bytes> = LazyLock::new(|| {
    let secret = [
        154, 227, 90, 239, 206, 184, 202, 234, 176, 161, 14, 91, 218, 98, 142, 13, 145, 223, 210,
        222, 224, 240, 98, 51, 142, 165, 255, 1, 159, 100, 242, 162,
    ];
    glib::Bytes::from_owned(secret)
});
static NONCE: LazyLock<glib::Bytes> = LazyLock::new(|| {
    let nonce = [
        144, 187, 179, 230, 15, 4, 241, 15, 37, 133, 22, 30, 50, 106, 70, 159, 243, 218, 173, 22,
        18, 36, 4, 45,
    ];
    glib::Bytes::from_owned(nonce)
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstsodium::plugin_register_static().unwrap();
        // set the nonce
        std::env::set_var("GST_SODIUM_ENCRYPT_NONCE", hex::encode(&*NONCE));
    });
}

#[test]
fn encrypt_file() {
    init();

    let input = include_bytes!("sample.mp3");
    let expected_output = include_bytes!("encrypted_sample.enc");

    let mut adapter = gst_base::UniqueAdapter::new();

    let enc = gst::ElementFactory::make("sodiumencrypter")
        .property("sender-key", &*SENDER_PRIVATE)
        .property("receiver-key", &*RECEIVER_PUBLIC)
        .property("block-size", 1024u32)
        .build()
        .unwrap();

    let mut h = gst_check::Harness::with_element(&enc, None, None);
    h.add_element_src_pad(&enc.static_pad("src").expect("failed to get src pad"));
    h.add_element_sink_pad(&enc.static_pad("sink").expect("failed to get src pad"));
    h.set_src_caps_str("application/x-sodium-encrypted");

    let buf = gst::Buffer::from_mut_slice(Vec::from(&input[..]));

    assert_eq!(h.push(buf), Ok(gst::FlowSuccess::Ok));
    h.push_event(gst::event::Eos::new());

    println!("Pulling buffer...");
    while let Ok(buf) = h.pull() {
        adapter.push(buf);
        if adapter.available() >= expected_output.len() {
            break;
        }
    }

    let buf = adapter
        .take_buffer(adapter.available())
        .expect("failed to take buffer");
    let map = buf.map_readable().expect("Couldn't map buffer readable");
    assert_eq!(map.as_ref(), expected_output.as_ref());
}

#[test]
fn test_state_changes() {
    init();

    // NullToReady without keys provided
    {
        let enc = gst::ElementFactory::make("sodiumencrypter")
            .build()
            .unwrap();
        assert!(enc.change_state(gst::StateChange::NullToReady).is_err());

        // Set only receiver key
        let enc = gst::ElementFactory::make("sodiumencrypter")
            .property("receiver-key", &*RECEIVER_PUBLIC)
            .build()
            .unwrap();
        assert!(enc.change_state(gst::StateChange::NullToReady).is_err());

        // Set only sender key
        let enc = gst::ElementFactory::make("sodiumencrypter")
            .property("sender-key", &*SENDER_PRIVATE)
            .build()
            .unwrap();
        assert!(enc.change_state(gst::StateChange::NullToReady).is_err());
    }

    // NullToReady
    {
        let enc = gst::ElementFactory::make("sodiumencrypter")
            .property("sender-key", &*SENDER_PRIVATE)
            .property("receiver-key", &*RECEIVER_PUBLIC)
            .build()
            .unwrap();
        assert!(enc.change_state(gst::StateChange::NullToReady).is_ok());
    }

    // ReadyToNull
    {
        let enc = gst::ElementFactory::make("sodiumencrypter")
            .property("sender-key", &*SENDER_PRIVATE)
            .property("receiver-key", &*RECEIVER_PUBLIC)
            .build()
            .unwrap();
        assert!(enc.change_state(gst::StateChange::NullToReady).is_ok());
    }
}
