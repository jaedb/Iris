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
        gstgopbuffer::plugin_register_static().unwrap();
    });
}

macro_rules! check_buffer {
    ($buf1:expr, $buf2:expr) => {
        assert_eq!($buf1.pts(), $buf2.pts());
        assert_eq!($buf1.dts(), $buf2.dts());
        assert_eq!($buf1.flags(), $buf2.flags());
    };
}

#[test]
fn test_min_one_gop_held() {
    const OFFSET: gst::ClockTime = gst::ClockTime::from_seconds(10);
    init();

    let mut h =
        gst_check::Harness::with_padnames("gopbuffer", Some("video_sink"), Some("video_src"));

    // 200ms min buffer time
    let element = h.element().unwrap();
    element.set_property("minimum-duration", gst::ClockTime::from_mseconds(200));

    h.set_src_caps(
        gst::Caps::builder("video/x-h264")
            .field("width", 320i32)
            .field("height", 240i32)
            .field("framerate", gst::Fraction::new(10, 1))
            .field("stream-format", "avc")
            .field("alignment", "au")
            .field("codec_data", gst::Buffer::with_size(1).unwrap())
            .build(),
    );
    let mut in_segment = gst::Segment::new();
    in_segment.set_format(gst::Format::Time);
    in_segment.set_base(10.seconds());
    assert!(h.push_event(gst::event::Segment::builder(&in_segment).build()));

    h.play();

    // Push 10 buffers of 100ms each, 2nd and 5th buffer without DELTA_UNIT flag
    let in_buffers: Vec<_> = (0..6)
        .map(|i| {
            let mut buffer = gst::Buffer::with_size(1).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(OFFSET + gst::ClockTime::from_mseconds(i * 100));
                buffer.set_dts(OFFSET + gst::ClockTime::from_mseconds(i * 100));
                buffer.set_duration(gst::ClockTime::from_mseconds(100));
                if i != 1 && i != 4 {
                    buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
                }
            }
            assert_eq!(h.push(buffer.clone()), Ok(gst::FlowSuccess::Ok));
            buffer
        })
        .collect();

    // pull mandatory events
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::StreamStart);
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Caps);
    // GstHarness pushes its own segment event that we need to eat
    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Segment);
    let ev = h.pull_event().unwrap();
    let gst::event::EventView::Segment(recv_segment) = ev.view() else {
        unreachable!()
    };
    let recv_segment = recv_segment.segment();
    assert_eq!(recv_segment, &in_segment);

    // check that at least the first GOP has been output already as it exceeds the minimum-time
    // value
    let mut in_iter = in_buffers.iter();

    // the first buffer is dropped because it was not preceded by a keyframe
    let _buffer = in_iter.next().unwrap();

    // a keyframe
    let out = h.pull().unwrap();
    let buffer = in_iter.next().unwrap();
    check_buffer!(buffer, out);

    // not a keyframe
    let out = h.pull().unwrap();
    let buffer = in_iter.next().unwrap();
    check_buffer!(buffer, out);

    // not a keyframe
    let out = h.pull().unwrap();
    let buffer = in_iter.next().unwrap();
    check_buffer!(buffer, out);

    // no more buffers
    assert_eq!(h.buffers_in_queue(), 0);

    // push eos to drain out the rest of the data
    assert!(h.push_event(gst::event::Eos::new()));
    for buffer in in_iter {
        let out = h.pull().unwrap();
        check_buffer!(buffer, out);
    }

    // no more buffers
    assert_eq!(h.buffers_in_queue(), 0);

    let ev = h.pull_event().unwrap();
    assert_eq!(ev.type_(), gst::EventType::Eos);
}
