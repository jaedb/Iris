// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use serial_test::serial;
use std::thread;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstquinn::plugin_register_static().expect("QUIC source sink send receive tests");
    });
}

fn make_buffer(content: &[u8]) -> gst::Buffer {
    let mut buf = gst::Buffer::from_slice(content.to_owned());
    buf.make_mut().set_pts(gst::ClockTime::from_mseconds(200));
    buf
}

#[test]
#[serial]
fn test_send_receive_without_datagram() {
    init();

    let content = "Hello, world!\n".as_bytes();

    thread::spawn(move || {
        let mut h1 = gst_check::Harness::new_empty();
        h1.add_parse("quinnquicsink secure-connection=false");

        h1.set_src_caps(gst::Caps::builder("text/plain").build());

        h1.play();

        assert!(h1.push(make_buffer(content)) == Ok(gst::FlowSuccess::Ok));

        // Wait a bit before sending Eos and shutting down the pipeline
        thread::sleep(std::time::Duration::from_secs(2));

        h1.push_event(gst::event::Eos::new());

        h1.element().unwrap().set_state(gst::State::Null).unwrap();

        drop(h1);
    });

    let mut h2 = gst_check::Harness::new_empty();
    h2.add_parse("quinnquicsrc secure-connection=false");

    h2.play();

    let buf = h2.pull_until_eos().unwrap().unwrap();

    assert_eq!(
        content,
        buf.into_mapped_buffer_readable().unwrap().as_slice()
    );

    h2.element().unwrap().set_state(gst::State::Null).unwrap();

    drop(h2);
}

#[test]
#[serial]
fn test_send_receive_with_datagram() {
    init();

    let content = "Hello, world!\n".as_bytes();

    // Use a different port address compared to the default that will be used
    // in the other test. We get a address already in use error otherwise.
    thread::spawn(move || {
        let mut h1 = gst_check::Harness::new_empty();
        h1.add_parse(
            "quinnquicsrc use-datagram=true address=127.0.0.1 port=6000 secure-connection=false",
        );

        h1.play();

        let buf = h1.pull_until_eos().unwrap().unwrap();

        assert_eq!(
            content,
            buf.into_mapped_buffer_readable().unwrap().as_slice()
        );

        h1.element().unwrap().set_state(gst::State::Null).unwrap();

        drop(h1);
    });

    let mut h2 = gst_check::Harness::new_empty();
    h2.add_parse("quinnquicsink use-datagram=true bind-address=127.0.0.1 bind-port=6001 address=127.0.0.1 port=6000 secure-connection=false");

    h2.set_src_caps(gst::Caps::builder("text/plain").build());

    h2.play();

    assert!(h2.push(make_buffer(content)) == Ok(gst::FlowSuccess::Ok));

    h2.push_event(gst::event::Eos::new());

    h2.element().unwrap().set_state(gst::State::Null).unwrap();

    drop(h2);
}
