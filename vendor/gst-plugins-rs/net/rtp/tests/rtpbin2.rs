//
// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::sync::{atomic::AtomicUsize, Arc, Mutex};

use gst::{prelude::*, Caps};
use gst_check::Harness;
use rtp_types::*;

static ELEMENT_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_element_counter() -> usize {
    ELEMENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsrtp::plugin_register_static().expect("rtpbin2 test");
    });
}

const TEST_SSRC: u32 = 0x12345678;
const TEST_PT: u8 = 96;
const TEST_CLOCK_RATE: u32 = 48000;

fn generate_rtp_buffer(seqno: u16, rtpts: u32, payload_len: usize) -> gst::Buffer {
    let payload = vec![4; payload_len];
    let packet = RtpPacketBuilder::new()
        .ssrc(TEST_SSRC)
        .payload_type(TEST_PT)
        .sequence_number(seqno)
        .timestamp(rtpts)
        .payload(payload.as_slice());
    let size = packet.calculate_size().unwrap();
    let mut data = vec![0; size];
    packet.write_into(&mut data).unwrap();
    gst::Buffer::from_mut_slice(data)
}

#[derive(Debug, Copy, Clone)]
struct PacketInfo {
    seq_no: u16,
    rtp_ts: u32,
    payload_len: usize,
}

impl PacketInfo {
    fn generate_buffer(&self, dts: Option<gst::ClockTime>) -> gst::Buffer {
        let mut buf = generate_rtp_buffer(self.seq_no, self.rtp_ts, self.payload_len);
        let buf_mut = buf.make_mut();
        buf_mut.set_dts(dts);
        buf
    }
}

fn send_init() -> gst_check::Harness {
    init();

    let id = next_element_counter();

    let elem = gst::ElementFactory::make("rtpsend")
        .property("rtp-id", id.to_string())
        .build()
        .unwrap();
    let mut h = Harness::with_element(&elem, Some("rtp_sink_0"), Some("rtp_src_0"));
    h.play();

    let caps = Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", TEST_PT as i32)
        .field("clock-rate", TEST_CLOCK_RATE as i32)
        .field("encoding-name", "custom-test")
        .build();
    h.set_src_caps(caps);

    h
}

fn send_push<I>(h: &mut gst_check::Harness, packets: I, buffer_list: bool)
where
    I: IntoIterator<Item = PacketInfo>,
{
    if buffer_list {
        let mut list = gst::BufferList::new();
        let list_mut = list.make_mut();
        for packet in packets {
            list_mut.add(packet.generate_buffer(None));
        }
        let push_pad = h
            .element()
            .unwrap()
            .static_pad("rtp_sink_0")
            .unwrap()
            .peer()
            .unwrap();
        push_pad.push_list(list).unwrap();
    } else {
        for packet in packets {
            h.push(packet.generate_buffer(None)).unwrap();
        }
    }
}

fn send_pull<I>(h: &mut gst_check::Harness, packets: I)
where
    I: IntoIterator<Item = PacketInfo>,
{
    for packet in packets {
        let buffer = h.pull().unwrap();
        let mapped = buffer.map_readable().unwrap();
        let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
        assert_eq!(rtp.sequence_number(), packet.seq_no);
    }
}

fn send_check_stats<I>(h: &mut gst_check::Harness, packets: I)
where
    I: IntoIterator<Item = PacketInfo>,
{
    let mut n_packets = 0;
    let mut n_bytes = 0;
    for packet in packets {
        n_packets += 1;
        n_bytes += packet.payload_len;
    }
    let stats = h.element().unwrap().property::<gst::Structure>("stats");

    let session_stats = stats.get::<gst::Structure>("0").unwrap();
    let source_stats = session_stats
        .get::<gst::Structure>(TEST_SSRC.to_string())
        .unwrap();
    assert_eq!(source_stats.get::<u32>("ssrc").unwrap(), TEST_SSRC);
    assert_eq!(
        source_stats.get::<u32>("clock-rate").unwrap(),
        TEST_CLOCK_RATE
    );
    assert!(source_stats.get::<bool>("sender").unwrap());
    assert!(source_stats.get::<bool>("local").unwrap());
    assert_eq!(source_stats.get::<u64>("packets-sent").unwrap(), n_packets);
    assert_eq!(
        source_stats.get::<u64>("octets-sent").unwrap(),
        n_bytes as u64
    );
}

