// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2021 Jan Schmidt <jan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::debug;
use gst::prelude::*;

use std::sync::LazyLock;

const LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(10);

static TEST_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "fallbackswitch-test",
        gst::DebugColorFlags::empty(),
        Some("fallbackswitch test"),
    )
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstfallbackswitch::plugin_register_static().expect("gstfallbackswitch test");
    });
}

macro_rules! assert_fallback_buffer {
    ($buffer:expr, $ts:expr) => {
        assert_eq!($buffer.pts(), $ts);
        assert_eq!($buffer.size(), 160 * 120 * 4);
    };
}

macro_rules! assert_buffer {
    ($buffer:expr, $ts:expr) => {
        assert_eq!($buffer.pts(), $ts);
        assert_eq!($buffer.size(), 320 * 240 * 4);
    };
}

#[test]
fn test_no_fallback_no_drops() {
    let pipeline = setup_pipeline(None, None, None);

    push_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    push_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::SECOND));

    push_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(2.seconds()));

    push_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_no_drops_live() {
    test_no_drops(true);
}

#[test]
fn test_no_drops_not_live() {
    test_no_drops(false);
}

fn test_no_drops(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);

    push_buffer(&pipeline, gst::ClockTime::ZERO);
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    push_fallback_buffer(&pipeline, 1.seconds());
    push_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::SECOND));

    push_buffer(&pipeline, 2.seconds());
    push_fallback_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(2.seconds()));

    // EOS on the fallback should not be required
    push_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_no_drops_but_no_fallback_frames_live() {
    test_no_drops_but_no_fallback_frames(true);
}

#[test]
fn test_no_drops_but_no_fallback_frames_not_live() {
    test_no_drops_but_no_fallback_frames(false);
}

fn test_no_drops_but_no_fallback_frames(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);

    push_buffer(&pipeline, gst::ClockTime::ZERO);
    // +10ms needed here because the immediate timeout will be always at running time 0, but
    // aggregator also adds the latency to it so we end up at 10ms instead.
    set_time(&pipeline, LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    push_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::SECOND));

    push_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(2.seconds()));

    // EOS on the fallback should not be required
    push_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_short_drop_live() {
    test_short_drop(true);
}

#[test]
fn test_short_drop_not_live() {
    test_short_drop(false);
}

fn test_short_drop(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);

    push_buffer(&pipeline, gst::ClockTime::ZERO);
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    // A timeout at 1s will get rid of the fallback buffer
    // but not output anything
    push_fallback_buffer(&pipeline, 1.seconds());
    // Time out the fallback buffer at +10ms
    set_time(&pipeline, 1.seconds() + 10.mseconds());

    push_fallback_buffer(&pipeline, 2.seconds());
    push_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(2.seconds()));

    push_eos(&pipeline);
    push_fallback_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_long_drop_and_eos_live() {
    test_long_drop_and_eos(true);
}

#[test]
fn test_long_drop_and_eos_not_live() {
    test_long_drop_and_eos(false);
}

fn test_long_drop_and_eos(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);

    // Produce the first frame
    push_buffer(&pipeline, gst::ClockTime::ZERO);
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    // Produce a second frame but only from the fallback source
    push_fallback_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + 10.mseconds());

    // Produce a third frame but only from the fallback source
    push_fallback_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + 10.mseconds());

    // Produce a fourth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 3.seconds());
    set_time(&pipeline, 3.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(3.seconds()));

    // Produce a fifth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 4.seconds());
    set_time(&pipeline, 4.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(4.seconds()));

    // Wait for EOS to arrive at appsink
    push_eos(&pipeline);
    push_fallback_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_long_drop_and_recover_live() {
    test_long_drop_and_recover(true);
}

#[test]
fn test_long_drop_and_recover_not_live() {
    test_long_drop_and_recover(false);
}

