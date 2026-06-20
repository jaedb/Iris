// Copyright (C) 2025 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use gst::prelude::*;

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare inter test");
    });
}

#[test]
fn one_to_one_down_first() {
    init();

    let pipe_up = gst::Pipeline::with_name("upstream::one_to_one_down_first");
    let audiotestsrc = gst::ElementFactory::make("audiotestsrc")
        .name("testsrc::one_to_one_down_first")
        .property("num-buffers", 20i32)
        .property("is-live", true)
        .build()
        .unwrap();
    let intersink = gst::ElementFactory::make("ts-intersink")
        .name("intersink::one_to_one_down_first")
        .property("inter-context", "inter::one_to_one_down_first")
        .build()
        .unwrap();

    let pipe_down = gst::Pipeline::with_name("downstream::one_to_one_down_first");
    let intersrc = gst::ElementFactory::make("ts-intersrc")
        .name("intersrc::one_to_one_down_first")
        .property("inter-context", "inter::one_to_one_down_first")
        .property("context", "inter::test")
        .property("context-wait", 20u32)
        .build()
        .unwrap();
    let appsink = gst_app::AppSink::builder()
        .name("appsink::one_to_one_down_first")
        .build();

    let upstream_elems = [&audiotestsrc, &intersink];
    pipe_up.add_many(upstream_elems).unwrap();
    gst::Element::link_many(upstream_elems).unwrap();

    pipe_down
        .add_many([&intersrc, appsink.upcast_ref()])
        .unwrap();
    intersrc.link(&appsink).unwrap();

    pipe_up.set_base_time(gst::ClockTime::ZERO);
    pipe_up.set_start_time(gst::ClockTime::NONE);
    pipe_down.set_base_time(gst::ClockTime::ZERO);
    pipe_down.set_start_time(gst::ClockTime::NONE);

    let samples = Arc::new(AtomicU32::new(0));
    let (eos_tx, mut eos_rx) = oneshot::channel::<()>();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample({
                let samples = samples.clone();
                move |appsink| {
                    let _ = appsink.pull_sample().unwrap();
                    samples.fetch_add(1, Ordering::SeqCst);

                    Ok(gst::FlowSuccess::Ok)
                }
            })
            .eos({
                let mut eos_tx = Some(eos_tx);
                move |_| eos_tx.take().unwrap().send(()).unwrap()
            })
            .build(),
    );

    // Starting downstream first => we will get all buffers
    pipe_down.set_state(gst::State::Playing).unwrap();
    pipe_up.set_state(gst::State::Playing).unwrap();

    let mut got_eos_evt = false;
    let mut got_eos_msg = false;

    futures::executor::block_on(async {
        use gst::MessageView::*;

        let mut bus_up_stream = pipe_up.bus().unwrap().stream();
        let mut bus_down_stream = pipe_down.bus().unwrap().stream();

        loop {
            futures::select! {
                _ = eos_rx => {
                    println!("inter::one_to_one_down_first got eos notif");
                    got_eos_evt = true;
                    if got_eos_msg {
                        break;
                    }
                }
                msg = bus_down_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down.recalculate_latency();
                        }
                        Eos(_) => {
                            println!("inter::one_to_one_down_first got eos msg");
                            got_eos_msg = true;
                            if got_eos_evt {
                                break;
                            }
                        }
                        Error(err) => unreachable!("inter::one_to_one_down_first {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_up_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_one_down_first {err:?}"),
                        _ => (),
                    }
                }
            };
        }
    });

    assert_eq!(samples.load(Ordering::SeqCst), 20);

    pipe_up.set_state(gst::State::Null).unwrap();
    pipe_down.set_state(gst::State::Null).unwrap();
}

