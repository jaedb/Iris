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
fn test_combine_multi() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 200.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    let sink_1 = combiner.request_pad_simple("sink_1").unwrap();

    let mut h0 = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h0.add_element_sink_pad(&sink_0);
    let mut h1 = gst_check::Harness::with_element(&combiner, None, None);
    h1.add_element_sink_pad(&sink_1);

    let h0_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(50, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    let h1_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Gray8, 320, 240)
        .fps(gst::Fraction::new(25, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h0.set_src_caps(h0_caps.clone());
    h0.play();

    h1.set_src_caps(h1_caps.clone());
    h1.play();

    // Push buffers according to the framerate for the first batch
    // and one additional for the second batch to get an output.
    for i in 0..12 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 20.mseconds());
            buffer.set_duration(20.mseconds());
        }
        assert_eq!(h0.push(buffer), Ok(gst::FlowSuccess::Ok));

        if i % 2 == 0 {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts((i / 2) * 40.mseconds());
                buffer.set_duration(40.mseconds());
            }
            assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));
        }
    }

    let buffer = h0.pull().unwrap();

    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(200.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 2);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h0_caps));

    let objects = stream.objects();
    assert_eq!(objects.len(), 10);
    for (idx, object) in objects.iter().enumerate() {
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(idx as u64 * 20.mseconds()));
        assert_eq!(b.duration(), Some(20.mseconds()));
    }
    let stream = &streams[1];
    assert_eq!(stream.index(), 1);
    assert_eq!(
        stream.stream_id(),
        Some(sink_1.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h1_caps));

    let objects = stream.objects();
    assert_eq!(objects.len(), 5);
    for (idx, object) in objects.iter().enumerate() {
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(idx as u64 * 40.mseconds()));
        assert_eq!(b.duration(), Some(40.mseconds()));
    }

    h0.push_event(gst::event::Eos::new());
    h1.push_event(gst::event::Eos::new());

    let buffer = h0.pull().unwrap();

    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(200.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 2);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h0_caps));

    let objects = stream.objects();
    assert_eq!(objects.len(), 2);
    for (idx, object) in objects.iter().enumerate() {
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(200.mseconds() + idx as u64 * 20.mseconds()));
        assert_eq!(b.duration(), Some(20.mseconds()));
    }
    let stream = &streams[1];
    assert_eq!(stream.index(), 1);
    assert_eq!(
        stream.stream_id(),
        Some(sink_1.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h1_caps));

    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    for (idx, object) in objects.iter().enumerate() {
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(200.mseconds() + idx as u64 * 40.mseconds()));
        assert_eq!(b.duration(), Some(40.mseconds()));
    }

    // Now finally check all the events

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h0.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].as_ref(), Some(&h0_caps));
    assert_eq!(streams[1].as_ref(), Some(&h1_caps));

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_strategy_all() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 100.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    sink_0.set_property_from_str("batch-strategy", "all");

    let mut h = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h.add_element_sink_pad(&sink_0);

    let h_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(30, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h.set_src_caps(h_caps.clone());
    h.play();

    let ptss = [0, 33, 66, 100];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 3);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(ptss[idx])));
        assert_eq!(b.duration(), Some(33_333_333.nseconds()));
    }

    let ptss = [133, 200];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let ptss = [100, 133];
    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(100.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 2);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(ptss[idx])));
        assert_eq!(b.duration(), Some(33_333_333.nseconds()));
    }

    let ptss = [233, 233, 266, 300];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let ptss = [200, 233, 233, 266];
    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 4);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(ptss[idx])));
        assert_eq!(b.duration(), Some(33_333_333.nseconds()));
    }

    h.push_event(gst::event::Eos::new());

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(300.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));

    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(300)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    // Now finally check all the events

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].as_ref(), Some(&h_caps));

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_strategy_first() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 100.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    sink_0.set_property_from_str("batch-strategy", "first-in-batch");

    let mut h = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h.add_element_sink_pad(&sink_0);

    let h_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(30, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h.set_src_caps(h_caps.clone());
    h.play();

    let ptss = [0, 33, 66, 100];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(0)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [133, 200];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(100.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(100)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [233, 233, 266, 300];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(200)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    h.push_event(gst::event::Eos::new());

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(300.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(300)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    // Now finally check all the events

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].as_ref(), Some(&h_caps));

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_strategy_first_with_overlap() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 100.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    sink_0.set_property_from_str("batch-strategy", "first-in-batch-with-overlap");

    let mut h = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h.add_element_sink_pad(&sink_0);

    let h_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(30, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h.set_src_caps(h_caps.clone());
    h.play();

    let ptss = [0, 33, 66, 100];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(0)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [133, 199, 233];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(100.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(100)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [233, 266, 301, 333];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(199)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    h.push_event(gst::event::Eos::new());

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(300.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(301)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    // Now finally check all the events

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].as_ref(), Some(&h_caps));

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_strategy_last() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 100.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    sink_0.set_property_from_str("batch-strategy", "last-in-batch");

    let mut h = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h.add_element_sink_pad(&sink_0);

    let h_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(30, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h.set_src_caps(h_caps.clone());
    h.play();

    let ptss = [0, 33, 66, 100];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(66)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [133, 200];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(100.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(133)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    let ptss = [233, 233, 266, 300];
    for pts in ptss {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pts));
            buffer.set_duration(33_333_333.nseconds());
        }
        assert_eq!(h.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(266)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    h.push_event(gst::event::Eos::new());

    let buffer = h.pull().unwrap();
    assert_eq!(buffer.pts(), Some(300.mseconds()));
    assert_eq!(buffer.duration(), Some(100.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    let object = &objects[0];
    assert_eq!(
        stream.stream_id(),
        Some(sink_0.stream_id().unwrap().as_gstr())
    );
    assert_eq!(
        stream.segment(),
        Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
    );
    assert_eq!(stream.caps().as_ref(), Some(&h_caps));
    let b = object.downcast_ref::<gst::Buffer>().unwrap();
    assert_eq!(b.pts(), Some(gst::ClockTime::from_mseconds(300)));
    assert_eq!(b.duration(), Some(33_333_333.nseconds()));

    // Now finally check all the events

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].as_ref(), Some(&h_caps));

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}

#[test]
fn test_combine_multi_initial_gap() {
    init();

    let combiner = gst::ElementFactory::make("analyticscombiner")
        .property("batch-duration", 200.mseconds())
        .build()
        .unwrap();
    let sink_0 = combiner.request_pad_simple("sink_0").unwrap();
    let sink_1 = combiner.request_pad_simple("sink_1").unwrap();

    let mut h0 = gst_check::Harness::with_element(&combiner, None, Some("src"));
    h0.add_element_sink_pad(&sink_0);
    let mut h1 = gst_check::Harness::with_element(&combiner, None, None);
    h1.add_element_sink_pad(&sink_1);

    let h0_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, 320, 240)
        .fps(gst::Fraction::new(50, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    let h1_caps = gst_video::VideoInfo::builder(gst_video::VideoFormat::Gray8, 320, 240)
        .fps(gst::Fraction::new(25, 1))
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    h0.set_src_caps(h0_caps.clone());
    h0.play();

    // Push buffers according to the framerate for the first batch but only for the first stream
    // and one additional buffer for the second batch to get an output.
    for i in 0..11 {
        let mut buffer = gst::Buffer::with_size(1).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(i * 20.mseconds());
            buffer.set_duration(20.mseconds());
        }
        assert_eq!(h0.push(buffer), Ok(gst::FlowSuccess::Ok));
    }

    // Crank the clock for timing out
    h0.crank_single_clock_wait().unwrap();

    let buffer = h0.pull().unwrap();

    assert_eq!(buffer.pts(), Some(0.mseconds()));
    assert_eq!(buffer.duration(), Some(200.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 2);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 10);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h0_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(idx as u64 * 20.mseconds()));
        assert_eq!(b.duration(), Some(20.mseconds()));
    }
    let stream = &streams[1];
    assert_eq!(stream.index(), 1);
    let objects = stream.objects();
    // Only an empty buffer with no events or anything for the second stream
    assert_eq!(objects.len(), 0);
    assert_eq!(stream.stream_id(), None);
    assert_eq!(stream.segment(), None);
    assert_eq!(stream.caps().as_ref(), None);

    // Now start the second stream
    h1.set_src_caps(h1_caps.clone());
    h1.play();

    // Push buffers according to the framerate for the second batch for both streams
    for i in 0..11 {
        if i > 0 {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(200.mseconds() + i * 20.mseconds());
                buffer.set_duration(20.mseconds());
            }
            assert_eq!(h0.push(buffer), Ok(gst::FlowSuccess::Ok));
        }

        if i % 2 == 0 {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(200.mseconds() + (i / 2) * 40.mseconds());
                buffer.set_duration(40.mseconds());
            }
            assert_eq!(h1.push(buffer), Ok(gst::FlowSuccess::Ok));
        }
    }

    let buffer = h0.pull().unwrap();

    assert_eq!(buffer.pts(), Some(200.mseconds()));
    assert_eq!(buffer.duration(), Some(200.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 2);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 10);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h0_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(200.mseconds() + idx as u64 * 20.mseconds()));
        assert_eq!(b.duration(), Some(20.mseconds()));
    }
    let stream = &streams[1];
    assert_eq!(stream.index(), 1);
    let objects = stream.objects();
    assert_eq!(objects.len(), 5);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_1.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h1_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(200.mseconds() + idx as u64 * 40.mseconds()));
        assert_eq!(b.duration(), Some(40.mseconds()));
    }

    h0.push_event(gst::event::Eos::new());
    h1.push_event(gst::event::Eos::new());

    let buffer = h0.pull().unwrap();

    assert_eq!(buffer.pts(), Some(400.mseconds()));
    assert_eq!(buffer.duration(), Some(200.mseconds()));
    let meta = buffer.meta::<gst_analytics::AnalyticsBatchMeta>().unwrap();
    let streams = meta.streams();
    assert_eq!(streams.len(), 2);
    let stream = &streams[0];
    assert_eq!(stream.index(), 0);
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_0.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h0_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(400.mseconds() + idx as u64 * 20.mseconds()));
        assert_eq!(b.duration(), Some(20.mseconds()));
    }
    let stream = &streams[1];
    assert_eq!(stream.index(), 1);
    let objects = stream.objects();
    assert_eq!(objects.len(), 1);
    for (idx, object) in objects.iter().enumerate() {
        assert_eq!(
            stream.stream_id(),
            Some(sink_1.stream_id().unwrap().as_gstr())
        );
        assert_eq!(
            stream.segment(),
            Some(gst::FormattedSegment::<gst::ClockTime>::new().upcast())
        );
        assert_eq!(stream.caps().as_ref(), Some(&h1_caps));
        let b = object.downcast_ref::<gst::Buffer>().unwrap();
        assert_eq!(b.pts(), Some(400.mseconds() + idx as u64 * 40.mseconds()));
        assert_eq!(b.duration(), Some(40.mseconds()));
    }

    // Now finally check all the events

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);

    let ev = h0.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].as_ref(), Some(&h0_caps));
    assert_eq!(
        streams[1].as_ref(),
        Some(&gst::Caps::new_empty_simple("application/unknown"))
    );

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);

    let ev = h0.pull_event().unwrap();
    let gst::EventView::Caps(ev) = ev.view() else {
        assert_eq!(ev.type_(), gst::EventType::Caps);
        unreachable!();
    };
    let caps = ev.caps();
    let s = caps.structure(0).unwrap();
    assert_eq!(s.name(), "multistream/x-analytics-batch");
    let streams = s
        .get::<gst::ArrayRef>("streams")
        .unwrap()
        .iter()
        .map(|v| v.get::<Option<gst::Caps>>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].as_ref(), Some(&h0_caps));
    assert_eq!(streams[1].as_ref(), Some(&h1_caps));

    let ev = h0.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}