fn test_long_drop_and_recover(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);
    let switch = pipeline.by_name("switch").unwrap();
    let mainsink = switch.static_pad("sink_0").unwrap();

    // Produce the first frame
    push_buffer(&pipeline, gst::ClockTime::ZERO);
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));
    assert!(mainsink.property::<bool>("is-healthy"));

    // Produce a second frame but only from the fallback source
    push_fallback_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + 10.mseconds());

    // Produce a third frame but only from the fallback source
    push_fallback_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + 10.mseconds());

    // Produce a fourth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 3.seconds());
    set_time(&pipeline, 3.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(3.seconds()));

    // Produce a fifth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 4.seconds());
    set_time(&pipeline, 4.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(4.seconds()));

    // Produce a sixth frame from the normal source, which
    // will make it healthy again
    push_buffer(&pipeline, 5.seconds());
    set_time(&pipeline, 5.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(5.seconds()));
    assert!(mainsink.property::<bool>("is-healthy"));
    drop(mainsink);
    drop(switch);

    // Produce a seventh frame from the normal source but no fallback.
    // This should still be output immediately
    push_buffer(&pipeline, 6.seconds());
    set_time(&pipeline, 6.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(6.seconds()));

    // Produce a eight frame from the normal source
    push_buffer(&pipeline, 7.seconds());
    push_fallback_buffer(&pipeline, 7.seconds());
    set_time(&pipeline, 7.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(7.seconds()));

    // Wait for EOS to arrive at appsink
    push_eos(&pipeline);
    push_fallback_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_initial_timeout_live() {
    test_initial_timeout(true);
}

#[test]
fn test_initial_timeout_not_live() {
    test_initial_timeout(false);
}

fn test_initial_timeout(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, None);

    // Produce the first frame but only from the fallback source
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO);

    // Produce a second frame but only from the fallback source
    push_fallback_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + 10.mseconds());

    // Produce a third frame but only from the fallback source
    push_fallback_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + 10.mseconds());

    // Produce a fourth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 3.seconds());
    set_time(&pipeline, 3.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(3.seconds()));

    // Produce a fifth frame but only from the fallback source
    // This should be output now
    push_fallback_buffer(&pipeline, 4.seconds());
    set_time(&pipeline, 4.seconds() + 10.mseconds());
    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(4.seconds()));

    // Wait for EOS to arrive at appsink
    push_eos(&pipeline);
    push_fallback_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_immediate_fallback_live() {
    test_immediate_fallback(true);
}

#[test]
fn test_immediate_fallback_not_live() {
    test_immediate_fallback(false);
}

fn test_immediate_fallback(live: bool) {
    let pipeline = setup_pipeline(Some(live), Some(true), None);

    // Produce the first frame but only from the fallback source
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO);

    let buffer = pull_buffer(&pipeline);
    assert_fallback_buffer!(buffer, Some(gst::ClockTime::ZERO));

    // Wait for EOS to arrive at appsink
    push_eos(&pipeline);
    push_fallback_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

#[test]
fn test_manual_switch_live() {
    test_manual_switch(true);
}

#[test]
fn test_manual_switch_not_live() {
    test_manual_switch(false);
}

fn test_manual_switch(live: bool) {
    let pipeline = setup_pipeline(Some(live), None, Some(false));
    let switch = pipeline.by_name("switch").unwrap();
    let mainsink = switch.static_pad("sink_0").unwrap();
    let fallbacksink = switch.static_pad("sink_1").unwrap();

    switch.set_property("active-pad", &mainsink);
    push_buffer(&pipeline, gst::ClockTime::ZERO);
    push_fallback_buffer(&pipeline, gst::ClockTime::ZERO);
    set_time(&pipeline, gst::ClockTime::ZERO + LATENCY);
    let buffer = pull_buffer(&pipeline);
    assert_buffer!(buffer, Some(gst::ClockTime::ZERO));

    switch.set_property("active-pad", &fallbacksink);
    push_fallback_buffer(&pipeline, 1.seconds());
    push_buffer(&pipeline, 1.seconds());
    set_time(&pipeline, 1.seconds() + LATENCY);
    let mut buffer = pull_buffer(&pipeline);
    // FIXME: Sometimes we first get the ZERO buffer from the fallback sink
    if buffer.pts() == Some(gst::ClockTime::ZERO) {
        buffer = pull_buffer(&pipeline);
    }
    assert_fallback_buffer!(buffer, Some(gst::ClockTime::SECOND));

    switch.set_property("active-pad", &mainsink);
    push_buffer(&pipeline, 2.seconds());
    push_fallback_buffer(&pipeline, 2.seconds());
    set_time(&pipeline, 2.seconds() + LATENCY);
    buffer = pull_buffer(&pipeline);
    // FIXME: Sometimes we first get the 1sec buffer from the main sink
    if buffer.pts() == Some(gst::ClockTime::SECOND) {
        buffer = pull_buffer(&pipeline);
    }
    assert_buffer!(buffer, Some(2.seconds()));

    drop(mainsink);
    drop(fallbacksink);
    drop(switch);
    // EOS on the fallback should not be required
    push_eos(&pipeline);
    wait_eos(&pipeline);

    stop_pipeline(pipeline);
}

