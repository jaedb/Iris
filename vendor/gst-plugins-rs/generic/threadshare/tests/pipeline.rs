// Copyright (C) 2019 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use std::sync::LazyLock;

use std::sync::mpsc;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-test",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing test"),
    )
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare pipeline test");
    });
}

#[test]
#[cfg(not(windows))]
fn multiple_contexts_queue() {
    use std::net;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::mpsc;

    init();

    const CONTEXT_NB: u32 = 2;
    const SRC_NB: u16 = 4;
    const CONTEXT_WAIT: u32 = 1;
    const BUFFER_NB: u32 = 3;
    const FIRST_PORT: u16 = 40000;

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();

    let (sender, receiver) = mpsc::channel();

    for i in 0..SRC_NB {
        let src = gst::ElementFactory::make("ts-udpsrc")
            .name(format!("src-{i}").as_str())
            .property("context", format!("context-{}", (i as u32) % CONTEXT_NB))
            .property("context-wait", CONTEXT_WAIT)
            .property("port", (FIRST_PORT + i) as i32)
            .build()
            .unwrap();

        let queue = gst::ElementFactory::make("ts-queue")
            .name(format!("queue-{i}").as_str())
            .property("context", format!("context-{}", (i as u32) % CONTEXT_NB))
            .property("context-wait", CONTEXT_WAIT)
            .build()
            .unwrap();

        let sink = gst_app::AppSink::builder()
            .name(format!("sink-{i}").as_str())
            .sync(false)
            .async_(false)
            .build();

        pipeline
            .add_many([&src, &queue, sink.upcast_ref()])
            .unwrap();
        gst::Element::link_many([&src, &queue, sink.upcast_ref()]).unwrap();

        let sender_clone = sender.clone();
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let _sample = appsink.pull_sample().unwrap();

                    sender_clone.send(()).unwrap();
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    let pipeline_clone = pipeline.clone();
    let l_clone = l.clone();
    let mut test_scenario = Some(move || {
        let buffer = [0; 160];
        let socket = net::UdpSocket::bind("0.0.0.0:0").unwrap();

        let ipaddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let destinations = (FIRST_PORT..(FIRST_PORT + SRC_NB))
            .map(|port| SocketAddr::new(ipaddr, port))
            .collect::<Vec<_>>();

        for _ in 0..BUFFER_NB {
            for dest in &destinations {
                gst::debug!(CAT, "multiple_contexts_queue: sending buffer to {:?}", dest);
                socket.send_to(&buffer, dest).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(CONTEXT_WAIT as u64));
            }
        }

        gst::debug!(
            CAT,
            "multiple_contexts_queue: waiting for all buffers notifications"
        );
        for _ in 0..(BUFFER_NB * (SRC_NB as u32)) {
            receiver.recv().unwrap();
        }

        pipeline_clone.set_state(gst::State::Null).unwrap();
        l_clone.quit();
    });

    let bus = pipeline.bus().unwrap();
    let l_clone = l.clone();
    let _bus_watch = bus
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::StateChanged(state_changed) => {
                    if let Some(source) = state_changed.src() {
                        if source.type_() == gst::Pipeline::static_type()
                            && state_changed.old() == gst::State::Paused
                            && state_changed.current() == gst::State::Playing
                        {
                            if let Some(test_scenario) = test_scenario.take() {
                                std::thread::spawn(test_scenario);
                            }
                        }
                    }
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "multiple_contexts_queue: Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::debug!(CAT, "Starting main loop for multiple_contexts_queue...");
    l.run();
    gst::debug!(CAT, "Stopping main loop for multiple_contexts_queue...");
}

#[test]
#[cfg(not(windows))]
fn multiple_contexts_proxy() {
    use std::net;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    init();

    const CONTEXT_NB: u32 = 2;
    const SRC_NB: u16 = 4;
    const CONTEXT_WAIT: u32 = 1;
    const BUFFER_NB: u32 = 3;
    // Don't overlap with `multiple_contexts_queue`
    const OFFSET: u16 = 10;
    const FIRST_PORT: u16 = 40000 + OFFSET;

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();

    let (sender, receiver) = mpsc::channel();

    for i in 0..SRC_NB {
        let pipeline_index = i + OFFSET;

        let src = gst::ElementFactory::make("ts-udpsrc")
            .name(format!("src-{pipeline_index}").as_str())
            .property("context", format!("context-{}", (i as u32) % CONTEXT_NB))
            .property("context-wait", CONTEXT_WAIT)
            .property("port", (FIRST_PORT + i) as i32)
            .build()
            .unwrap();

        let proxysink = gst::ElementFactory::make("ts-proxysink")
            .name(format!("proxysink-{pipeline_index}").as_str())
            .property("proxy-context", format!("proxy-{pipeline_index}"))
            .build()
            .unwrap();

        let proxysrc = gst::ElementFactory::make("ts-proxysrc")
            .name(format!("proxysrc-{pipeline_index}").as_str())
            .property(
                "context",
                format!("context-{}", (pipeline_index as u32) % CONTEXT_NB),
            )
            .property("proxy-context", format!("proxy-{pipeline_index}"))
            .build()
            .unwrap();

        let sink = gst_app::AppSink::builder()
            .name(format!("sink-{pipeline_index}").as_str())
            .sync(false)
            .async_(false)
            .build();

        pipeline
            .add_many([&src, &proxysink, &proxysrc, sink.upcast_ref()])
            .unwrap();
        src.link(&proxysink).unwrap();
        proxysrc.link(&sink).unwrap();

        let sender_clone = sender.clone();
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let _sample = appsink.pull_sample().unwrap();

                    sender_clone.send(()).unwrap();
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    let pipeline_clone = pipeline.clone();
    let l_clone = l.clone();
    let mut test_scenario = Some(move || {
        let buffer = [0; 160];
        let socket = net::UdpSocket::bind("0.0.0.0:0").unwrap();

        let ipaddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let destinations = (FIRST_PORT..(FIRST_PORT + SRC_NB))
            .map(|port| SocketAddr::new(ipaddr, port))
            .collect::<Vec<_>>();

        for _ in 0..BUFFER_NB {
            for dest in &destinations {
                gst::debug!(CAT, "multiple_contexts_proxy: sending buffer to {:?}", dest);
                socket.send_to(&buffer, dest).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(CONTEXT_WAIT as u64));
            }
        }

        gst::debug!(
            CAT,
            "multiple_contexts_proxy: waiting for all buffers notifications"
        );
        for _ in 0..(BUFFER_NB * (SRC_NB as u32)) {
            receiver.recv().unwrap();
        }

        pipeline_clone.set_state(gst::State::Null).unwrap();
        l_clone.quit();
    });

    let bus = pipeline.bus().unwrap();
    let l_clone = l.clone();
    let _bus_watch = bus
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::StateChanged(state_changed) => {
                    if let Some(source) = state_changed.src() {
                        if source.type_() == gst::Pipeline::static_type()
                            && state_changed.old() == gst::State::Paused
                            && state_changed.current() == gst::State::Playing
                        {
                            if let Some(test_scenario) = test_scenario.take() {
                                std::thread::spawn(test_scenario);
                            }
                        }
                    }
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "multiple_contexts_proxy: Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::debug!(CAT, "Starting main loop for multiple_contexts_proxy...");
    l.run();
    gst::debug!(CAT, "Stopping main loop for multiple_contexts_proxy...");
}

#[test]
fn eos() {
    const CONTEXT: &str = "test_eos";

    init();

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();

    let caps = gst::Caps::builder("foo/bar").build();

    let src = gst::ElementFactory::make("ts-appsrc")
        .name("src-eos")
        .property("caps", &caps)
        .property("do-timestamp", true)
        .property("context", CONTEXT)
        .build()
        .unwrap();

    let queue = gst::ElementFactory::make("ts-queue")
        .name("queue-eos")
        .property("context", CONTEXT)
        .build()
        .unwrap();

    let sink = gst_app::AppSink::builder()
        .name("sink-eos")
        .sync(false)
        .async_(false)
        .build();

    pipeline
        .add_many([&src, &queue, sink.upcast_ref()])
        .unwrap();
    gst::Element::link_many([&src, &queue, sink.upcast_ref()]).unwrap();

    let (sample_notifier, sample_notif_rcv) = mpsc::channel();
    let (eos_notifier, eos_notif_rcv) = mpsc::channel();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                gst::debug!(CAT, obj = appsink, "eos: pulling sample");
                let _ = appsink.pull_sample().unwrap();

                sample_notifier.send(()).unwrap();

                Ok(gst::FlowSuccess::Ok)
            })
            .eos(move |_appsink| eos_notifier.send(()).unwrap())
            .build(),
    );

    fn push_buffer(src: &gst::Element) -> bool {
        gst::debug!(CAT, obj = src, "eos: pushing buffer");
        src.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![0; 1024])])
    }

    let pipeline_clone = pipeline.clone();
    let l_clone = l.clone();
    let mut scenario = Some(move || {
        // Initialize the dataflow
        assert!(push_buffer(&src));

        sample_notif_rcv.recv().unwrap();

        assert!(src.emit_by_name::<bool>("end-of-stream", &[]));

        eos_notif_rcv.recv().unwrap();

        // FIXME not ideal, but better than previous approach.
        // I think the "end-of-stream" signal should block
        // until the **src** element has actually reached EOS
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if !push_buffer(&src) {
                break;
            }
        }

        pipeline_clone.set_state(gst::State::Null).unwrap();
        l_clone.quit();
    });

    let l_clone = l.clone();
    let _bus_watch = pipeline
        .bus()
        .unwrap()
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::StateChanged(state_changed) => {
                    if let Some(source) = state_changed.src() {
                        if source.type_() != gst::Pipeline::static_type() {
                            return glib::ControlFlow::Continue;
                        }
                        if state_changed.old() == gst::State::Paused
                            && state_changed.current() == gst::State::Playing
                        {
                            if let Some(scenario) = scenario.take() {
                                std::thread::spawn(scenario);
                            }
                        }
                    }
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "eos: Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::debug!(CAT, "Starting main loop for eos...");
    l.run();
    gst::debug!(CAT, "Stopping main loop for eos...");
}