#[test]
fn test_send() {
    init();

    let mut h = send_init();

    send_push(&mut h, PACKETS_TEST_1, false);
    send_pull(&mut h, PACKETS_TEST_1);
    send_check_stats(&mut h, PACKETS_TEST_1);
}

#[test]
fn test_send_list() {
    let mut h = send_init();

    send_push(&mut h, PACKETS_TEST_1, true);
    send_pull(&mut h, PACKETS_TEST_1);
    send_check_stats(&mut h, PACKETS_TEST_1);
}

#[test]
fn test_send_benchmark() {
    init();
    let clock = gst::SystemClock::obtain();
    const N_PACKETS: usize = 2 * 1024 * 1024;
    let mut packets = Vec::with_capacity(N_PACKETS);
    for i in 0..N_PACKETS {
        packets.push(
            PacketInfo {
                seq_no: (i % u16::MAX as usize) as u16,
                rtp_ts: i as u32,
                payload_len: 8,
            }
            .generate_buffer(None),
        )
    }

    let mut h = send_init();

    let start = clock.time();

    for packet in packets {
        h.push(packet).unwrap();
    }

    let pushed = clock.time();

    for i in 0..N_PACKETS {
        let buffer = h.pull().unwrap();
        let mapped = buffer.map_readable().unwrap();
        let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
        assert_eq!(rtp.sequence_number(), (i % u16::MAX as usize) as u16);
    }

    let end = clock.time();

    let test_time = end.opt_sub(start);
    let pull_time = end.opt_sub(pushed);
    let push_time = pushed.opt_sub(start);
    println!(
        "test took {} (push {}, pull {})",
        test_time.display(),
        push_time.display(),
        pull_time.display()
    );
}

#[test]
fn test_send_list_benchmark() {
    init();
    let clock = gst::SystemClock::obtain();
    const N_PACKETS: usize = 2 * 1024 * 1024;
    let mut list = gst::BufferList::new();
    let list_mut = list.make_mut();
    for i in 0..N_PACKETS {
        list_mut.add(
            PacketInfo {
                seq_no: (i % u16::MAX as usize) as u16,
                rtp_ts: i as u32,
                payload_len: 8,
            }
            .generate_buffer(None),
        );
    }
    let mut h = send_init();

    let push_pad = h
        .element()
        .unwrap()
        .static_pad("rtp_sink_0")
        .unwrap()
        .peer()
        .unwrap();

    let start = clock.time();

    push_pad.push_list(list).unwrap();

    let pushed = clock.time();

    for i in 0..N_PACKETS {
        let buffer = h.pull().unwrap();
        let mapped = buffer.map_readable().unwrap();
        let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
        assert_eq!(rtp.sequence_number(), (i % u16::MAX as usize) as u16);
    }

    let end = clock.time();

    let test_time = end.opt_sub(start);
    let pull_time = end.opt_sub(pushed);
    let push_time = pushed.opt_sub(start);
    println!(
        "test took {} (push {}, pull {})",
        test_time.display(),
        push_time.display(),
        pull_time.display()
    );
}

fn receive_init() -> Arc<Mutex<gst_check::Harness>> {
    receive_init_with_new_srcpad(|h, srcpad| h.add_element_src_pad(srcpad))
}

fn receive_init_with_new_srcpad<F>(new_srcpad: F) -> Arc<Mutex<gst_check::Harness>>
where
    F: Fn(&mut gst_check::Harness, &gst::Pad) + Send + Sync + 'static,
{
    let id = next_element_counter();

    let elem = gst::ElementFactory::make("rtprecv")
        .property("rtp-id", id.to_string())
        .build()
        .unwrap();
    let h = Arc::new(Mutex::new(Harness::with_element(
        &elem,
        Some("rtp_sink_0"),
        None,
    )));
    let weak_h = Arc::downgrade(&h);
    let mut inner = h.lock().unwrap();
    inner
        .element()
        .unwrap()
        .connect_pad_added(move |_elem, pad| {
            let h = weak_h.upgrade().unwrap();
            let mut h = h.lock().unwrap();
            new_srcpad(&mut h, pad);
        });
    inner.play();
    let caps = Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", TEST_PT as i32)
        .field("clock-rate", TEST_CLOCK_RATE as i32)
        .field("encoding-name", "custom-test")
        .build();
    inner.set_src_caps(caps);

    drop(inner);

    h
}

