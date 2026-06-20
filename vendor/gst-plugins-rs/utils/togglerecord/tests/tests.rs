// Copyright (C) 2017 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use either::*;

use std::sync::{mpsc, Mutex};
use std::{thread, time::Duration};

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gsttogglerecord::plugin_register_static().expect("gsttogglerecord tests");
    });
}

enum SendData {
    Buffers(usize),
    BuffersDelta(usize),
    Gaps(usize),
    Eos,
    Terminate,
}

#[allow(clippy::type_complexity)]
fn setup_sender_receiver(
    pipeline: &gst::Pipeline,
    togglerecord: &gst::Element,
    pad: &str,
    offset: gst::ClockTime,
    live: bool,
) -> (
    mpsc::Sender<SendData>,
    mpsc::Receiver<()>,
    mpsc::Receiver<Either<gst::Buffer, gst::Event>>,
    thread::JoinHandle<()>,
) {
    let fakesink = gst::ElementFactory::make("fakesink")
        .property("async", false)
        .build()
        .unwrap();
    pipeline.add(&fakesink).unwrap();

    let main_stream = pad == "src";

    let (srcpad, sinkpad) = if main_stream {
        (
            togglerecord.static_pad("src").unwrap(),
            togglerecord.static_pad("sink").unwrap(),
        )
    } else {
        let sinkpad = togglerecord.request_pad_simple("sink_%u").unwrap();
        let srcpad = sinkpad.iterate_internal_links().next().unwrap().unwrap();
        (srcpad, sinkpad)
    };

    sinkpad.add_probe(
        gst::PadProbeType::QUERY_UPSTREAM,
        move |_pad, probe_info| {
            let Some(query) = probe_info.query_mut() else {
                unreachable!();
            };

            match query.view_mut() {
                gst::QueryViewMut::Latency(q) => {
                    q.set(live, gst::ClockTime::ZERO, None);
                    gst::PadProbeReturn::Handled
                }
                _ => gst::PadProbeReturn::Ok,
            }
        },
    );

    let fakesink_sinkpad = fakesink.static_pad("sink").unwrap();
    srcpad.link(&fakesink_sinkpad).unwrap();

    let (sender_output, receiver_output) = mpsc::channel::<Either<gst::Buffer, gst::Event>>();
    let sender_output = Mutex::new(sender_output);
    srcpad.add_probe(
        gst::PadProbeType::BUFFER | gst::PadProbeType::EVENT_DOWNSTREAM,
        move |_, ref probe_info| {
            match probe_info.data {
                Some(gst::PadProbeData::Buffer(ref buffer)) => {
                    sender_output
                        .lock()
                        .unwrap()
                        .send(Left(buffer.clone()))
                        .unwrap();
                }
                Some(gst::PadProbeData::Event(ref event)) => {
                    sender_output
                        .lock()
                        .unwrap()
                        .send(Right(event.clone()))
                        .unwrap();
                }
                _ => {
                    unreachable!();
                }
            }

            gst::PadProbeReturn::Ok
        },
    );

    let (sender_input, receiver_input) = mpsc::channel::<SendData>();
    let (sender_input_done, receiver_input_done) = mpsc::channel::<()>();
    let thread = thread::spawn(move || {
        let mut i = 0;
        let mut first = true;
        while let Ok(send_data) = receiver_input.recv() {
            if first {
                assert!(sinkpad.send_event(gst::event::StreamStart::new("test")));
                let caps = if main_stream {
                    gst_video::VideoCapsBuilder::new()
                        .format(gst_video::VideoFormat::Argb)
                        .width(320)
                        .height(240)
                        .framerate(50.into())
                        .build()
                } else {
                    gst_audio::AudioCapsBuilder::new_interleaved()
                        .format(gst_audio::AUDIO_FORMAT_U8)
                        .rate(800)
                        .channels(1)
                        .build()
                };
                assert!(sinkpad.send_event(gst::event::Caps::new(&caps)));

                let segment = gst::FormattedSegment::<gst::ClockTime>::new();
                assert!(sinkpad.send_event(gst::event::Segment::new(&segment)));

                let mut tags = gst::TagList::new();
                tags.get_mut()
                    .unwrap()
                    .add::<gst::tags::Title>(&"some title", gst::TagMergeMode::Append);
                assert!(sinkpad.send_event(gst::event::Tag::new(tags)));

                first = false;
            }

            let buffer = if main_stream {
                gst::Buffer::with_size(320 * 240 * 4).unwrap()
            } else {
                gst::Buffer::with_size(160).unwrap()
            };

            match send_data {
                SendData::Eos => {
                    break;
                }
                SendData::Buffers(n) => {
                    for _ in 0..n {
                        let mut buffer = buffer.clone();
                        {
                            let buffer = buffer.make_mut();
                            buffer.set_pts(offset + i * 20.mseconds());
                            buffer.set_duration(20.mseconds());
                        }
                        let _ = sinkpad.chain(buffer);
                        i += 1;
                    }
                }
                SendData::BuffersDelta(n) => {
                    for _ in 0..n {
                        let mut buffer = gst::Buffer::new();
                        buffer
                            .get_mut()
                            .unwrap()
                            .set_pts(offset + i * 20.mseconds());
                        buffer.get_mut().unwrap().set_duration(20.mseconds());
                        buffer
                            .get_mut()
                            .unwrap()
                            .set_flags(gst::BufferFlags::DELTA_UNIT);
                        let _ = sinkpad.chain(buffer);
                        i += 1;
                    }
                }
                SendData::Gaps(n) => {
                    for _ in 0..n {
                        let event = gst::event::Gap::new(offset + i * 20.mseconds(), 20.mseconds());
                        let _ = sinkpad.send_event(event);
                        i += 1;
                    }
                }
                SendData::Terminate => {
                    let _ = sender_input_done.send(());
                    return;
                }
            }

            let _ = sender_input_done.send(());
        }

        let _ = sinkpad.send_event(gst::event::Eos::new());
        let _ = sender_input_done.send(());
    });

    (sender_input, receiver_input_done, receiver_output, thread)
}