#[test]
fn one_to_one_up_first() {
    init();

    let pipe_up = gst::Pipeline::with_name("upstream::one_to_one_up_first");
    let audiotestsrc = gst::ElementFactory::make("audiotestsrc")
        .name("testsrc::one_to_one_up_first")
        .property("is-live", true)
        .build()
        .unwrap();
    let opusenc = gst::ElementFactory::make("opusenc")
        .name("opusenc::one_to_one_down_first")
        .build()
        .unwrap();
    let intersink = gst::ElementFactory::make("ts-intersink")
        .name("intersink::one_to_one_up_first")
        .property("inter-context", "inter::one_to_one_up_first")
        .build()
        .unwrap();

    let pipe_down = gst::Pipeline::with_name("downstream::one_to_one_up_first");
    let intersrc = gst::ElementFactory::make("ts-intersrc")
        .name("intersrc::one_to_one_up_first")
        .property("inter-context", "inter::one_to_one_up_first")
        .property("context", "inter::one_to_one_up_first")
        .property("context-wait", 20u32)
        .build()
        .unwrap();
    let appsink = gst_app::AppSink::builder()
        .name("appsink::one_to_one_up_first")
        .build();

    let upstream_elems = [&audiotestsrc, &opusenc, &intersink];
    pipe_up.add_many(upstream_elems).unwrap();
    gst::Element::link_many(upstream_elems).unwrap();

    let downstream_elems = [&intersrc, appsink.upcast_ref()];
    pipe_down.add_many(downstream_elems).unwrap();
    gst::Element::link_many(downstream_elems).unwrap();

    pipe_up.set_base_time(gst::ClockTime::ZERO);
    pipe_up.set_start_time(gst::ClockTime::NONE);
    pipe_down.set_base_time(gst::ClockTime::ZERO);
    pipe_down.set_start_time(gst::ClockTime::NONE);

    let got_latency_evt = Arc::new(AtomicBool::new(false));
    let (n_buf_tx, mut n_buf_rx) = oneshot::channel::<()>();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample({
                let got_latency_evt = got_latency_evt.clone();
                let mut samples = 0;
                let mut n_buf_tx = Some(n_buf_tx);
                move |appsink| {
                    let _ = appsink.pull_sample().unwrap();
                    if got_latency_evt.load(Ordering::SeqCst) {
                        samples += 1;
                        if samples == 10 {
                            n_buf_tx.take().unwrap().send(()).unwrap();
                        }
                    }

                    Ok(gst::FlowSuccess::Ok)
                }
            })
            .build(),
    );

    appsink.static_pad("sink").unwrap().add_probe(
        gst::PadProbeType::EVENT_UPSTREAM,
        move |_, info| {
            let Some(gst::PadProbeData::Event(ref evt)) = info.data else {
                unreachable!();
            };

            if let gst::EventView::Latency(evt) = evt.view() {
                if evt.latency() > gst::ClockTime::ZERO {
                    got_latency_evt.store(true, Ordering::SeqCst);
                }
            }
            gst::PadProbeReturn::Ok
        },
    );

    // Starting upstream first
    pipe_up.set_state(gst::State::Playing).unwrap();
    pipe_down.set_state(gst::State::Playing).unwrap();

    futures::executor::block_on(async {
        use gst::MessageView::*;

        let mut bus_up_stream = pipe_up.bus().unwrap().stream();
        let mut bus_down_stream = pipe_down.bus().unwrap().stream();

        loop {
            futures::select! {
                _ = n_buf_rx => {
                    println!("inter::one_to_one_up_first got n_buf notif");
                    break;
                }
                msg = bus_up_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_one_up_first {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_down_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_one_up_first {err:?}"),
                        _ => (),
                    }
                }
            };
        }
    });

    let mut q = gst::query::Latency::new();
    assert!(pipe_up.query(&mut q));
    // Upstream latency: testsrc (20ms) + opusenc (20ms)
    let up_latency = q.result().1;
    assert!(
        up_latency >= 40.mseconds(),
        "unexpected upstream latency {up_latency}"
    );

    let mut q = gst::query::Latency::new();
    assert!(pipe_down.query(&mut q));
    // Downstream latency: upstream latency + intersrc (context-wait 20ms) + appsink
    let down_latency = q.result().1;
    assert!(
        down_latency > up_latency + 20.mseconds(),
        "unexpected downstream latency {down_latency}"
    );

    pipe_down.set_state(gst::State::Null).unwrap();
    pipe_up.set_state(gst::State::Null).unwrap();
}