#[test]
fn premature_shutdown() {
    init();

    const APPSRC_CONTEXT_WAIT: u32 = 0;
    const QUEUE_CONTEXT_WAIT: u32 = 1;
    const QUEUE_ITEMS_CAPACITY: u32 = 1;

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();

    let caps = gst::Caps::builder("foo/bar").build();

    let src = gst::ElementFactory::make("ts-appsrc")
        .name("src-ps")
        .property("caps", &caps)
        .property("do-timestamp", true)
        .property("context", "appsrc-context")
        .property("context-wait", APPSRC_CONTEXT_WAIT)
        .build()
        .unwrap();

    let queue = gst::ElementFactory::make("ts-queue")
        .name("queue-ps")
        .property("context", "queue-context")
        .property("context-wait", QUEUE_CONTEXT_WAIT)
        .property("max-size-buffers", QUEUE_ITEMS_CAPACITY)
        .build()
        .unwrap();

    let sink = gst_app::AppSink::builder()
        .name("sink-ps")
        .sync(false)
        .async_(false)
        .build();

    pipeline
        .add_many([&src, &queue, sink.upcast_ref()])
        .unwrap();
    gst::Element::link_many([&src, &queue, sink.upcast_ref()]).unwrap();

    let (appsink_sender, appsink_receiver) = mpsc::channel();

    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                gst::debug!(CAT, obj = appsink, "premature_shutdown: pulling sample");
                let _sample = appsink.pull_sample().unwrap();

                appsink_sender.send(()).unwrap();

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    fn push_buffer(src: &gst::Element, intent: &str) -> bool {
        gst::debug!(
            CAT,
            obj = src,
            "premature_shutdown: pushing buffer {}",
            intent
        );
        src.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![0; 1024])])
    }

    let pipeline_clone = pipeline.clone();
    let l_clone = l.clone();
    let mut scenario = Some(move || {
        gst::debug!(CAT, "premature_shutdown: STEP 1: Playing");
        // Initialize the dataflow
        assert!(push_buffer(&src, "(initial)"));

        // Wait for the buffer to reach AppSink
        appsink_receiver.recv().unwrap();
        assert_eq!(
            appsink_receiver.try_recv().unwrap_err(),
            mpsc::TryRecvError::Empty
        );

        assert!(push_buffer(&src, "before Playing -> Paused"));

        gst::debug!(CAT, "premature_shutdown: STEP 2: Playing -> Paused");
        pipeline_clone.set_state(gst::State::Paused).unwrap();

        gst::debug!(CAT, "premature_shutdown: STEP 3: Paused -> Playing");
        pipeline_clone.set_state(gst::State::Playing).unwrap();

        gst::debug!(CAT, "premature_shutdown: Playing again");

        gst::debug!(CAT, "Waiting for buffer sent before Playing -> Paused");
        appsink_receiver.recv().unwrap();

        assert!(push_buffer(&src, "after Paused -> Playing"));
        gst::debug!(CAT, "Waiting for buffer sent after Paused -> Playing");
        appsink_receiver.recv().unwrap();

        // Fill up the (dataqueue) and abruptly shutdown
        assert!(push_buffer(&src, "filling 1"));
        assert!(push_buffer(&src, "filling 2"));

        gst::debug!(CAT, "premature_shutdown: STEP 4: Playing -> Null");

        pipeline_clone.set_state(gst::State::Null).unwrap();

        assert!(!push_buffer(&src, "after Null"));

        l_clone.quit();
    });

    let l_clone = l.clone();
    let _bus_watch = pipeline
        .bus()
        .unwrap()
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::StateChanged(state_changed) => {
                    if let Some(source) = state_changed.src() {
                        if source.type_() != gst::Pipeline::static_type() {
                            return glib::ControlFlow::Continue;
                        }
                        if state_changed.old() == gst::State::Paused
                            && state_changed.current() == gst::State::Playing
                        {
                            if let Some(scenario) = scenario.take() {
                                std::thread::spawn(scenario);
                            }
                        }
                    }
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "premature_shutdown: Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::debug!(CAT, "Starting main loop for premature_shutdown...");
    l.run();
    gst::debug!(CAT, "Stopped main loop for premature_shutdown...");
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/250
#[ignore]
fn socket_play_null_play() {
    use gio::{
        prelude::SocketExt, InetAddress, InetSocketAddress, SocketFamily, SocketProtocol,
        SocketType,
    };

    const TEST: &str = "socket_play_null_play";

    init();

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();

    let socket = gio::Socket::new(
        SocketFamily::Ipv4,
        SocketType::Datagram,
        SocketProtocol::Udp,
    )
    .unwrap();
    socket
        .bind(
            &InetSocketAddress::new(&InetAddress::from_string("127.0.0.1").unwrap(), 4500),
            true,
        )
        .unwrap();

    let sink = gst::ElementFactory::make("ts-udpsink")
        .name(format!("sink-{TEST}").as_str())
        .property("socket", &socket)
        .property("context", TEST)
        .property("context-wait", 20u32)
        .build()
        .unwrap();

    pipeline.add(&sink).unwrap();

    let pipeline_clone = pipeline.clone();
    let l_clone = l.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut scenario = Some(move || {
        gst::debug!(CAT, "{}: to Null", TEST);
        pipeline_clone.set_state(gst::State::Null).unwrap();

        gst::debug!(CAT, "{}: Play again", TEST);
        let res = pipeline_clone.set_state(gst::State::Playing);
        l_clone.quit();
        sender.send(res).unwrap();
    });

    let l_clone = l.clone();
    let _bus_watch = pipeline
        .bus()
        .unwrap()
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::StateChanged(state_changed) => {
                    if let Some(source) = state_changed.src() {
                        if source.type_() != gst::Pipeline::static_type() {
                            return glib::ControlFlow::Continue;
                        }
                        if state_changed.old() == gst::State::Paused
                            && state_changed.current() == gst::State::Playing
                        {
                            if let Some(scenario) = scenario.take() {
                                std::thread::spawn(scenario);
                            }
                        }
                    }
                }
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "{}: Error from {:?}: {} ({:?})",
                        TEST,
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    l_clone.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap();

    gst::debug!(CAT, "{}: Playing", TEST);
    pipeline.set_state(gst::State::Playing).unwrap();

    gst::debug!(CAT, "Starting main loop for {}...", TEST);
    l.run();
    gst::debug!(CAT, "Stopping main loop for {}...", TEST);
    let _ = pipeline.set_state(gst::State::Null);

    if let Err(err) = receiver.recv().unwrap() {
        panic!("{TEST}: {err}");
    }
}
