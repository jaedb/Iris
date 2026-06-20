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

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const THROUGHPUT_PERIOD: Duration = Duration::from_secs(20);

pub static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-benchmark",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing benchmarking receiver"),
    )
});

fn main() {
    gst::init().unwrap();
    // Register the plugins statically:
    // - The executable can be run from anywhere.
    // - No risk of running against a previous version.
    // - `main` can use features that rely on `static`s or `thread_local`
    //   such as `Context::acquire` which otherwise don't point to
    //   the same `static` or `thread_local`, probably because
    //   the shared object uses its owns and the executable, others.
    gstthreadshare::plugin_register_static().unwrap();

    let args = env::args().collect::<Vec<_>>();
    assert!(args.len() > 4);
    let n_streams: u16 = args[1].parse().unwrap();
    let source = &args[2];
    let n_groups: u32 = args[3].parse().unwrap();
    let wait: u32 = args[4].parse().unwrap();

    // Nb buffers to await before stopping.
    let max_buffers: Option<f32> = if args.len() > 5 {
        args[5].parse().ok()
    } else {
        None
    };
    let is_rtp = args.len() > 6 && (args[6] == "rtp");

    let rtp_caps = gst::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", 8i32)
        .field("clock-rate", 8000)
        .field("encoding-name", "PCMA")
        .build();

    let l = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::default();
    let counter = Arc::new(AtomicU64::new(0));

    for i in 0..n_streams {
        let build_context = || format!("context-{}", (i as u32) % n_groups);

        let sink = gst::ElementFactory::make("fakesink")
            .name(format!("sink-{i}").as_str())
            .property("sync", false)
            .property("async", false)
            .property("signal-handoffs", true)
            .build()
            .unwrap();
        sink.connect_closure(
            "handoff",
            true,
            glib::closure!(
                #[strong]
                counter,
                move |_fakesink: &gst::Element, _buffer: &gst::Buffer, _pad: &gst::Pad| {
                    let _ = counter.fetch_add(1, Ordering::SeqCst);
                }
            ),
        );

        let (source, context) = match source.as_str() {
            "udpsrc" => {
                let source = gst::ElementFactory::make("udpsrc")
                    .name(format!("source-{i}").as_str())
                    .property("port", 5004i32 + i as i32)
                    .property("retrieve-sender-address", false)
                    .build()
                    .unwrap();

                (source, None)
            }
            "ts-udpsrc" => {
                let context = build_context();
                let source = gst::ElementFactory::make("ts-udpsrc")
                    .name(format!("source-{i}").as_str())
                    .property("port", 5004i32 + i as i32)
                    .property("context", &context)
                    .property("context-wait", wait)
                    .property_if("caps", &rtp_caps, is_rtp)
                    .build()
                    .unwrap();

                (source, Some(context))
            }
            "tcpclientsrc" => {
                let source = gst::ElementFactory::make("tcpclientsrc")
                    .name(format!("source-{i}").as_str())
                    .property("host", "127.0.0.1")
                    .property("port", 40000i32)
                    .build()
                    .unwrap();

                (source, None)
            }
            "ts-tcpclientsrc" => {
                let context = build_context();
                let source = gst::ElementFactory::make("ts-tcpclientsrc")
                    .name(format!("source-{i}").as_str())
                    .property("host", "127.0.0.1")
                    .property("port", 40000i32)
                    .property("context", &context)
                    .property("context-wait", wait)
                    .build()
                    .unwrap();

                (source, Some(context))
            }
            "tonegeneratesrc" => {
                let source = gst::ElementFactory::make("tonegeneratesrc")
                    .name(format!("source-{i}").as_str())
                    .property("samplesperbuffer", (wait as i32) * 8000 / 1000)
                    .build()
                    .unwrap();

                sink.set_property("sync", true);

                (source, None)
            }
            "ts-tonesrc" => {
                let context = build_context();
                let source = gst::ElementFactory::make("ts-tonesrc")
                    .name(format!("source-{i}").as_str())
                    .property("samples-per-buffer", wait * 8000 / 1000)
                    .property("context", &context)
                    .property("context-wait", wait)
                    .build()
                    .unwrap();

                (source, Some(context))
            }
            _ => unimplemented!(),
        };

        if is_rtp {
            let jb = gst::ElementFactory::make("ts-jitterbuffer")
                .name(format!("jb-{i}").as_str())
                .property("context-wait", wait)
                .property("latency", wait)
                .property_if_some("context", context.as_ref())
                .build()
                .unwrap();

            let elements = &[&source, &jb, &sink];
            pipeline.add_many(elements).unwrap();
            gst::Element::link_many(elements).unwrap();
        } else {
            let elements = &[&source, &sink];
            pipeline.add_many(elements).unwrap();
            gst::Element::link_many(elements).unwrap();
        }
    }

    let bus = pipeline.bus().unwrap();
    let l_clone = l.clone();
    let _bus_watch = bus
        .add_watch(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => l_clone.quit(),
                MessageView::Error(err) => {
                    gst::error!(
                        CAT,
                        "Error from {:?}: {} ({:?})",
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
        .expect("Failed to add bus watch");

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::info!(CAT, "started");

    let l_clone = l.clone();
    thread::spawn(move || {
        let n_streams_f32 = n_streams as f32;

        let mut total_count = 0.0;
        let mut ramp_up_complete_instant: Option<Instant> = None;

        #[cfg(feature = "tuning")]
        let ctx_0 = gstthreadshare::runtime::Context::acquire(
            "context-0",
            Duration::from_millis(wait as u64),
        )
        .unwrap();
        #[cfg(feature = "tuning")]
        let mut parked_init = Duration::ZERO;

        loop {
            total_count += counter.fetch_and(0, Ordering::SeqCst) as f32 / n_streams_f32;
            if let Some(max_buffers) = max_buffers {
                if total_count > max_buffers {
                    gst::info!(CAT, "Stopping");
                    let stopping_instant = Instant::now();
                    pipeline.set_state(gst::State::Ready).unwrap();
                    gst::info!(CAT, "Stopped. Took {:?}", stopping_instant.elapsed());
                    pipeline.set_state(gst::State::Null).unwrap();
                    gst::info!(CAT, "Unprepared");
                    l_clone.quit();
                    break;
                }
            }

            if let Some(init) = ramp_up_complete_instant {
                let elapsed = init.elapsed();
                gst::info!(
                    CAT,
                    "Thrpt: {:>6.2}",
                    total_count * 1_000.0 / elapsed.as_millis() as f32
                );

                #[cfg(feature = "tuning")]
                gst::info!(
                    CAT,
                    "Parked: {:>6.2}%",
                    (ctx_0.parked_duration() - parked_init).as_nanos() as f32 * 100.0
                        / elapsed.as_nanos() as f32
                );
            } else {
                // Ramp up 30s worth of buffers before following parked
                if total_count > 50.0 * 30.0 {
                    total_count = 0.0;
                    ramp_up_complete_instant = Some(Instant::now());
                    #[cfg(feature = "tuning")]
                    {
                        parked_init = ctx_0.parked_duration();
                    }
                }
            }

            thread::sleep(THROUGHPUT_PERIOD);
        }
    });

    l.run();
}