#[test]
fn one_to_many_up_first() {
    use gstthreadshare::runtime::executor as ts_executor;

    init();

    fn build_pipe_down(
        i: u32,
        num_buf: impl Into<Option<u32>>,
    ) -> (
        gst::Pipeline,
        Arc<AtomicU32>,
        Option<futures::channel::oneshot::Receiver<()>>,
    ) {
        let num_buf = num_buf.into();

        let pipe_down =
            gst::Pipeline::with_name(&format!("downstream-{i:02}::one_to_many_up_first"));
        let intersrc = gst::ElementFactory::make("ts-intersrc")
            .name(format!("intersrc-{i:02}::one_to_many_up_first"))
            .property("inter-context", "inter::one_to_many_up_first")
            .property("context", "inter::one_to_many_up_first")
            .property("context-wait", 20u32)
            .build()
            .unwrap();
        let appsink = gst_app::AppSink::builder()
            .name(format!("appsink-{i:02}::one_to_many_up_first"))
            .sync(false)
            .build();

        pipe_down
            .add_many([&intersrc, appsink.upcast_ref()])
            .unwrap();
        intersrc.link(&appsink).unwrap();

        pipe_down.set_base_time(gst::ClockTime::ZERO);
        pipe_down.set_start_time(gst::ClockTime::NONE);

        let samples = Arc::new(AtomicU32::new(0));
        let (mut n_buf_tx, n_buf_rx) = if num_buf.is_some() {
            let (n_buf_tx, n_buf_rx) = oneshot::channel::<()>();
            (Some(n_buf_tx), Some(n_buf_rx))
        } else {
            (None, None)
        };
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample({
                    let samples = samples.clone();
                    move |appsink| {
                        let _ = appsink.pull_sample().unwrap();
                        let cur = samples.fetch_add(1, Ordering::SeqCst);
                        if let Some(num_buf) = num_buf {
                            if cur + 1 == num_buf {
                                n_buf_tx.take().unwrap().send(()).unwrap();
                            }
                        }
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .build(),
        );

        (pipe_down, samples, n_buf_rx)
    }

    let pipe_up = gst::Pipeline::with_name("upstream::one_to_many_up_first");
    let audiotestsrc = gst::ElementFactory::make("audiotestsrc")
        .name("testsrc::one_to_many_up_first")
        .property("is-live", true)
        .build()
        .unwrap();
    let intersink = gst::ElementFactory::make("ts-intersink")
        .name("intersink::one_to_many_up_first")
        .property("inter-context", "inter::one_to_many_up_first")
        .build()
        .unwrap();

    let upstream_elems = [&audiotestsrc, &intersink];
    pipe_up.add_many(upstream_elems).unwrap();
    gst::Element::link_many(upstream_elems).unwrap();

    pipe_up.set_base_time(gst::ClockTime::ZERO);
    pipe_up.set_start_time(gst::ClockTime::NONE);

    // Starting upstream first
    pipe_up.set_state(gst::State::Playing).unwrap();

    let (pipe_down_1, samples_1, n_buf_rx_1) = build_pipe_down(1, 20);
    let mut n_buf_rx_1 = n_buf_rx_1.unwrap();
    pipe_down_1.set_state(gst::State::Playing).unwrap();

    let (pipe_down_2, samples_2, n_buf_rx_2) = build_pipe_down(2, 20);
    let n_buf_rx_2 = n_buf_rx_2.unwrap();
    pipe_down_2.set_state(gst::State::Playing).unwrap();

    futures::executor::block_on(async {
        use gst::MessageView::*;

        let mut bus_down_stream_1 = pipe_down_1.bus().unwrap().stream();
        let mut bus_down_stream_2 = pipe_down_2.bus().unwrap().stream();
        let mut bus_up_stream = pipe_up.bus().unwrap().stream();

        loop {
            futures::select! {
                _ = n_buf_rx_1 => {
                    println!("inter::one_to_many_up_first got n_buf notif");
                    break;
                }
                msg_down_1 = bus_down_stream_1.next() => {
                    let Some(msg) = msg_down_1 else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down_1.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_many_up_first {err:?}"),
                        _ => (),
                    }
                }
                msg_down_2 = bus_down_stream_2.next() => {
                    let Some(msg) = msg_down_2 else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down_2.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_many_up_first {err:?}"),
                        _ => (),
                    }
                }
                msg_up = bus_up_stream.next() => {
                    let Some(msg) = msg_up else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::one_to_many_up_first {err:?}"),
                        _ => (),
                    }
                }
            }
        }
    });

    pipe_down_1.set_state(gst::State::Null).unwrap();
    // pipe_down_1 was set to stop after 20 buffers (but more might have go through before we stopped)
    assert!(samples_1.load(Ordering::SeqCst) >= 20);

    // Waiting for pipe_down_2 to handle its buffers too
    futures::executor::block_on(n_buf_rx_2).unwrap();
    pipe_down_2.set_state(gst::State::Null).unwrap();
    assert!(samples_2.load(Ordering::SeqCst) >= 20);

    pipe_up.set_state(gst::State::Null).unwrap();
    println!("up null");

    let (pipe_down_3, samples_3, _) = build_pipe_down(3, None);
    pipe_down_3.set_state(gst::State::Playing).unwrap();

    let bus_down_3 = pipe_down_3.bus().unwrap();

    ts_executor::block_on(async move {
        let mut bus_down_stream_3 = bus_down_3.stream();

        let mut timer = ts_executor::timer::delay_for(Duration::from_millis(200)).fuse();

        loop {
            futures::select! {
                _ = timer => {
                    break;
                }
                msg_down_3 = bus_down_stream_3.next() => {
                    let Some(msg) = msg_down_3 else { continue };
                    if let gst::MessageView::Error(err) = msg.view() {
                        unreachable!("inter::one_to_many_up_first {err:?}");
                    }
                }
            }
        }
    });

    println!("down3 to null");
    pipe_down_3.set_state(gst::State::Null).unwrap();

    // pipe_down_3 was set to Playing after pipe_up was shutdown
    assert!(samples_3.load(Ordering::SeqCst) == 0);
}