fn receive_push<I>(h: Arc<Mutex<gst_check::Harness>>, packets: I, buffer_list: bool)
where
    I: IntoIterator<Item = PacketInfo>,
{
    let inner = h.lock().unwrap();
    // Cannot push with harness lock as the 'pad-added' handler needs to add the newly created pad to
    // the harness and needs to also take the harness lock.  Workaround by pushing from the
    // internal harness pad directly.
    let push_pad = inner
        .element()
        .unwrap()
        .static_pad("rtp_sink_0")
        .unwrap()
        .peer()
        .unwrap();
    drop(inner);

    if buffer_list {
        let mut list = gst::BufferList::new();
        let list_mut = list.make_mut();
        for packet in packets {
            list_mut.add(packet.generate_buffer(None));
        }
        push_pad.push_list(list).unwrap();
    } else {
        for packet in packets {
            push_pad.push(packet.generate_buffer(None)).unwrap();
        }
    }
}

fn receive_pull<I>(h: Arc<Mutex<gst_check::Harness>>, packets: I)
where
    I: IntoIterator<Item = PacketInfo>,
{
    let mut inner = h.lock().unwrap();
    for packet in packets {
        let buffer = inner.pull().unwrap();
        let mapped = buffer.map_readable().unwrap();
        let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
        assert_eq!(rtp.sequence_number(), packet.seq_no);
    }
}

fn receive_check_stats<I>(h: Arc<Mutex<gst_check::Harness>>, packets: I)
where
    I: IntoIterator<Item = PacketInfo>,
{
    let mut n_packets = 0;
    let mut n_bytes = 0;
    for packet in packets {
        n_packets += 1;
        n_bytes += packet.payload_len;
    }

    let inner = h.lock().unwrap();
    let stats = inner.element().unwrap().property::<gst::Structure>("stats");
    drop(inner);

    let session_stats = stats.get::<gst::Structure>("0").unwrap();
    let source_stats = session_stats
        .get::<gst::Structure>(TEST_SSRC.to_string())
        .unwrap();
    let jitterbuffers_stats = session_stats
        .get::<gst::List>("jitterbuffer-stats")
        .unwrap();
    assert_eq!(jitterbuffers_stats.len(), 1);
    let jitterbuffer_stats = jitterbuffers_stats
        .first()
        .unwrap()
        .get::<gst::Structure>()
        .unwrap();
    assert_eq!(source_stats.get::<u32>("ssrc").unwrap(), TEST_SSRC);
    assert_eq!(
        source_stats.get::<u32>("clock-rate").unwrap(),
        TEST_CLOCK_RATE
    );
    assert!(source_stats.get::<bool>("sender").unwrap());
    assert!(!source_stats.get::<bool>("local").unwrap());
    assert_eq!(
        source_stats.get::<u64>("packets-received").unwrap(),
        n_packets
    );
    assert_eq!(
        source_stats.get::<u64>("octets-received").unwrap(),
        n_bytes as u64
    );
    assert_eq!(jitterbuffer_stats.get::<u64>("num-late").unwrap(), 0);
    assert_eq!(jitterbuffer_stats.get::<u64>("num-lost").unwrap(), 0);
    assert_eq!(jitterbuffer_stats.get::<u64>("num-duplicates").unwrap(), 0);
    assert_eq!(
        jitterbuffer_stats.get::<u64>("num-pushed").unwrap(),
        n_packets
    );
    assert_eq!(jitterbuffer_stats.get::<i32>("pt").unwrap(), TEST_PT as i32);
    assert_eq!(
        jitterbuffer_stats.get::<i32>("ssrc").unwrap(),
        TEST_SSRC as i32
    );
}

