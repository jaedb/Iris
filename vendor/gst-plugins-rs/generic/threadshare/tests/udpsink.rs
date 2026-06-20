// Copyright (C) 2019 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::thread;

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare udpsrc test");
    });
}

#[test]
fn test_client_management() {
    init();

    let h = gst_check::Harness::new("ts-udpsink");
    let udpsink = h.element().unwrap();

    let clients = udpsink.property::<String>("clients");

    assert_eq!(clients, "127.0.0.1:5004");

    udpsink.emit_by_name::<()>("add", &[&"192.168.1.1", &57i32]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "127.0.0.1:5004,192.168.1.1:57");

    /* Adding a client twice is not supported */
    udpsink.emit_by_name::<()>("add", &[&"192.168.1.1", &57i32]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "127.0.0.1:5004,192.168.1.1:57");

    udpsink.emit_by_name::<()>("remove", &[&"192.168.1.1", &57i32]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "127.0.0.1:5004");

    /* Removing a non-existing client should not be a problem */
    udpsink.emit_by_name::<()>("remove", &[&"192.168.1.1", &57i32]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "127.0.0.1:5004");

    /* Removing the default client is possible */
    udpsink.emit_by_name::<()>("remove", &[&"127.0.0.1", &5004i32]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "");

    /* The client properties is writable too */
    udpsink.set_property("clients", "127.0.0.1:5004,192.168.1.1:57");
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "127.0.0.1:5004,192.168.1.1:57");

    udpsink.emit_by_name::<()>("clear", &[]);
    let clients = udpsink.property::<String>("clients");
    assert_eq!(clients, "");
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/250
#[ignore]
fn test_chain() {
    init();

    let mut h = gst_check::Harness::new("ts-udpsink");
    h.set_src_caps_str("foo/bar");
    {
        let udpsink = h.element().unwrap();
        udpsink.set_property("clients", "127.0.0.1:5005");
    }

    thread::spawn(move || {
        use std::net;
        use std::time;

        thread::sleep(time::Duration::from_millis(50));

        let socket = net::UdpSocket::bind("127.0.0.1:5005").unwrap();
        let mut buf = [0; 5];
        let (amt, _) = socket.recv_from(&mut buf).unwrap();

        assert!(amt == 4);
        assert!(buf == [42, 43, 44, 45, 0]);
    });

    let buf = gst::Buffer::from_slice([42, 43, 44, 45]);
    assert!(h.push(buf) == Ok(gst::FlowSuccess::Ok));
}
