// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::task;
use std::thread;
use std::time::Duration;

use futures::channel::mpsc;
use futures::executor::block_on;
use futures::prelude::*;

use gst::prelude::*;

use gstthreadshare::runtime::Context;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare blocking-adapter test");
    });
}

#[derive(Debug)]
enum Item {
    Buffer(gst::Buffer),
    Event(gst::Event),
}

async fn run_srctask(
    srcpad: gst::Pad,
    mut item_rx: mpsc::Receiver<Item>,
    mut res_tx: mpsc::Sender<Result<(), gst::FlowError>>,
) {
    loop {
        let mut res = match item_rx.next().await {
            Some(Item::Event(event)) => {
                if srcpad.push_event(event) {
                    Ok(())
                } else {
                    Err(gst::FlowError::Error)
                }
            }
            Some(Item::Buffer(buffer)) => srcpad.push(buffer).map(drop),
            None => break,
        };

        if res.is_ok() {
            res = Context::drain_sub_tasks().await;
        }

        res_tx.send(res).await.unwrap();
    }

    println!("Exiting src task");
}

fn send_initial_events(
    test_name: &str,
    srcpad: &gst::Pad,
    appsink: &gst_app::AppSink,
    item_tx: &mut mpsc::Sender<Item>,
    res_rx: &mut mpsc::Receiver<Result<(), gst::FlowError>>,
) {
    assert!(appsink
        .try_pull_object(gst::ClockTime::from_mseconds(20))
        .is_none());

    item_tx
        .try_send(Item::Event(
            gst::event::StreamStart::builder(test_name)
                .group_id(gst::GroupId::next())
                .build(),
        ))
        .unwrap();
    assert_eq!(
        appsink
            .pull_object()
            .unwrap()
            .downcast::<gst::Event>()
            .unwrap()
            .type_(),
        gst::EventType::StreamStart
    );
    block_on(res_rx.next()).unwrap().unwrap();

    item_tx
        .try_send(Item::Event(gst::event::Caps::new(
            srcpad.pad_template().unwrap().caps(),
        )))
        .unwrap();
    assert_eq!(
        appsink
            .pull_object()
            .unwrap()
            .downcast::<gst::Event>()
            .unwrap()
            .type_(),
        gst::EventType::Caps
    );
    block_on(res_rx.next()).unwrap().unwrap();

    item_tx
        .try_send(Item::Event(gst::event::Segment::new(
            &gst::FormattedSegment::<gst::format::Time>::new(),
        )))
        .unwrap();
    assert_eq!(
        appsink
            .pull_object()
            .unwrap()
            .downcast::<gst::Event>()
            .unwrap()
            .type_(),
        gst::EventType::Segment
    );
    block_on(res_rx.next()).unwrap().unwrap();
}