static PACKETS_TEST_1: [PacketInfo; 2] = [
    PacketInfo {
        seq_no: 500,
        rtp_ts: 20,
        payload_len: 13,
    },
    PacketInfo {
        seq_no: 501,
        rtp_ts: 30,
        payload_len: 7,
    },
];

#[test]
fn test_receive() {
    init();

    let h = receive_init();
    receive_push(h.clone(), PACKETS_TEST_1, false);
    receive_pull(h.clone(), PACKETS_TEST_1);
    receive_check_stats(h, PACKETS_TEST_1);
}

#[test]
fn test_receive_list() {
    init();

    let h = receive_init();
    receive_push(h.clone(), PACKETS_TEST_1, true);
    receive_pull(h.clone(), PACKETS_TEST_1);
    receive_check_stats(h, PACKETS_TEST_1);
}

#[test]
fn test_receive_flush() {
    init();
    let h = receive_init();

    receive_push(h.clone(), PACKETS_TEST_1, false);

    let mut inner = h.lock().unwrap();
    let seqnum = gst::Seqnum::next();
    inner.push_event(gst::event::FlushStart::builder().seqnum(seqnum).build());
    inner.push_event(gst::event::FlushStop::builder(false).seqnum(seqnum).build());

    let event = inner.pull_event().unwrap();
    let gst::EventView::FlushStart(fs) = event.view() else {
        unreachable!();
    };
    assert_eq!(fs.seqnum(), seqnum);
    let event = inner.pull_event().unwrap();
    let gst::EventView::FlushStop(fs) = event.view() else {
        unreachable!();
    };
    assert_eq!(fs.seqnum(), seqnum);
}

#[test]
fn test_receive_benchmark() {
    init();
    let clock = gst::SystemClock::obtain();
    //const N_PACKETS: usize = 1024 * 1024;
    const N_PACKETS: usize = 1024;
    let mut packets = Vec::with_capacity(N_PACKETS);
    for i in 0..N_PACKETS {
        packets.push(
            PacketInfo {
                seq_no: (i % u16::MAX as usize) as u16,
                rtp_ts: i as u32,
                payload_len: 8,
            }
            .generate_buffer(None),
        )
    }

    let h = receive_init();
    let inner = h.lock().unwrap();
    let push_pad = inner
        .element()
        .unwrap()
        .static_pad("rtp_sink_0")
        .unwrap()
        .peer()
        .unwrap();
    drop(inner);

    let start = clock.time();

    for packet in packets {
        push_pad.push(packet).unwrap();
    }

    let pushed = clock.time();

    let mut inner = h.lock().unwrap();
    for i in 0..N_PACKETS {
        let buffer = inner.pull().unwrap();
        let mapped = buffer.map_readable().unwrap();
        let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
        assert_eq!(rtp.sequence_number(), (i % u16::MAX as usize) as u16);
    }

    let end = clock.time();
    drop(inner);

    let test_time = end.opt_sub(start);
    let pull_time = end.opt_sub(pushed);
    let push_time = pushed.opt_sub(start);
    println!(
        "test took {} (push {}, pull {})",
        test_time.display(),
        push_time.display(),
        pull_time.display()
    );
}

#[derive(Debug)]
enum BufferOrList {
    Buffer(gst::Buffer),
    List(gst::BufferList),
}