#[test]
fn changing_inter_ctx() {
    init();

    let pipe_up1 = gst::Pipeline::with_name("upstream1::changing_inter_ctx");
    let src1 = gst::ElementFactory::make("audiotestsrc")
        .name("testsrc1::changing_inter_ctx")
        .property("is-live", true)
        .build()
        .unwrap();
    let capsfilter1 = gst::ElementFactory::make("capsfilter")
        .name("capsfilter1::one_to_one_down_first")
        .property(
            "caps",
            gst::Caps::builder("audio/x-raw")
                .field("channels", 1i32)
                .build(),
        )
        .build()
        .unwrap();
    let intersink1 = gst::ElementFactory::make("ts-intersink")
        .name("intersink1::changing_inter_ctx")
        .property("inter-context", "inter1::changing_inter_ctx")
        .build()
        .unwrap();

    let upstream1_elems = [&src1, &capsfilter1, &intersink1];
    pipe_up1.add_many(upstream1_elems).unwrap();
    gst::Element::link_many(upstream1_elems).unwrap();

    let pipe_up2 = gst::Pipeline::with_name("upstream2::changing_inter_ctx");
    let src2 = gst::ElementFactory::make("audiotestsrc")
        .name("testsrc2::changing_inter_ctx")
        .property("is-live", true)
        .build()
        .unwrap();
    let capsfilter2 = gst::ElementFactory::make("capsfilter")
        .name("capsfilter2::one_to_one_down_first")
        .property(
            "caps",
            gst::Caps::builder("audio/x-raw")
                .field("channels", 2i32)
                .build(),
        )
        .build()
        .unwrap();
    let intersink2 = gst::ElementFactory::make("ts-intersink")
        .name("intersink2::changing_inter_ctx")
        .property("inter-context", "inter2::changing_inter_ctx")
        .build()
        .unwrap();

    let upstream2_elems = [&src2, &capsfilter2, &intersink2];
    pipe_up2.add_many(upstream2_elems).unwrap();
    gst::Element::link_many(upstream2_elems).unwrap();

    let pipe_down = gst::Pipeline::with_name("downstream::changing_inter_ctx");
    let intersrc = gst::ElementFactory::make("ts-intersrc")
        .name("intersrc::changing_inter_ctx")
        .property("context", "inter::changing_inter_ctx")
        .property("context-wait", 20u32)
        .build()
        .unwrap();
    let appsink = gst_app::AppSink::builder()
        .name("appsink::changing_inter_ctx")
        .build();

    let downstream_elems = [&intersrc, appsink.upcast_ref()];
    pipe_down.add_many(downstream_elems).unwrap();
    gst::Element::link_many(downstream_elems).unwrap();

    pipe_up1.set_base_time(gst::ClockTime::ZERO);
    pipe_up1.set_start_time(gst::ClockTime::NONE);
    pipe_up2.set_base_time(gst::ClockTime::ZERO);
    pipe_up2.set_start_time(gst::ClockTime::NONE);
    pipe_down.set_base_time(gst::ClockTime::ZERO);
    pipe_down.set_start_time(gst::ClockTime::NONE);

    let (mut caps_tx, mut caps_rx) = mpsc::channel(1);
    let (mut n_buffers_tx, mut n_buffers_rx) = mpsc::channel(1);
    let starting = Arc::new(AtomicBool::new(true));
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample({
                let starting = starting.clone();
                let mut cur_caps = None;
                let mut samples = 0;
                move |appsink| {
                    if starting.fetch_and(false, Ordering::SeqCst) {
                        cur_caps = None;
                        samples = 0;
                    }

                    let sample = appsink.pull_sample().unwrap();
                    if let Some(caps) = sample.caps() {
                        if cur_caps.as_ref().is_none() {
                            cur_caps = Some(caps.to_owned());
                            caps_tx.try_send(caps.to_owned()).unwrap();
                        }
                    }

                    samples += 1;

                    if samples == 10 {
                        n_buffers_tx.try_send(()).unwrap();
                    }

                    Ok(gst::FlowSuccess::Ok)
                }
            })
            .new_event(move |appsink| {
                let obj = appsink.pull_object().unwrap();
                if let Some(event) = obj.downcast_ref::<gst::Event>() {
                    if let gst::EventView::FlushStop(_) = event.view() {
                        println!("inter::changing_inter_ctx: appsink got FlushStop");
                        starting.store(true, Ordering::SeqCst);
                    }
                }
                // let basesink handle the event
                false
            })
            .build(),
    );

    // Starting upstream first
    pipe_up1.set_state(gst::State::Playing).unwrap();
    pipe_up2.set_state(gst::State::Playing).unwrap();

    // Connect downstream to pipe_up1 initially
    intersrc.set_property("inter-context", "inter1::changing_inter_ctx");
    pipe_down.set_state(gst::State::Playing).unwrap();

    let mut bus_up1_stream = pipe_up1.bus().unwrap().stream();
    let mut bus_up2_stream = pipe_up1.bus().unwrap().stream();
    let mut bus_down_stream = pipe_down.bus().unwrap().stream();

    futures::executor::block_on(async {
        use gst::MessageView::*;

        loop {
            futures::select! {
                caps = caps_rx.next() => {
                    println!("inter::changing_inter_ctx: caps 1: {caps:?}");
                    if let Some(caps) = caps {
                        let s = caps.structure(0).unwrap();
                        assert_eq!(s.get::<i32>("channels").unwrap(), 1);
                    }
                }
                _ = n_buffers_rx.next() => {
                    println!("inter::changing_inter_ctx: got n buffers 1");
                    break;
                }
                msg = bus_up1_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up1.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::changing_inter_ctx {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_up2_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up2.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::changing_inter_ctx {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_down_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::changing_inter_ctx {err:?}"),
                        _ => (),
                    }
                }
            };
        }
    });

    println!("inter::changing_inter_ctx: changing now");
    intersrc.set_property("inter-context", "inter2::changing_inter_ctx");

    futures::executor::block_on(async {
        use gst::MessageView::*;

        loop {
            println!("changing_inter_ctx: iter 2");

            futures::select! {
                caps = caps_rx.next() => {
                    println!("inter::changing_inter_ctx: caps 2: {caps:?}");
                    if let Some(caps) = caps {
                        let s = caps.structure(0).unwrap();
                        assert_eq!(s.get::<i32>("channels").unwrap(), 2);
                    }
                }
                _ = n_buffers_rx.next() => {
                    println!("inter::changing_inter_ctx: got n buffers 2");
                    break;
                }
                msg = bus_up2_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up2.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::changing_inter_ctx {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_down_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::changing_inter_ctx {err:?}"),
                        _ => (),
                    }
                }
            };
        }
    });

    println!("inter::changing_inter_ctx: stopping");
    pipe_down.set_state(gst::State::Null).unwrap();
    pipe_up2.set_state(gst::State::Null).unwrap();
    pipe_up1.set_state(gst::State::Null).unwrap();
}