struct Pipeline {
    pipeline: gst::Pipeline,
    clock_join_handle: Option<std::thread::JoinHandle<()>>,
}

impl std::ops::Deref for Pipeline {
    type Target = gst::Pipeline;

    fn deref(&self) -> &gst::Pipeline {
        &self.pipeline
    }
}

fn setup_pipeline(
    with_live_fallback: Option<bool>,
    immediate_fallback: Option<bool>,
    auto_switch: Option<bool>,
) -> Pipeline {
    init();

    debug!(TEST_CAT, "Setting up pipeline");

    let clock = gst_check::TestClock::new();
    clock.set_time(gst::ClockTime::ZERO);
    let pipeline = gst::Pipeline::default();

    // Running time 0 in our pipeline is going to be clock time 1s. All
    // clock ids before 1s are used for signalling to our clock advancing
    // thread.
    pipeline.use_clock(Some(&clock));
    pipeline.set_base_time(gst::ClockTime::SECOND);
    pipeline.set_start_time(gst::ClockTime::NONE);

    let src = gst_app::AppSrc::builder()
        .name("src")
        .is_live(true)
        .format(gst::Format::Time)
        .min_latency(LATENCY.nseconds() as i64)
        .caps(
            &gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Argb)
                .width(320)
                .height(240)
                .framerate((0, 1).into())
                .build(),
        )
        .build();

    let switch = gst::ElementFactory::make("fallbackswitch")
        .name("switch")
        .property("timeout", 3.seconds())
        .property_if_some("immediate-fallback", immediate_fallback)
        .property_if_some("auto-switch", auto_switch)
        .build()
        .unwrap();

    let sink = gst_app::AppSink::builder().name("sink").sync(false).build();

    let queue = gst::ElementFactory::make("queue").build().unwrap();

    pipeline
        .add_many([src.upcast_ref(), &switch, &queue, sink.upcast_ref()])
        .unwrap();
    src.link_pads(Some("src"), &switch, Some("sink_0")).unwrap();
    switch.link_pads(Some("src"), &queue, Some("sink")).unwrap();
    queue.link_pads(Some("src"), &sink, Some("sink")).unwrap();

    let sink_pad = switch.static_pad("sink_0").unwrap();
    sink_pad.set_property("priority", 0u32);

    if let Some(live) = with_live_fallback {
        let fallback_src = gst_app::AppSrc::builder()
            .name("fallback-src")
            .is_live(live)
            .format(gst::Format::Time)
            .min_latency(LATENCY.nseconds() as i64)
            .caps(
                &gst_video::VideoCapsBuilder::new()
                    .format(gst_video::VideoFormat::Argb)
                    .width(160)
                    .height(120)
                    .framerate((0, 1).into())
                    .build(),
            )
            .build();

        pipeline.add(&fallback_src).unwrap();

        fallback_src
            .link_pads(Some("src"), &switch, Some("sink_1"))
            .unwrap();
        let sink_pad = switch.static_pad("sink_1").unwrap();
        sink_pad.set_property("priority", 1u32);
    }

    pipeline.set_state(gst::State::Playing).unwrap();

    let clock_join_handle = std::thread::spawn(move || {
        loop {
            while let Some(clock_id) = clock.peek_next_pending_id().and_then(|clock_id| {
                // Process if the clock ID is in the past or now
                if clock.time() >= clock_id.time() {
                    Some(clock_id)
                } else {
                    None
                }
            }) {
                debug!(
                    TEST_CAT,
                    "Processing clock ID {} at {:?}",
                    clock_id.time(),
                    clock.time()
                );
                if let Some(clock_id) = clock.process_next_clock_id() {
                    debug!(TEST_CAT, "Processed clock ID {}", clock_id.time());
                    if clock_id.time().is_zero() {
                        debug!(TEST_CAT, "Stopping clock thread");
                        return;
                    }
                }
            }

            // Sleep for 5ms as long as we have pending clock IDs that are in the future
            // at the top of the queue. We don't want to do a busy loop here.
            while clock.peek_next_pending_id().iter().any(|clock_id| {
                // Sleep if the clock ID is in the future
                clock.time() < clock_id.time()
            }) {
                use std::{thread, time};

                thread::sleep(time::Duration::from_millis(10));
            }

            // Otherwise if there are none (or they are ready now) wait until there are
            // clock ids again
            let _ = clock.wait_for_next_pending_id();
        }
    });

    Pipeline {
        pipeline,
        clock_join_handle: Some(clock_join_handle),
    }
}