#[allow(clippy::type_complexity)]
fn recv_buffers(
    receiver_output: &mpsc::Receiver<Either<gst::Buffer, gst::Event>>,
    segment: &mut gst::FormattedSegment<gst::ClockTime>,
    wait_buffers: usize,
) -> (
    Vec<(
        Option<gst::ClockTime>,
        Option<gst::ClockTime>,
        Option<gst::ClockTime>,
    )>,
    bool,
) {
    let mut res = Vec::new();
    let mut n_buffers = 0;
    let mut saw_eos = false;
    while let Ok(val) = receiver_output.recv() {
        match val {
            Left(buffer) => {
                res.push((
                    segment.to_running_time(buffer.pts()),
                    buffer.pts(),
                    buffer.duration(),
                ));
                n_buffers += 1;
                if wait_buffers > 0 && n_buffers == wait_buffers {
                    return (res, saw_eos);
                }
            }
            Right(event) => {
                use gst::EventView;

                match event.view() {
                    EventView::Gap(e) => {
                        let (ts, duration) = e.get();

                        res.push((segment.to_running_time(ts), Some(ts), duration));
                        n_buffers += 1;
                        if wait_buffers > 0 && n_buffers == wait_buffers {
                            return (res, saw_eos);
                        }
                    }
                    EventView::Eos(..) => {
                        saw_eos = true;
                        return (res, saw_eos);
                    }
                    EventView::Segment(e) => {
                        *segment = e.segment().clone().downcast().unwrap();
                    }
                    _ => (),
                }
            }
        }
    }

    (res, saw_eos)
}

#[test]
fn test_create() {
    init();
    assert!(gst::ElementFactory::make("togglerecord").build().is_ok());
}

#[test]
fn test_create_pads() {
    init();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();

    let sinkpad = togglerecord.request_pad_simple("sink_%u").unwrap();
    let srcpad = sinkpad.iterate_internal_links().next().unwrap().unwrap();

    assert_eq!(sinkpad.name(), "sink_0");
    assert_eq!(srcpad.name(), "src_0");

    togglerecord.release_request_pad(&sinkpad);
    assert!(sinkpad.parent().is_none());
    assert!(srcpad.parent().is_none());
}