#[test]
fn without_adapter() {
    init();

    const CONTEXT_NAME: &str = "blocking-adapter::without";
    let ts_ctx = Context::acquire(CONTEXT_NAME, Duration::ZERO).unwrap();

    let pipeline = gst::Pipeline::default();

    let srcpad_templ = gst::PadTemplate::builder(
        "src",
        gst::PadDirection::Src,
        gst::PadPresence::Always,
        &gst::Caps::new_empty_simple("test/x-raw"),
    )
    .build()
    .unwrap();
    let srcpad = gst::Pad::builder_from_template(&srcpad_templ)
        .name("itemsrc::blocking-adapter::without")
        .build();
    srcpad.set_active(true).unwrap();

    let appsink = gst_app::AppSink::builder()
        .name("appsink::blocking-adapter::without")
        .property("max-buffers", 1u32)
        .build();

    let appsink_elem = appsink.upcast_ref::<gst::Element>();
    pipeline.add(appsink_elem).unwrap();
    srcpad
        .link(&appsink_elem.static_pad("sink").unwrap())
        .unwrap();

    let (mut item_tx, item_rx) = mpsc::channel(0);
    let (res_tx, mut res_rx) = mpsc::channel(0);
    let srctask_handle = ts_ctx.spawn(run_srctask(srcpad.clone(), item_rx, res_tx));

    pipeline.set_state(gst::State::Playing).unwrap();

    send_initial_events(
        "blocking-adapter::without",
        &srcpad,
        &appsink,
        &mut item_tx,
        &mut res_rx,
    );

    // Push & pull one buffer => latency init
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    println!("awaiting 1st buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    // This one will be enqueued by appsink
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    println!("awaiting 2d buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();
    // not pulling 2d buffer

    // This 3d buffer will be blocked by appsink because the queue is full
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    // wait a bit in order to make sure it is handled
    thread::sleep(Duration::from_millis(20));
    // result not available yet
    assert!(res_rx.try_next().is_err());

    // Spawn a concurrent task on the ts-context
    let mut ts_task_handle = ts_ctx.spawn(futures::future::ready(42));

    // The concurrent task is blocked
    // wait a bit in order to make sure it is handled
    thread::sleep(Duration::from_millis(20));
    let mut cx = task::Context::from_waker(task::Waker::noop());
    assert!(ts_task_handle.poll_unpin(&mut cx).is_pending());

    // 3d buffer handling result still not available
    assert!(res_rx.try_next().is_err());

    println!("Pulling 2d buffer => unblocking");
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    // The concurrent task is un-blocked too
    println!("awaiting concurrent task");
    assert_eq!(block_on(ts_task_handle).unwrap(), 42);

    println!("awaiting 3d buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();

    println!("Pulling 3d buffer");
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    pipeline.set_state(gst::State::Ready).unwrap();

    drop(item_tx);
    srcpad.set_active(false).unwrap();
    block_on(srctask_handle).unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn with_adapter() {
    init();

    const CONTEXT_NAME: &str = "blocking-adapter::with";
    let ts_ctx = Context::acquire(CONTEXT_NAME, Duration::ZERO).unwrap();

    let pipeline = gst::Pipeline::default();

    let srcpad = gst::Pad::builder_from_template(
        &gst::PadTemplate::builder(
            "src",
            gst::PadDirection::Src,
            gst::PadPresence::Always,
            &gst::Caps::new_empty_simple("test/x-raw"),
        )
        .build()
        .unwrap(),
    )
    .name("itemsrc::blocking-adapter::with")
    .build();
    srcpad.set_active(true).unwrap();

    let blocking_adapter = gst::ElementFactory::make("ts-blocking-adapter")
        .name("blocking-adapter::with")
        .build()
        .unwrap();
    let appsink = gst_app::AppSink::builder()
        .name("appsink::blocking-adapter::with")
        .property("max-buffers", 1u32)
        .build();

    let elems = [&blocking_adapter, appsink.upcast_ref()];
    pipeline.add_many(elems).unwrap();
    srcpad
        .link(&blocking_adapter.static_pad("sink").unwrap())
        .unwrap();
    gst::Element::link_many(elems).unwrap();

    let (mut item_tx, item_rx) = mpsc::channel(0);
    let (res_tx, mut res_rx) = mpsc::channel(0);
    let srctask_handle = ts_ctx.spawn(run_srctask(srcpad.clone(), item_rx, res_tx));

    pipeline.set_state(gst::State::Playing).unwrap();

    send_initial_events(
        "blocking-adapter::with",
        &srcpad,
        &appsink,
        &mut item_tx,
        &mut res_rx,
    );

    // Push & pull one buffer => latency init
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    println!("awaiting 1st buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    // This one will be enqueued by appsink
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    println!("awaiting 2d buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();
    // not pulling 2d buffer

    // This 3d buffer will be blocked by appsink because the queue is full
    item_tx.try_send(Item::Buffer(gst::Buffer::new())).unwrap();
    // wait a bit in order to make sure it is handled
    thread::sleep(Duration::from_millis(20));
    // result not available yet
    assert!(res_rx.try_next().is_err());

    // Spawn a concurrent task on the ts-context
    let ts_task_handle = ts_ctx.spawn(futures::future::ready(42));

    // The concurrent task is NOT blocked
    println!("awaiting concurrent task");
    assert_eq!(block_on(ts_task_handle).unwrap(), 42);

    // 3d buffer handling result still not available
    assert!(res_rx.try_next().is_err());

    println!("Pulling 2d buffer => unblocking");
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    println!("awaiting 3d buffer push res");
    block_on(res_rx.next()).unwrap().unwrap();

    println!("Pulling 3d buffer");
    appsink
        .pull_object()
        .unwrap()
        .downcast::<gst::Sample>()
        .unwrap();

    pipeline.set_state(gst::State::Ready).unwrap();

    drop(item_tx);
    srcpad.set_active(false).unwrap();
    block_on(srctask_handle).unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}