fn push_buffer(pipeline: &Pipeline, time: gst::ClockTime) {
    let src = pipeline
        .by_name("src")
        .unwrap()
        .downcast::<gst_app::AppSrc>()
        .unwrap();
    let mut buffer = gst::Buffer::with_size(320 * 240 * 4).unwrap();
    {
        let buffer = buffer.get_mut().unwrap();
        buffer.set_pts(time);
    }
    src.push_buffer(buffer).unwrap();
}

fn push_fallback_buffer(pipeline: &Pipeline, time: gst::ClockTime) {
    let src = pipeline
        .by_name("fallback-src")
        .unwrap()
        .downcast::<gst_app::AppSrc>()
        .unwrap();
    let mut buffer = gst::Buffer::with_size(160 * 120 * 4).unwrap();
    {
        let buffer = buffer.get_mut().unwrap();
        buffer.set_pts(time);
    }
    src.push_buffer(buffer).unwrap();
}

fn push_eos(pipeline: &Pipeline) {
    let src = pipeline
        .by_name("src")
        .unwrap()
        .downcast::<gst_app::AppSrc>()
        .unwrap();
    src.end_of_stream().unwrap();
}

fn push_fallback_eos(pipeline: &Pipeline) {
    let src = pipeline
        .by_name("fallback-src")
        .unwrap()
        .downcast::<gst_app::AppSrc>()
        .unwrap();
    src.end_of_stream().unwrap();
}

fn pull_buffer(pipeline: &Pipeline) -> gst::Buffer {
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    let sample = sink.pull_sample().unwrap();
    sample.buffer_owned().unwrap()
}

fn set_time(pipeline: &Pipeline, time: gst::ClockTime) {
    let clock = pipeline
        .clock()
        .unwrap()
        .downcast::<gst_check::TestClock>()
        .unwrap();

    debug!(TEST_CAT, "Setting time to {}", time);
    clock.set_time(gst::ClockTime::SECOND + time);
}

fn wait_eos(pipeline: &Pipeline) {
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    // FIXME: Ideally without a sleep
    loop {
        use std::{thread, time};

        if sink.is_eos() {
            debug!(TEST_CAT, "Waited for EOS");
            break;
        }
        thread::sleep(time::Duration::from_millis(10));
    }
}

fn stop_pipeline(mut pipeline: Pipeline) {
    pipeline.set_state(gst::State::Null).unwrap();

    let clock = pipeline
        .clock()
        .unwrap()
        .downcast::<gst_check::TestClock>()
        .unwrap();

    // Signal shutdown to the clock thread
    let clock_id = clock.new_single_shot_id(gst::ClockTime::ZERO);
    let _ = clock_id.wait();

    let switch = pipeline.by_name("switch").unwrap();
    let switch_weak = switch.downgrade();
    drop(switch);
    let pipeline_weak = pipeline.downgrade();

    pipeline.clock_join_handle.take().unwrap().join().unwrap();
    drop(pipeline);

    assert!(switch_weak.upgrade().is_none());
    assert!(pipeline_weak.upgrade().is_none());
}