#[test]
fn test_receive_list_benchmark() {
    init();
    let clock = gst::SystemClock::obtain();
    const N_PACKETS: usize = 32 * 1024;
    const N_PUSHES: usize = 1024 / 32;

    let (send, recv) = std::sync::mpsc::channel();

    let sinkpad = gst::Pad::builder(gst::PadDirection::Sink)
        .name("sink")
        .chain_list_function({
            let send = send.clone();
            move |_pad, _parent, list| {
                send.send(BufferOrList::List(list)).unwrap();
                Ok(gst::FlowSuccess::Ok)
            }
        })
        .chain_function(move |_pad, _parent, buffer| {
            send.send(BufferOrList::Buffer(buffer)).unwrap();

            Ok(gst::FlowSuccess::Ok)
        })
        .build();
    sinkpad.set_active(true).unwrap();

    let h = receive_init_with_new_srcpad(move |_h, srcpad| {
        srcpad.link(&sinkpad).unwrap();
    });
    let inner = h.lock().unwrap();

    let push_pad = inner
        .element()
        .unwrap()
        .static_pad("rtp_sink_0")
        .unwrap()
        .peer()
        .unwrap();
    drop(inner);

    let mut lists = Vec::with_capacity(N_PUSHES);
    for p in 0..N_PUSHES {
        let mut list = gst::BufferList::new();
        let list_mut = list.make_mut();
        for i in 0..N_PACKETS {
            list_mut.add(
                PacketInfo {
                    seq_no: ((p * N_PACKETS + i) % u16::MAX as usize) as u16,
                    rtp_ts: (p * N_PACKETS + i) as u32,
                    payload_len: 8,
                }
                .generate_buffer(None),
            );
        }
        lists.push(list);
    }

    let start = clock.time();

    for list in lists {
        push_pad.push_list(list).unwrap();
    }

    let pushed = clock.time();

    let mut idx = 0;
    loop {
        let list = recv.recv().unwrap();
        println!("{idx} received {list:?}");
        match list {
            BufferOrList::List(list) => {
                list.foreach(|buffer, _buf_idx| {
                    let mapped = buffer.map_readable().unwrap();
                    let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
                    assert_eq!(rtp.sequence_number(), (idx % u16::MAX as usize) as u16);
                    idx += 1;
                    std::ops::ControlFlow::Continue(())
                });
            }
            BufferOrList::Buffer(buffer) => {
                let mapped = buffer.map_readable().unwrap();
                let rtp = rtp_types::RtpPacket::parse(&mapped).unwrap();
                assert_eq!(rtp.sequence_number(), (idx % u16::MAX as usize) as u16);
                idx += 1;
            }
        }
        if idx >= N_PUSHES * N_PACKETS {
            break;
        }
    }

    let end = clock.time();

    let test_time = end.opt_sub(start);
    let pull_time = end.opt_sub(pushed);
    let push_time = pushed.opt_sub(start);
    println!(
        "test took {} (push {}, pull {})",
        test_time.display(),
        push_time.display(),
        pull_time.display()
    );
}

#[test]
fn recv_release_sink_pad() {
    init();

    let id = next_element_counter();

    let elem = gst::ElementFactory::make("rtprecv")
        .property("rtp-id", id.to_string())
        .build()
        .unwrap();
    elem.set_state(gst::State::Playing).unwrap();
    let sinkpad = elem.request_pad_simple("rtp_sink_0").unwrap();
    let stream_start = gst::event::StreamStart::new("random");
    sinkpad.send_event(stream_start);
    let caps = Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", TEST_PT as i32)
        .field("clock-rate", TEST_CLOCK_RATE as i32)
        .field("encoding-name", "custom-test")
        .build();
    sinkpad.send_event(gst::event::Caps::new(&caps));
    let segment = gst::FormattedSegment::<gst::ClockTime>::new();
    sinkpad.send_event(gst::event::Segment::new(&segment));

    let (sender, recv) = std::sync::mpsc::sync_channel(1);
    elem.connect_pad_added({
        let sender = sender.clone();
        move |_elem, pad| {
            let other_pad = gst::Pad::builder(gst::PadDirection::Sink)
                .chain_function(|_pad, _parent, _buffer| Ok(gst::FlowSuccess::Ok))
                .build();
            other_pad.set_active(true).unwrap();
            pad.link(&other_pad).unwrap();
            sender.send(other_pad).unwrap();
        }
    });
    // push two buffers to get past the rtpsource validation
    sinkpad
        .chain(
            PacketInfo {
                seq_no: 30,
                rtp_ts: 10,
                payload_len: 4,
            }
            .generate_buffer(Some(gst::ClockTime::from_mseconds(50))),
        )
        .unwrap();
    sinkpad
        .chain(
            PacketInfo {
                seq_no: 31,
                rtp_ts: 10,
                payload_len: 4,
            }
            .generate_buffer(Some(gst::ClockTime::from_mseconds(100))),
        )
        .unwrap();
    let _other_pad = recv.recv().unwrap();

    elem.release_request_pad(&sinkpad);
    elem.set_state(gst::State::Null).unwrap();
}
