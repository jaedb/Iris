// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsanalytics::plugin_register_static().unwrap();
    });
}

#[test]
fn test_combine_split_single() {
    init();

    let bin = gst::parse::bin_from_description("videotestsrc name=src num-buffers=10 ! video/x-raw,framerate=25/1 ! analyticscombiner batch-duration=100000000 ! analyticssplitter ! identity name=identity", false).unwrap();
    let identity = bin.by_name("identity").unwrap();
    bin.add_pad(&gst::GhostPad::with_target(&identity.static_pad("src").unwrap()).unwrap())
        .unwrap();

    let mut h = gst_check::Harness::with_element(&bin, None, Some("src"));
    h.play();

    let ptss = std::array::from_fn::<_, 10, _>(|idx| idx as u64 * 40.mseconds());
    for pts in ptss {
        let buffer = h.pull().unwrap();
        assert_eq!(buffer.pts(), Some(pts));
        assert_eq!(buffer.duration(), Some(40.mseconds()));
    }

    // Now finally check all the events
    let element = h.element().unwrap().downcast::<gst::Bin>().unwrap();
    let src = element.by_name("src").unwrap();
    let srcpad = src.static_pad("src").unwrap();
    let stream_id = srcpad.stream_id().unwrap();
    let caps = srcpad.current_caps().unwrap();

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let gst::EventView::StreamStart(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::StreamStart);
        unreachable!();
    };
    assert_eq!(ev.stream_id(), &stream_id);

    let ev = h.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    assert_eq!(ev.caps(), &caps);

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_combine_split_multi() {
    init();

    let bin = gst::parse::bin_from_description(
        "videotestsrc name=src_0 num-buffers=10 ! video/x-raw,framerate=25/1 ! combiner.sink_0 \
         videotestsrc name=src_1 num-buffers=20 ! video/x-raw,framerate=50/1 ! combiner.sink_1 \
        analyticscombiner name=combiner batch-duration=100000000 ! analyticssplitter name=splitter \
        splitter.src_0_0 ! queue name=queue_0 \
        splitter.src_0_1 ! queue name=queue_1",
        false,
    )
    .unwrap();
    let queue = bin.by_name("queue_0").unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&queue.static_pad("src").unwrap())
            .unwrap()
            .name("src_0")
            .build(),
    )
    .unwrap();
    let queue = bin.by_name("queue_1").unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&queue.static_pad("src").unwrap())
            .unwrap()
            .name("src_1")
            .build(),
    )
    .unwrap();

    let mut h0 = gst_check::Harness::with_element(&bin, None, Some("src_0"));
    let mut h1 = gst_check::Harness::with_element(&bin, None, Some("src_1"));
    h0.play();
    h1.play();

    let ptss = std::array::from_fn::<_, 20, _>(|idx| idx as u64 * 20.mseconds());
    for (idx, pts) in ptss.into_iter().enumerate() {
        if idx % 2 == 0 {
            let buffer = h0.pull().unwrap();
            assert_eq!(buffer.pts(), Some(pts));
            assert_eq!(buffer.duration(), Some(40.mseconds()));
        }
        let buffer = h1.pull().unwrap();
        assert_eq!(buffer.pts(), Some(pts));
        assert_eq!(buffer.duration(), Some(20.mseconds()));
    }

    // Now finally check all the events
    let src = bin.by_name("src_0").unwrap();
    let srcpad = src.static_pad("src").unwrap();
    let stream_id_0 = srcpad.stream_id().unwrap();
    let caps_0 = srcpad.current_caps().unwrap();

    let src = bin.by_name("src_1").unwrap();
    let srcpad = src.static_pad("src").unwrap();
    let stream_id_1 = srcpad.stream_id().unwrap();
    let caps_1 = srcpad.current_caps().unwrap();

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let gst::EventView::StreamStart(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::StreamStart);
        unreachable!();
    };
    assert_eq!(ev.stream_id(), &stream_id_0);

    let ev = h0.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    assert_eq!(ev.caps(), &caps_0);

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let gst::EventView::StreamStart(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::StreamStart);
        unreachable!();
    };
    assert_eq!(ev.stream_id(), &stream_id_1);

    let ev = h1.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    assert_eq!(ev.caps(), &caps_1);

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_combine_split_multi_with_initial_gap() {
    init();

    let h0_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(25, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    let h1_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Gray8, 320, 240)
        .fps(gst::Fraction::new(50, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    let bin = gst::Bin::new();
    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 200.mseconds())
        .build()
        .unwrap();
    let splitter = gst::ElementFactory::make("analyticssplitter")
        .build()
        .unwrap();

    bin.add_many([&combiner, &splitter]).unwrap();
    combiner.link(&splitter).unwrap();

    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&sink_0)
            .unwrap()
            .name("sink_0")
            .build(),
    )
    .unwrap();

    let sink_1 = combiner.request_pad_simple("sink_1").unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&sink_1)
            .unwrap()
            .name("sink_1")
            .build(),
    )
    .unwrap();

    let queue = gst::ElementFactory::make("queue")
        .name("queue_0")
        .build()
        .unwrap();
    bin.add(&queue).unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&queue.static_pad("src").unwrap())
            .unwrap()
            .name("src_0")
            .build(),
    )
    .unwrap();

    let queue = gst::ElementFactory::make("queue")
        .name("queue_1")
        .build()
        .unwrap();
    bin.add(&queue).unwrap();
    bin.add_pad(
        &gst::GhostPad::builder_with_target(&queue.static_pad("src").unwrap())
            .unwrap()
            .name("src_1")
            .build(),
    )
    .unwrap();

    splitter.connect_closure(
        "pad-added",
        false,
        glib::closure!(
            #[weak]
            bin,
            move |_splitter: &gst::Element, pad: &gst::Pad| {
                let pad_name = pad.name();
                let [_, _count, stream_id] =
                    pad_name.split("_").collect::<Vec<_>>().try_into().unwrap();
                let Some(queue) = bin.by_name(&format!("queue_{stream_id}")) else {
                    return;
                };
                let sinkpad = queue.static_pad("sink").unwrap();
                if let Some(peer) = sinkpad.peer() {
                    peer.unlink(&sinkpad).unwrap();
                }
                pad.link(&sinkpad).unwrap();
            }
        ),
    );

    let mut h0 = gst_check::Harness::with_element(&bin, Some("sink_0"), Some("src_0"));
    h0.set_src_caps(h0_caps.clone());
    h0.play();

    // Push first 6 buffers on the first stream only
    for i in 0..6 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 40.mseconds());
            buffer.set_duration(40.mseconds());
        }
        assert_eq!(h0.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Time out so the first batch is output
    h0.crank_single_clock_wait().unwrap();

    // Now consume buffers for the first stream for the first batch
    let ptss = std::array::from_fn::<_, 5, _>(|idx| idx as u64 * 40.mseconds());
    for pts in ptss {
        let buffer = h0.pull().unwrap();
        assert_eq!(buffer.pts(), Some(pts));
        assert_eq!(buffer.duration(), Some(40.mseconds()));
    }

    // Start the second stream
    let mut h1 = gst_check::Harness::with_element(&bin, Some("sink_1"), Some("src_1"));
    h1.set_src_caps(h1_caps.clone());
    h1.play();

    // Push another batch of buffers for both streams this time.
    // Skip the first buffer for the first stream as it was pushed above already.
    for i in 0..10 {
        if i > 0 && i % 2 == 0 {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(200.mseconds() + (i / 2) * 40.mseconds());
                buffer.set_duration(40.mseconds());
            }
            assert_eq!(h0.push(buffer), Ok(gst::FlowSuccess::Ok));
        }

        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(200.mseconds() + i * 20.mseconds());
            buffer.set_duration(20.mseconds());
        }
        assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Now EOS so that the second batch is output
    h0.push_event(gst::event::Eos::new());
    h1.push_event(gst::event::Eos::new());

    let ptss = std::array::from_fn::<_, 10, _>(|idx| 200.mseconds() + idx as u64 * 20.mseconds());
    for (idx, pts) in ptss.into_iter().enumerate() {
        if idx % 2 == 0 {
            let buffer = h0.pull().unwrap();
            assert_eq!(buffer.pts(), Some(pts));
            assert_eq!(buffer.duration(), Some(40.mseconds()));
        }
        let buffer = h1.pull().unwrap();
        assert_eq!(buffer.pts(), Some(pts));
        assert_eq!(buffer.duration(), Some(20.mseconds()));
    }

    // Now finally check all the events
    let stream_id_0 = sink_0.stream_id().unwrap();
    let stream_id_1 = sink_1.stream_id().unwrap();

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let gst::EventView::StreamStart(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::StreamStart);
        unreachable!();
    };
    assert_eq!(ev.stream_id(), &stream_id_0);

    let ev = h0.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    assert_eq!(ev.caps(), &h0_caps);

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let gst::EventView::StreamStart(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::StreamStart);
        unreachable!();
    };
    assert_eq!(ev.stream_id(), &stream_id_1);

    let ev = h1.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    assert_eq!(ev.caps(), &h1_caps);

    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h1.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}
