// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;
use gst::prelude::*;
use serial_test::serial;

use pretty_assertions::assert_eq;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsinter::plugin_register_static().unwrap();
    });
}

fn start_consumer(producer_name: &str) -> gst_check::Harness {
    let mut hc = gst_check::Harness::new("intersrc");

    hc.element()
        .unwrap()
        .set_property("producer-name", producer_name);
    hc.play();

    hc
}

fn start_producer(producer_name: &str) -> (gst::Pad, gst::Element) {
    let element = gst::ElementFactory::make("intersink")
        .property("producer-name", producer_name)
        .build()
        .unwrap();

    element.set_state(gst::State::Playing).unwrap();

    let sinkpad = element.static_pad("sink").unwrap();
    let srcpad = gst::Pad::new(gst::PadDirection::Src);
    srcpad.set_active(true).unwrap();
    srcpad.link(&sinkpad).unwrap();

    srcpad.push_event(gst::event::StreamStart::builder("foo").build());
    srcpad
        .push_event(gst::event::Caps::builder(&gst::Caps::builder("video/x-raw").build()).build());
    srcpad.push_event(
        gst::event::Segment::builder(&gst::FormattedSegment::<gst::format::Time>::new()).build(),
    );

    (srcpad, element)
}

fn push_one(srcpad: &gst::Pad, pts: gst::ClockTime) {
    let mut inbuf = gst::Buffer::with_size(1).unwrap();

    {
        let buf = inbuf.get_mut().unwrap();
        buf.set_pts(pts);
    }

    srcpad.push(inbuf).unwrap();
}

#[test]
#[serial]
fn test_forward_one_buffer() {
    init();

    let mut hc = start_consumer("p1");
    let (srcpad, element) = start_producer("p1");

    push_one(&srcpad, gst::ClockTime::from_nseconds(1));

    let outbuf = hc.pull().unwrap();

    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(1)));

    element.set_state(gst::State::Null).unwrap();
}

#[test]
#[serial]
fn test_change_name_of_producer() {
    init();

    let mut hc1 = start_consumer("p1");
    let mut hc2 = start_consumer("p2");
    let (srcpad, element) = start_producer("p1");

    /* Once this returns, the buffer should have been dispatched only to hc1 */
    push_one(&srcpad, gst::ClockTime::from_nseconds(1));
    let outbuf = hc1.pull().unwrap();
    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(1)));

    element.set_property("producer-name", "p2");

    /* This should only get dispatched to hc2, and it should be its first buffer */
    push_one(&srcpad, gst::ClockTime::from_nseconds(2));
    let outbuf = hc2.pull().unwrap();
    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(2)));

    element.set_property("producer-name", "p1");

    /* Back to hc1, which should not see the buffer we pushed in the previous step */
    push_one(&srcpad, gst::ClockTime::from_nseconds(3));
    let outbuf = hc1.pull().unwrap();
    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(3)));

    element.set_state(gst::State::Null).unwrap();
}

#[test]
#[serial]
fn test_change_producer_name() {
    init();

    let mut hc = start_consumer("p1");
    let (srcpad1, element1) = start_producer("p1");
    let (srcpad2, element2) = start_producer("p2");

    /* This buffer should be dispatched to no consumer */
    push_one(&srcpad2, gst::ClockTime::from_nseconds(1));

    /* This one should be dispatched to hc, and it should be its first buffer */
    push_one(&srcpad1, gst::ClockTime::from_nseconds(2));
    let outbuf = hc.pull().unwrap();
    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(2)));

    hc.element().unwrap().set_property("producer-name", "p2");

    /* This buffer should be dispatched to no consumer */
    push_one(&srcpad1, gst::ClockTime::from_nseconds(3));

    /* This one should be dispatched to hc, and it should be its next buffer */
    push_one(&srcpad2, gst::ClockTime::from_nseconds(4));
    let outbuf = hc.pull().unwrap();
    assert_eq!(outbuf.pts(), Some(gst::ClockTime::from_nseconds(4)));

    element1.set_state(gst::State::Null).unwrap();
    element2.set_state(gst::State::Null).unwrap();
}