#[test]
fn test_one_stream_open_nonlivein_nonliveout() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", false)
        .build()
        .unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, _, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_open_nonlivein_liveout() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", true)
        .build()
        .unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, _, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, _, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_gaps_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, _, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(5)).unwrap();
    sender_input.send(SendData::Gaps(5)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_close_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, receiver_input_done, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);

    pipeline.set_state(gst::State::Playing).unwrap();

    sender_input.send(SendData::Buffers(10)).unwrap();
    receiver_input_done.recv().unwrap();
    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), (10 + index) * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_open_close() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, receiver_input_done, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    receiver_input_done.recv().unwrap();
    togglerecord.set_property("record", false);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 10);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_one_stream_open_close_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input, receiver_input_done, receiver_output, thread) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    receiver_input_done.recv().unwrap();
    togglerecord.set_property("record", false);
    sender_input.send(SendData::Buffers(10)).unwrap();
    receiver_input_done.recv().unwrap();
    togglerecord.set_property("record", true);
    sender_input.send(SendData::Buffers(10)).unwrap();
    drop(sender_input);

    let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers, _) = recv_buffers(&receiver_output, &mut segment, 0);
    assert_eq!(buffers.len(), 20);
    for (index, &(running_time, pts, duration)) in buffers.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }

    thread.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_shift() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(&pipeline, &togglerecord, "src_%u", 5.mseconds(), true);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // Second to last buffer should be clipped from second stream, last should be dropped
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), 5.mseconds() + index * 20.mseconds());
        assert_eq!(pts.unwrap(), 5.mseconds() + index * 20.mseconds());
        if index == 9 {
            assert_eq!(duration.unwrap(), 15.mseconds());
        } else {
            assert_eq!(duration.unwrap(), 20.mseconds());
        }
    }
    assert_eq!(buffers_2.len(), 10);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_shift_main() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", 5.mseconds(), true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(12)).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // PTS 5 maps to running time 0 now
    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), 5.mseconds() + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // First and second last buffer should be clipped from second stream,
    // last buffer should be dropped
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        if index == 0 {
            assert_eq!(running_time.unwrap(), index * 20.mseconds());
            assert_eq!(pts.unwrap(), 5.mseconds() + index * 20.mseconds());
            assert_eq!(duration.unwrap(), 15.mseconds());
        } else if index == 10 {
            assert_eq!(running_time.unwrap(), index * 20.mseconds() - 5.mseconds());
            assert_eq!(pts.unwrap(), index * 20.mseconds());
            assert_eq!(duration.unwrap(), 5.mseconds());
        } else {
            assert_eq!(running_time.unwrap(), index * 20.mseconds() - 5.mseconds());
            assert_eq!(pts.unwrap(), index * 20.mseconds());
            assert_eq!(duration.unwrap(), 20.mseconds());
        }
    }
    assert_eq!(buffers_2.len(), 11);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_close() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv().unwrap();

    // Stop recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_close_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", false);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv().unwrap();

    // Start recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), (10 + index) * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), (10 + index) * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_close_open_nonlivein_nonliveout() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", false)
        .property("record", false)
        .build()
        .unwrap();
    pipeline.add(&togglerecord).unwrap();

    let main_buffers_in_gap = 10u64;
    let secondary_buffers_in_gap = main_buffers_in_gap + 1;
    let main_buffers_after_gap = 10u64;
    let secondary_buffers_after_gap = 9u64;
    let recv_timeout = Duration::from_secs(10);

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            false,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    sender_input_1
        .send(SendData::Buffers(main_buffers_in_gap as usize))
        .unwrap();
    assert_eq!(
        receiver_input_done_1.recv_timeout(Duration::from_millis(20)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    sender_input_2
        .send(SendData::Buffers((secondary_buffers_in_gap) as usize))
        .unwrap();
    assert_eq!(
        receiver_input_done_2.recv_timeout(Duration::from_millis(20)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Start recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", true);
    sender_input_1
        .send(SendData::Buffers(main_buffers_after_gap as usize))
        .unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2
        .send(SendData::Buffers(secondary_buffers_after_gap as usize))
        .unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_1.len() as u64,
        main_buffers_in_gap + main_buffers_in_gap
    );

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_2.len() as u64,
        secondary_buffers_in_gap + secondary_buffers_after_gap
    );

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/513
#[ignore]
fn test_two_stream_close_open_nonlivein_liveout() {
    init();

    let testclock = gst_check::TestClock::new();
    let pipeline = gst::Pipeline::default();
    pipeline.use_clock(Some(&testclock));
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", true)
        .property("record", false)
        .build()
        .unwrap();
    togglerecord.set_clock(Some(&testclock)).unwrap();
    pipeline.add(&togglerecord).unwrap();
    let testclock = testclock.downcast::<gst_check::TestClock>().unwrap();
    testclock.set_time(gst::ClockTime::ZERO);

    let main_buffers_in_gap = 10u64;
    let secondary_buffers_in_gap = main_buffers_in_gap + 1;
    let main_buffers_after_gap = 10u64;
    let secondary_buffers_after_gap = 9u64;
    let recv_timeout = Duration::from_secs(10);

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            false,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    sender_input_1
        .send(SendData::Buffers(main_buffers_in_gap as usize))
        .unwrap();
    assert_eq!(
        receiver_input_done_1.recv_timeout(Duration::from_millis(60)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    sender_input_2
        .send(SendData::Buffers((secondary_buffers_in_gap) as usize))
        .unwrap();
    assert_eq!(
        receiver_input_done_2.recv_timeout(Duration::from_millis(60)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Advance the clock
    let block_time = gst::ClockTime::from_mseconds(42);
    testclock.advance_time(block_time.nseconds() as i64);

    // Start recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", true);
    sender_input_1
        .send(SendData::Buffers(main_buffers_after_gap as usize))
        .unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2
        .send(SendData::Buffers(secondary_buffers_after_gap as usize))
        .unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), block_time + index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_1.len() as u64,
        main_buffers_in_gap + main_buffers_in_gap
    );

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), block_time + index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_2.len() as u64,
        secondary_buffers_in_gap + secondary_buffers_after_gap
    );

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_close_open_nonlivein_liveout() {
    init();

    let testclock = gst_check::TestClock::new();
    let pipeline = gst::Pipeline::default();
    pipeline.use_clock(Some(&testclock));
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", true)
        .build()
        .unwrap();
    togglerecord.set_clock(Some(&testclock)).unwrap();
    pipeline.add(&togglerecord).unwrap();
    let testclock = testclock.downcast::<gst_check::TestClock>().unwrap();
    testclock.set_time(gst::ClockTime::ZERO);

    let main_buffers_before_gap = 10u64;
    let secondary_buffers_before_gap = main_buffers_before_gap + 1;
    let buffers_in_gap = 10u64;
    let buffers_after_gap = 10u64;
    let recv_timeout = Duration::from_secs(10);

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            false,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1
        .send(SendData::Buffers(main_buffers_before_gap as usize))
        .unwrap();
    sender_input_2
        .send(SendData::Buffers(
            (secondary_buffers_before_gap - 1) as usize,
        ))
        .unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();
    sender_input_2.send(SendData::Buffers(1)).unwrap();
    assert_eq!(
        receiver_input_done_2.recv_timeout(Duration::from_millis(20)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Stop recording and push new buffers to sender 1, this will block
    togglerecord.set_property("record", false);
    sender_input_1
        .send(SendData::Buffers(buffers_in_gap as usize))
        .unwrap();

    // Send another 10 buffers to sender 2, both are the same position at 9 buffers, the next one
    // will block until record=true
    sender_input_2
        .send(SendData::Buffers(buffers_in_gap as usize))
        .unwrap();

    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();
    // Advance the clock
    let block_time = gst::ClockTime::from_mseconds(42);
    testclock.advance_time(block_time.nseconds() as i64);

    // Start recording again and send another set of buffers to both senders
    togglerecord.set_property("record", true);
    sender_input_1
        .send(SendData::Buffers(buffers_after_gap as usize))
        .unwrap();
    sender_input_2
        .send(SendData::Buffers(buffers_after_gap as usize))
        .unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(
        &receiver_output_1,
        &mut segment_1,
        main_buffers_before_gap as usize,
    );
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(
            running_time.unwrap(),
            block_time + (index + main_buffers_before_gap) * 20.mseconds()
        );
        assert_eq!(
            pts.unwrap(),
            (index + main_buffers_before_gap) * 20.mseconds()
        );
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_1.len(),
        (buffers_in_gap + buffers_after_gap) as usize
    );

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(
        &receiver_output_2,
        &mut segment_2,
        secondary_buffers_before_gap as usize,
    );
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(
            running_time.unwrap(),
            block_time + (index + secondary_buffers_before_gap) * 20.mseconds()
        );
        assert_eq!(
            pts.unwrap(),
            (index + secondary_buffers_before_gap) * 20.mseconds()
        );
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(
        buffers_2.len(),
        (buffers_in_gap + buffers_after_gap - 1) as usize
    );

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}
#[test]
fn test_two_stream_open_close_open_nonlivein_nonliveout() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord")
        .property("is-live", false)
        .build()
        .unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, false);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            false,
        );

    let recv_timeout = Duration::from_secs(10);

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();

    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    sender_input_2.send(SendData::Buffers(1)).unwrap();
    assert_eq!(
        receiver_input_done_2.recv_timeout(Duration::from_millis(20)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    // Stop recording and push new buffers to sender 1, this will block
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::Buffers(10)).unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();

    // Send another buffer to sender 2, this will block until record=true
    sender_input_2.send(SendData::Buffers(1)).unwrap();

    // Start recording again and send another set of buffers to both senders
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();
    receiver_input_done_2.recv_timeout(recv_timeout).unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 30);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 30);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_close_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv().unwrap();

    // Stop recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another buffer to sender 2, this will block until sender 1 advances
    // but must not be dropped, although we're not recording (yet)
    sender_input_2.send(SendData::Buffers(1)).unwrap();

    // Start recording again and send another set of buffers to both senders
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_1.recv().unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 20);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 20);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_open_close_open_gaps() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(3)).unwrap();
    sender_input_1.send(SendData::Gaps(3)).unwrap();
    sender_input_1.send(SendData::Buffers(4)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_1.recv().unwrap();

    // Stop recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2 above
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another 4 gaps and 5 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Gaps(4)).unwrap();
    sender_input_2.send(SendData::Buffers(5)).unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another gap to sender 2, this will block until sender 1 advances
    // but must not be dropped, although we're not recording (yet)
    sender_input_2.send(SendData::Gaps(1)).unwrap();

    // Start recording again and send another set of buffers to both senders
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_1.recv().unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 20);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 20);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_close_open_close_delta() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", false);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();

    // Sender 2 is waiting for sender 1 to continue, sender 1 is finished
    receiver_input_done_1.recv().unwrap();

    // Start recording and push new buffers to sender 1. The first one is a delta frame,
    // so will be dropped, and as such the next frame of sender 2 will also be dropped
    // Sender 2 is empty now
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::BuffersDelta(1)).unwrap();
    sender_input_1.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another 9 buffers to sender 2, both are the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();

    // Wait until all 20 buffers of both senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send another buffer to sender 2, this will block until sender 1 advances
    // but must not be dropped, and we're still recording
    sender_input_2.send(SendData::Buffers(1)).unwrap();

    // Stop recording again and send another set of buffers to both senders
    // The first one is a delta frame, so we only actually stop recording
    // after recording another frame
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::BuffersDelta(1)).unwrap();
    sender_input_1.send(SendData::Buffers(9)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_1.recv().unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), (11 + index) * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), (11 + index) * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_three_stream_open_close_open() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );
    let (sender_input_3, receiver_input_done_3, receiver_output_3, thread_3) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(11)).unwrap();
    sender_input_3.send(SendData::Buffers(11)).unwrap();

    // Sender 2/3 is waiting for sender 1 to continue
    receiver_input_done_1.recv().unwrap();

    // Stop recording and push new buffers to sender 1, which will advance
    // it and release the 11th buffer of sender 2/3 above
    togglerecord.set_property("record", false);
    sender_input_1.send(SendData::Buffers(10)).unwrap();

    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    // Send another 9 buffers to sender 2/3, all streams are at the same position now
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    sender_input_3.send(SendData::Buffers(9)).unwrap();

    // Wait until all 20 buffers of all senders are done
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    // Send another buffer to sender 2, this will block until sender 1 advances
    // but must not be dropped, although we're not recording (yet)
    sender_input_2.send(SendData::Buffers(1)).unwrap();

    // Start recording again and send another set of buffers to both senders
    togglerecord.set_property("record", true);
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(10)).unwrap();
    sender_input_3.send(SendData::Buffers(5)).unwrap();
    receiver_input_done_1.recv().unwrap();
    // The single buffer above for sender 1 should be handled now
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    sender_input_3.send(SendData::Buffers(5)).unwrap();
    receiver_input_done_3.recv().unwrap();

    // Send EOS and wait for it to be handled
    sender_input_1.send(SendData::Eos).unwrap();
    sender_input_2.send(SendData::Eos).unwrap();
    sender_input_3.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, _) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 20);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, _) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 20);

    let mut segment_3 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_3, _) = recv_buffers(&receiver_output_3, &mut segment_3, 0);
    for (index, &(running_time, pts, duration)) in buffers_3.iter().enumerate() {
        let pts_off = if index >= 10 {
            10 * 20.mseconds()
        } else {
            gst::ClockTime::ZERO
        };

        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), pts_off + index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_3.len(), 20);

    thread_1.join().unwrap();
    thread_2.join().unwrap();
    thread_3.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_main_eos() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    // Send 10 buffers to main stream first
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // And send EOS, at this moment, recording state should be still recording
    // since running time of main stream is advanced than secondary stream
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // Send 2 buffers to secondary stream. At this moment, main stream got eos
    // already (after 10 buffers) and secondary stream got 2 buffers.
    // it will make running time of secondary stream to be advanced than main
    // stream and results in all-eos state even if we don't send EOS event
    // explicitly
    sender_input_2.send(SendData::Buffers(2)).unwrap();
    receiver_input_done_2.recv().unwrap();
    sender_input_2.send(SendData::Terminate).unwrap();
    receiver_input_done_2.recv().unwrap();

    // At this moment, all streams should be in eos state. So togglerecord
    // must be in stopped state
    let recording = togglerecord.property::<bool>("recording");
    assert!(!recording);

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, saw_eos) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);
    assert!(saw_eos);

    // Last buffer should be dropped from second stream
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, saw_eos) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);
    assert!(saw_eos);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_two_stream_secondary_eos_first() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    // Send 10 buffers to main stream first
    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();

    // Send EOS to the second stream
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();

    // Since main stream is not yet EOS state, we should be in recording state
    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // And send EOS to the main stream then it will update state to Stopped
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(!recording);

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, saw_eos) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);
    assert!(saw_eos);

    // We sent 9 buffers to the second stream, and there should be no dropped
    // buffer
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, saw_eos) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 9);
    assert!(saw_eos);

    thread_1.join().unwrap();
    thread_2.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_three_stream_main_eos() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );
    let (sender_input_3, receiver_input_done_3, receiver_output_3, thread_3) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    sender_input_3.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    // And send EOS, at this moment, recording state should be still recording
    // since running time of the second stream is not in eos state
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // Send 2 buffers to non-main streams. At this moment, main stream got EOS
    // already (after 10 buffers) and the other streams got 9 buffers.
    // So those 2 additional buffers to the non-main streams will make running
    // times of those streams to be advanced than main stream.
    // It will result in all-eos state even if we don't send EOS event
    // to the non-main streams explicitly
    sender_input_2.send(SendData::Buffers(2)).unwrap();
    receiver_input_done_2.recv().unwrap();
    sender_input_2.send(SendData::Terminate).unwrap();
    receiver_input_done_2.recv().unwrap();

    // The third stream is not in EOS state yet, so still recording == true
    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // And terminate the third thread without EOS
    sender_input_3.send(SendData::Buffers(2)).unwrap();
    receiver_input_done_3.recv().unwrap();
    sender_input_3.send(SendData::Terminate).unwrap();
    receiver_input_done_3.recv().unwrap();

    // At this moment, all streams should be in eos state. So togglerecord
    // must be in stopped state
    let recording = togglerecord.property::<bool>("recording");
    assert!(!recording);

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, saw_eos) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);
    assert!(saw_eos);

    // Last buffer should be dropped from non-main streams
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, saw_eos) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 10);
    assert!(saw_eos);

    let mut segment_3 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_3, saw_eos) = recv_buffers(&receiver_output_3, &mut segment_3, 0);
    for (index, &(running_time, pts, duration)) in buffers_3.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_3.len(), 10);
    assert!(saw_eos);

    thread_1.join().unwrap();
    thread_2.join().unwrap();
    thread_3.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_three_stream_main_and_second_eos() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );
    let (sender_input_3, receiver_input_done_3, receiver_output_3, thread_3) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    sender_input_3.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    // And send EOS, at this moment, recording state should be still recording
    // since running time of the third stream is not in eos state
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // And send EOS to the second stream, but state shouldn't be affected by
    // this EOS. The third stream is still not in EOS state
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // Send 2 buffers to the third stream. At this moment, main stream and
    // the second stream got EOS already (after 10 buffers) and the third stream
    // got 9 buffers.
    // So those 2 additional buffers to the third streams will make running
    // time of the stream to be advanced than main stream.
    // It will result in all-eos state even if we don't send EOS event
    // to the third stream explicitly
    sender_input_3.send(SendData::Buffers(2)).unwrap();
    receiver_input_done_3.recv().unwrap();
    sender_input_3.send(SendData::Terminate).unwrap();
    receiver_input_done_3.recv().unwrap();

    // At this moment, all streams should be in eos state. So togglerecord
    // must be in stopped state
    let recording = togglerecord.property::<bool>("recording");
    assert!(!recording);

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, saw_eos) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);
    assert!(saw_eos);

    // We sent 9 buffers to the second stream, and there must be no dropped one
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, saw_eos) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 9);
    assert!(saw_eos);

    // Last buffer should be dropped from the third stream
    let mut segment_3 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_3, saw_eos) = recv_buffers(&receiver_output_3, &mut segment_3, 0);
    for (index, &(running_time, pts, duration)) in buffers_3.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_3.len(), 10);
    assert!(saw_eos);

    thread_1.join().unwrap();
    thread_2.join().unwrap();
    thread_3.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}

