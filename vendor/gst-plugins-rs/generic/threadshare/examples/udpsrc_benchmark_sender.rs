// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use std::sync::LazyLock;

use std::net;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{env, thread, time};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-udpsrc-benchmark-sender",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing UDP src benchmark sender"),
    )
});

fn main() {
    gst::init().unwrap();
    gstthreadshare::plugin_register_static().unwrap();

    let args = env::args().collect::<Vec<_>>();
    assert!(args.len() > 1);
    let n_streams: u16 = args[1].parse().unwrap();

    let num_buffers: Option<i32> = if args.len() > 3 {
        args[3].parse().ok()
    } else {
        None
    };

    if args.len() > 2 {
        match args[2].as_str() {
            "raw" => send_raw_buffers(n_streams),
            "rtp" => send_rtp_buffers(n_streams, num_buffers),
            _ => send_test_buffers(n_streams, num_buffers),
        }
    } else {
        send_test_buffers(n_streams, num_buffers);
    }
}

fn send_raw_buffers(n_streams: u16) {
    let buffer = [0; 160];
    let socket = net::UdpSocket::bind("0.0.0.0:0").unwrap();

    let ipaddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let destinations = (5004..(5004 + n_streams))
        .map(|port| SocketAddr::new(ipaddr, port))
        .collect::<Vec<_>>();

    let wait = time::Duration::from_millis(20);

    thread::sleep(time::Duration::from_millis(1000));

    loop {
        let now = time::Instant::now();

        for dest in &destinations {
            socket.send_to(&buffer, dest).unwrap();
        }

        let elapsed = now.elapsed();
        if elapsed < wait {
            thread::sleep(wait - elapsed);
        }
    }
}

fn send_test_buffers(n_streams: u16, num_buffers: Option<i32>) {
    let pipeline = gst::Pipeline::default();
    for i in 0..n_streams {
        let src = gst::ElementFactory::make("ts-audiotestsrc")
            .name(format!("ts-audiotestsrc-{i}").as_str())
            .property("context-wait", 20u32)
            .property("do-timestamp", true)
            .property_if_some("num-buffers", num_buffers)
            .build()
            .unwrap();

        #[cfg(feature = "tuning")]
        if i == 0 {
            src.set_property("main-elem", true);
        }

        let sink = gst::ElementFactory::make("ts-udpsink")
            .name(format!("udpsink-{i}").as_str())
            .property("clients", format!("127.0.0.1:{}", i + 5004))
            .property("context-wait", 20u32)
            .build()
            .unwrap();

        let elements = &[&src, &sink];
        pipeline.add_many(elements).unwrap();
        gst::Element::link_many(elements).unwrap();
    }

    run(pipeline);
}

fn send_rtp_buffers(n_streams: u16, num_buffers: Option<i32>) {
    let pipeline = gst::Pipeline::default();
    for i in 0..n_streams {
        let src = gst::ElementFactory::make("ts-audiotestsrc")
            .name(format!("ts-audiotestsrc-{i}").as_str())
            .property("context-wait", 20u32)
            .property("do-timestamp", true)
            .property_if_some("num-buffers", num_buffers)
            .build()
            .unwrap();

        #[cfg(feature = "tuning")]
        if i == 0 {
            src.set_property("main-elem", true);
        }

        let enc = gst::ElementFactory::make("alawenc")
            .name(format!("alawenc-{i}").as_str())
            .build()
            .unwrap();
        let pay = gst::ElementFactory::make("rtppcmapay")
            .name(format!("rtppcmapay-{i}").as_str())
            .build()
            .unwrap();

        let sink = gst::ElementFactory::make("ts-udpsink")
            .name(format!("udpsink-{i}").as_str())
            .property("context-wait", 20u32)
            .property("clients", format!("127.0.0.1:{}", i + 5004))
            .build()
            .unwrap();

        let elements = &[&src, &enc, &pay, &sink];
        pipeline.add_many(elements).unwrap();
        gst::Element::link_many(elements).unwrap();
    }

    run(pipeline);
}

fn run(pipeline: gst::Pipeline) {
    let l = glib::MainLoop::new(None, false);

    let bus = pipeline.bus().unwrap();
    let l_clone = l.clone();
    let _bus_watch = bus
        .add_watch(move |_, msg| {
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(_) => {
                    gst::info!(CAT, "Received eos");
                    l_clone.quit();

                    glib::ControlFlow::Break
                }
                MessageView::Error(msg) => {
                    gst::error!(
                        CAT,
                        "Error from {:?}: {} ({:?})",
                        msg.src().map(|s| s.path_string()),
                        msg.error(),
                        msg.debug()
                    );
                    l_clone.quit();

                    glib::ControlFlow::Break
                }
                _ => glib::ControlFlow::Continue,
            }
        })
        .expect("Failed to add bus watch");

    pipeline.set_state(gst::State::Playing).unwrap();
    l.run();

    pipeline.set_state(gst::State::Null).unwrap();
}