#[test]
#[serial]
fn test_event_forwarding() {
    init();

    let mut hc = start_consumer("p");
    let (srcpad, intersink) = start_producer("p");

    intersink.set_state(gst::State::Null).unwrap();
    intersink.set_property(
        "event-types",
        gst::Array::new(vec![gst::EventType::Eos, gst::EventType::CustomDownstream]),
    );
    intersink.set_state(gst::State::Playing).unwrap();

    // FYI necessary push b/c:
    // https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/merge_requests/1875#note_2630453
    push_one(&srcpad, gst::ClockTime::from_nseconds(1));

    let s = gst::Structure::builder("MyEvent")
        .field("unsigned", 100u64)
        .build();
    assert!(srcpad.push_event(gst::event::CustomDownstream::new(s)));
    assert!(srcpad.push_event(gst::event::Eos::new()));

    let mut found = false;
    loop {
        use gst::EventView;

        if let Ok(event) = hc.pull_event() {
            match event.view() {
                EventView::CustomDownstream(e) => {
                    let v = e.structure().unwrap().get::<u64>("unsigned");
                    assert!(v.is_ok_and(|v| v == 100u64));
                    found = true;
                    break;
                }
                EventView::Eos(..) => {
                    break;
                }
                _ => (),
            };
        }
    }
    intersink.set_state(gst::State::Null).unwrap();
    assert!(found);
}

fn test_latency_propagation_with(sync: bool) {
    init();

    let pipe_up = gst::parse::launch(&format!(
        "videotestsrc is-live=true ! intersink sync={sync} name=producer"
    ))
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();
    let pipe_down = gst::parse::launch("intersrc ! fakesink name=sink")
        .unwrap()
        .downcast::<gst::Pipeline>()
        .unwrap();

    let got_latency = Arc::new(AtomicBool::new(false));
    let sink_pad = pipe_down
        .by_name("sink")
        .unwrap()
        .static_pad("sink")
        .unwrap();
    sink_pad.add_probe(
        gst::PadProbeType::QUERY_UPSTREAM | gst::PadProbeType::PULL,
        {
            let got_latency = got_latency.clone();
            move |_, info| {
                let Some(q) = info.query() else {
                    unreachable!();
                };

                if let gst::QueryView::Latency(q) = q.view() {
                    println!(
                        "==> Downstream: latency query (sync {sync}): {}",
                        q.result().1
                    );
                    if q.result().1 > gst::ClockTime::ZERO {
                        got_latency.store(true, Ordering::SeqCst);
                    }
                }
                gst::PadProbeReturn::Ok
            }
        },
    );

    let clock = gst::SystemClock::obtain();
    let base_time = clock.time();

    pipe_up.set_clock(Some(&clock)).unwrap();
    pipe_up.set_start_time(gst::ClockTime::NONE);
    pipe_up.set_base_time(base_time);

    pipe_down.set_clock(Some(&clock)).unwrap();
    pipe_down.set_start_time(gst::ClockTime::NONE);
    pipe_down.set_base_time(base_time);

    pipe_up.set_state(gst::State::Playing).unwrap();
    pipe_down.set_state(gst::State::Playing).unwrap();

    futures::executor::block_on(async {
        use gst::MessageView::*;

        let mut bus_up_stream = pipe_up.bus().unwrap().stream();
        let mut bus_down_stream = pipe_down.bus().unwrap().stream();

        loop {
            if got_latency.load(Ordering::SeqCst) {
                break;
            }

            futures::select! {
                msg = bus_down_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            println!("\n==> Got downstream pipeline latency message (sync {sync})");
                            let _ = pipe_down.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::latency_propagation (sync {sync}) {err:?}"),
                        _ => (),
                    }
                }
                msg = bus_up_stream.next() => {
                    let Some(msg) = msg else { continue };
                    match msg.view() {
                        Latency(_) => {
                            println!("\n==> Got upstream pipeline latency message (sync {sync})");
                            let _ = pipe_up.recalculate_latency();
                        }
                        Error(err) => unreachable!("inter::latency_propagation (sync {sync}) {err:?}"),
                        _ => (),
                    }
                }
            };
        }
    });

    let producer_pad = pipe_up
        .by_name("producer")
        .unwrap()
        .static_pad("sink")
        .unwrap();
    let mut q_lat_prod = gst::query::Latency::new();
    assert!(producer_pad.peer_query(&mut q_lat_prod));

    let mut q_lat_sink = gst::query::Latency::new();
    assert!(sink_pad.peer_query(&mut q_lat_sink));

    let expected = if sync {
        q_lat_prod.result().1 + 20.mseconds() // appsink's processing deadline
    } else {
        q_lat_prod.result().1
    };
    assert_eq!(expected, q_lat_sink.result().1);

    pipe_up.set_state(gst::State::Null).unwrap();
    pipe_down.set_state(gst::State::Null).unwrap();
}

#[test]
#[serial]
fn test_latency_propagation_sync() {
    test_latency_propagation_with(true);
}

#[test]
#[serial]
fn test_latency_propagation_non_sync() {
    test_latency_propagation_with(false);
}