#[test]
fn test_three_stream_secondary_eos_first() {
    init();

    let pipeline = gst::Pipeline::default();
    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();
    pipeline.add(&togglerecord).unwrap();

    let (sender_input_1, receiver_input_done_1, receiver_output_1, thread_1) =
        setup_sender_receiver(&pipeline, &togglerecord, "src", gst::ClockTime::ZERO, true);
    let (sender_input_2, receiver_input_done_2, receiver_output_2, thread_2) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );
    let (sender_input_3, receiver_input_done_3, receiver_output_3, thread_3) =
        setup_sender_receiver(
            &pipeline,
            &togglerecord,
            "src_%u",
            gst::ClockTime::ZERO,
            true,
        );

    pipeline.set_state(gst::State::Playing).unwrap();

    togglerecord.set_property("record", true);

    sender_input_1.send(SendData::Buffers(10)).unwrap();
    sender_input_2.send(SendData::Buffers(9)).unwrap();
    sender_input_3.send(SendData::Buffers(9)).unwrap();
    receiver_input_done_1.recv().unwrap();
    receiver_input_done_2.recv().unwrap();
    receiver_input_done_3.recv().unwrap();

    // Send EOS to non-main streams
    sender_input_2.send(SendData::Eos).unwrap();
    receiver_input_done_2.recv().unwrap();

    sender_input_3.send(SendData::Eos).unwrap();
    receiver_input_done_3.recv().unwrap();

    // Since main stream is not yet EOS state, we should be in recording state
    let recording = togglerecord.property::<bool>("recording");
    assert!(recording);

    // And send EOS, Send EOS to the main stream then it will update state to
    // Stopped
    sender_input_1.send(SendData::Eos).unwrap();
    receiver_input_done_1.recv().unwrap();

    let recording = togglerecord.property::<bool>("recording");
    assert!(!recording);

    let mut segment_1 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_1, saw_eos) = recv_buffers(&receiver_output_1, &mut segment_1, 0);
    for (index, &(running_time, pts, duration)) in buffers_1.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_1.len(), 10);
    assert!(saw_eos);

    // Last buffer should be dropped from non-main streams
    let mut segment_2 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_2, saw_eos) = recv_buffers(&receiver_output_2, &mut segment_2, 0);
    for (index, &(running_time, pts, duration)) in buffers_2.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_2.len(), 9);
    assert!(saw_eos);

    let mut segment_3 = gst::FormattedSegment::<gst::ClockTime>::new();
    let (buffers_3, saw_eos) = recv_buffers(&receiver_output_3, &mut segment_3, 0);
    for (index, &(running_time, pts, duration)) in buffers_3.iter().enumerate() {
        let index = index as u64;
        assert_eq!(running_time.unwrap(), index * 20.mseconds());
        assert_eq!(pts.unwrap(), index * 20.mseconds());
        assert_eq!(duration.unwrap(), 20.mseconds());
    }
    assert_eq!(buffers_3.len(), 9);
    assert!(saw_eos);

    thread_1.join().unwrap();
    thread_2.join().unwrap();
    thread_3.join().unwrap();

    pipeline.set_state(gst::State::Null).unwrap();
}
