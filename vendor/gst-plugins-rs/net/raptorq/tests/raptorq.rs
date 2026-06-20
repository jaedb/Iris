// Copyright (C) 2022 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use gst_rtp::rtp_buffer::*;
use gst_rtp::RTPBuffer;

use rand::Rng;

#[must_use]
struct RaptorqTest {
    protected_packets: usize,
    repair_packets: usize,
    repair_window: usize,
    symbol_size: usize,
    mtu: usize,
    initial_seq: u16,
    lost_buffers: Vec<usize>,
    swapped_buffers: Vec<usize>,
    input_buffers: usize,
    expect_output_buffers: usize,
}

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstraptorq::plugin_register_static().expect("Failed to register raptorqenc plugin");
    });
}

impl RaptorqTest {
    fn new() -> Self {
        init();

        let enc = gst::ElementFactory::make("raptorqenc").build().unwrap();

        let protected_packets = enc.property::<u32>("protected-packets") as usize;
        let repair_packets = enc.property::<u32>("repair-packets") as usize;
        let repair_window = enc.property::<u32>("repair-window") as usize;
        let symbol_size = enc.property::<u32>("symbol-size") as usize;
        let mtu = enc.property::<u32>("mtu") as usize;

        Self {
            protected_packets,
            repair_packets,
            repair_window,
            symbol_size,
            mtu,
            initial_seq: 42,
            lost_buffers: vec![0],
            swapped_buffers: vec![],
            input_buffers: protected_packets,
            expect_output_buffers: protected_packets,
        }
    }

    fn protected_packets(mut self, protected_packets: usize) -> Self {
        self.protected_packets = protected_packets;
        self
    }

    fn repair_packets(mut self, repair_packets: usize) -> Self {
        self.repair_packets = repair_packets;
        self
    }

    fn repair_window(mut self, repair_window: usize) -> Self {
        self.repair_window = repair_window;
        self
    }

    fn symbol_size(mut self, symbol_size: usize) -> Self {
        self.symbol_size = symbol_size;
        self
    }

    fn initial_seq(mut self, initial_seq: u16) -> Self {
        self.initial_seq = initial_seq;
        self
    }

    fn mtu(mut self, mtu: usize) -> Self {
        self.mtu = mtu;
        self
    }

    fn lost_buffers(mut self, lost_buffers: Vec<usize>) -> Self {
        self.lost_buffers = lost_buffers;
        self
    }

    fn swapped_buffers(mut self, swapped_buffers: Vec<usize>) -> Self {
        self.swapped_buffers = swapped_buffers;
        self
    }

    fn input_buffers(mut self, input_buffers: usize) -> Self {
        self.input_buffers = input_buffers;
        self
    }

    fn expect_output_buffers(mut self, expect_output_buffers: usize) -> Self {
        self.expect_output_buffers = expect_output_buffers;
        self
    }

    fn run(self) {
        assert!(self.input_buffers >= self.protected_packets);

        // 1. Decoder Setup:
        let enc = gst::ElementFactory::make("raptorqenc")
            .property("protected-packets", self.protected_packets as u32)
            .property("repair-packets", self.repair_packets as u32)
            .property("repair-window", self.repair_window as u32)
            .property("symbol-size", self.symbol_size as u32)
            .property("mtu", self.mtu as u32)
            .build()
            .unwrap();

        let mut h_enc = gst_check::Harness::with_element(&enc, Some("sink"), Some("src"));
        let mut h_enc_fec = gst_check::Harness::with_element(&enc, None, Some("fec_0"));

        h_enc.set_src_caps_str("application/x-rtp,clock-rate=8000");

        // 2. Decoder Setup:
        let dec = gst::ElementFactory::make("raptorqdec").build().unwrap();

        let mut h_dec = gst_check::Harness::with_element(&dec, Some("sink"), Some("src"));
        let mut h_dec_fec = gst_check::Harness::with_element(&dec, Some("fec_0"), None);

        let caps = gst::Caps::builder("application/x-rtp")
            .field("raptor-scheme-id", "6")
            .field("repair-window", "1000000")
            .field("t", self.symbol_size.to_string())
            .build();

        h_dec.set_src_caps_str("application/x-rtp");
        h_dec_fec.set_src_caps(caps);

        let mut rng = rand::rng();

        let input_buffers = (0..self.input_buffers)
            .map(|i| {
                // payload size without RTP Header and ADUI Header
                let size = rng.random_range(1..self.mtu - 12 - 3);
                let data = (0..size).map(|_| rng.random()).collect::<Vec<u8>>();

                let mut buf = gst::Buffer::new_rtp_with_sizes(size as u32, 0, 0).unwrap();
                {
                    let buf_mut = buf.get_mut().unwrap();
                    buf_mut.set_pts(gst::ClockTime::ZERO);
                    buf_mut.set_dts(gst::ClockTime::ZERO);

                    let mut rtpbuf = RTPBuffer::from_buffer_writable(buf_mut).unwrap();
                    let payload = rtpbuf.payload_mut().unwrap();

                    payload.copy_from_slice(data.as_slice());
                    rtpbuf.set_seq(self.initial_seq.wrapping_add(i as u16));
                    rtpbuf.set_timestamp(0);
                }

                buf
            })
            .collect::<Vec<_>>();

        // 3. Encoder Operations:

        // Do not consume buffers here so we can compare it with the output
        for buf in &input_buffers {
            let result = h_enc.push(buf.clone());
            assert!(result.is_ok());
        }

        assert_eq!(h_enc.buffers_in_queue(), self.input_buffers as u32);

        let mut media_packets = (0..self.input_buffers)
            .map(|_| {
                let result = h_enc.pull();
                assert!(result.is_ok());
                result.unwrap()
            })
            .collect::<Vec<_>>();

        // Simulate out of order packets
        for x in self.swapped_buffers.chunks_exact(2) {
            media_packets.swap(x[0], x[1])
        }

        // Check if repair packets pushed from encoder are delayed properly
        let delay_step = ((self.repair_window / self.repair_packets) as u64).mseconds();
        let mut delay = delay_step;

        let repair_packets = (0..self.repair_packets)
            .map(|_| {
                // Set time just before the timer to push the buffer fires up,
                // we shouldn't see the buffer just yet.
                h_enc_fec.set_time(delay - gst::ClockTime::NSECOND).unwrap();
                assert_eq!(h_enc_fec.buffers_in_queue(), 0);

                // Advance time to the delay and crank clock id, we should
                // get a buffer with adjusted timestamps now. All input buffers
                // have zero timestamp, so the pts/dts/rtp-timestamp should be
                // equal to delay.
                h_enc_fec.set_time(delay).unwrap();
                h_enc_fec.crank_single_clock_wait().unwrap();

                let result = h_enc_fec.pull();
                assert!(result.is_ok());

                let buf = result.unwrap();
                assert_eq!(buf.pts().unwrap(), delay);
                assert_eq!(buf.dts().unwrap(), delay);

                let ts = RTPBuffer::from_buffer_readable(&buf).unwrap().timestamp();
                let expected_ts =
                    *delay.mul_div_round(*gst::ClockTime::SECOND, 8000).unwrap() as u32;

                assert_eq!(ts, expected_ts);

                delay += delay_step;
                buf
            })
            .collect::<Vec<_>>();

        // 4. Decoder Operations:

        // remove media packets to simulate packet loss
        let media_packets = media_packets
            .iter()
            .cloned()
            .enumerate()
            .filter(|(i, _)| !self.lost_buffers.contains(i))
            .map(|(_, x)| x)
            .collect::<Vec<_>>();

        // Push media packets to decoder
        for buf in media_packets {
            assert!(h_dec.push(buf).is_ok());
        }

        // Push repair packets to decoder
        for buf in repair_packets {
            assert!(h_dec_fec.push(buf).is_ok());
        }

        // At this point decoder has all the information it needs to
        // recover packets, we just need an input buffer to run sink
        // chain operations.
        let result = h_dec.push(input_buffers.iter().last().unwrap().clone());
        assert!(result.is_ok());

        let mut output_buffers = (0..self.expect_output_buffers)
            .map(|_| {
                let result = h_dec.pull();
                assert!(result.is_ok());
                result.unwrap()
            })
            .collect::<Vec<_>>();

        // Output buffers are out of sequence, we should sort it by
        // seqnum so we can compare them with input buffers.
        output_buffers.sort_unstable_by(|a, b| {
            let aa = RTPBuffer::from_buffer_readable(a).unwrap();
            let bb = RTPBuffer::from_buffer_readable(b).unwrap();

            match gst_rtp::compare_seqnum(bb.seq(), aa.seq()) {
                x if x > 0 => std::cmp::Ordering::Greater,
                x if x < 0 => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            }
        });

        assert_eq!(output_buffers.len(), self.expect_output_buffers);

        if self.input_buffers == self.expect_output_buffers {
            for (inbuf, outbuf) in Iterator::zip(input_buffers.iter(), output_buffers.iter()) {
                let rtp1 = RTPBuffer::from_buffer_readable(inbuf).unwrap();
                let rtp2 = RTPBuffer::from_buffer_readable(outbuf).unwrap();

                assert_eq!(rtp1.seq(), rtp2.seq());
                assert_eq!(rtp1.payload().unwrap(), rtp2.payload().unwrap());
            }
        }
    }
}

#[test]
fn test_raptorq_all_default() {
    RaptorqTest::new().run();
}

#[test]
fn test_raptorq_decoder_media_packets_out_of_sequence() {
    RaptorqTest::new()
        .swapped_buffers(vec![5, 10, 12, 15])
        .run();
}

#[test]
fn test_raptorq_10_percent_overhead() {
    RaptorqTest::new()
        .protected_packets(100)
        .repair_packets(10)
        .lost_buffers(vec![4, 42, 43, 44, 45])
        .input_buffers(100)
        .expect_output_buffers(100)
        .run();
}

#[test]
fn test_raptorq_5_percent_overhead() {
    RaptorqTest::new()
        .protected_packets(100)
        .repair_packets(5)
        .input_buffers(100)
        .lost_buffers(vec![8, 11])
        .expect_output_buffers(100)
        .run();
}

#[test]
fn test_raptorq_symbol_size_128() {
    RaptorqTest::new()
        .protected_packets(20)
        .repair_packets(4)
        .symbol_size(128)
        .mtu(400)
        .input_buffers(20)
        .lost_buffers(vec![9])
        .expect_output_buffers(20)
        .run();
}

#[test]
fn test_raptorq_symbol_size_192() {
    RaptorqTest::new()
        .protected_packets(20)
        .repair_packets(4)
        .symbol_size(192)
        .mtu(999)
        .input_buffers(20)
        .lost_buffers(vec![16, 19])
        .expect_output_buffers(20)
        .run();
}

#[test]
fn test_raptorq_symbol_size_1024() {
    RaptorqTest::new()
        .protected_packets(20)
        .repair_packets(8)
        .symbol_size(192)
        .mtu(100)
        .input_buffers(20)
        .lost_buffers(vec![0, 1, 2, 3, 4, 5])
        .expect_output_buffers(20)
        .run();
}

#[test]
fn test_raptorq_mtu_lt_symbol_size() {
    RaptorqTest::new()
        .protected_packets(20)
        .repair_packets(8)
        .symbol_size(1400)
        .mtu(100)
        .input_buffers(20)
        .lost_buffers(vec![14, 15, 16, 17, 18, 19])
        .expect_output_buffers(20)
        .run();
}

#[test]
fn test_raptorq_heavy_loss() {
    RaptorqTest::new()
        .protected_packets(40)
        .repair_packets(8)
        .input_buffers(40)
        .lost_buffers(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
        .expect_output_buffers(30)
        .run();
}

#[test]
fn test_raptorq_repair_window_100ms() {
    RaptorqTest::new()
        .protected_packets(10)
        .repair_packets(10)
        .repair_window(100)
        .input_buffers(10)
        .lost_buffers(vec![2, 6])
        .expect_output_buffers(10)
        .run();
}

#[test]
fn test_raptorq_repair_window_500ms() {
    RaptorqTest::new()
        .protected_packets(8)
        .repair_packets(2)
        .repair_window(500)
        .input_buffers(8)
        .lost_buffers(vec![])
        .expect_output_buffers(8)
        .run();
}

#[test]
fn test_raptorq_wrapping_sequence_number_1() {
    RaptorqTest::new().initial_seq(u16::MAX - 5).run();
}

#[test]
fn test_raptorq_wrapping_sequence_number_2() {
    RaptorqTest::new()
        .initial_seq(u16::MAX - 5)
        .swapped_buffers(vec![4, 5])
        .run();
}

#[test]
fn test_raptorq_wrapping_sequence_number_3() {
    RaptorqTest::new()
        .initial_seq(u16::MAX - 3)
        .lost_buffers(vec![0, 1, 2, 8])
        .run();
}

#[test]
fn test_raptorq_encoder_flush_cancels_pending_timers() {
    init();

    let enc = gst::ElementFactory::make("raptorqenc")
        // Set delay to 5s, this way each buffer should be delayed by 1s
        .property("repair-window", 5000u32)
        .property("protected-packets", 5u32)
        .property("repair-packets", 5u32)
        .build()
        .unwrap();

    let mut h_enc = gst_check::Harness::with_element(&enc, Some("sink"), Some("src"));
    let mut h_enc_fec = gst_check::Harness::with_element(&enc, None, Some("fec_0"));

    h_enc.set_src_caps_str("application/x-rtp,clock-rate=8000");

    for i in 0u64..5 {
        let mut buf = gst::Buffer::new_rtp_with_sizes(42, 0, 0).unwrap();

        let buf_mut = buf.get_mut().unwrap();
        buf_mut.set_pts(gst::ClockTime::SECOND * i);

        let mut rtpbuf = RTPBuffer::from_buffer_writable(buf_mut).unwrap();
        rtpbuf.set_seq(i as u16);

        drop(rtpbuf);

        let result = h_enc.push(buf);
        assert!(result.is_ok());
    }

    // We want to check if flush cancels pending timers, last buffer of source
    // block is at 5s, at 6s we should have 1 buffer qeued already, then we flush
    // and move time to 10s. Flush should cancel pending timers and we should
    // have no buffers at the output
    h_enc_fec.set_time(gst::ClockTime::SECOND * 6).unwrap();
    h_enc_fec.crank_single_clock_wait().unwrap();

    let result = h_enc_fec.pull();
    assert!(result.is_ok());

    h_enc.push_event(gst::event::FlushStart::new());
    h_enc.push_event(gst::event::FlushStop::new(true));

    h_enc_fec.set_time(gst::ClockTime::SECOND * 10).unwrap();

    loop {
        let event = h_enc.pull_event();

        if let Ok(event) = event {
            match event.view() {
                gst::EventView::FlushStart(_) => {
                    continue;
                }
                gst::EventView::FlushStop(_) => {
                    break;
                }
                _ => (),
            }
        }
    }

    assert_eq!(h_enc_fec.buffers_in_queue(), 0);
    assert_eq!(h_enc_fec.testclock().unwrap().peek_id_count(), 0);
}

#[test]
fn test_raptorq_repair_window_tolerance() {
    init();

    let enc = gst::ElementFactory::make("raptorqenc")
        // Set delay to 5s, this way each buffer should be delayed by 1s
        .property("repair-window", 1000u32)
        .property("protected-packets", 5u32)
        .property("repair-packets", 5u32)
        .build()
        .unwrap();

    let mut h_enc = gst_check::Harness::with_element(&enc, Some("sink"), Some("src"));
    let mut h_enc_fec = gst_check::Harness::with_element(&enc, None, Some("fec_0"));

    h_enc.set_src_caps_str("application/x-rtp,clock-rate=8000");

    for i in 0u64..5 {
        let mut buf = gst::Buffer::new_rtp_with_sizes(42, 0, 0).unwrap();

        let buf_mut = buf.get_mut().unwrap();
        buf_mut.set_pts(gst::ClockTime::SECOND * i);

        let mut rtpbuf = RTPBuffer::from_buffer_writable(buf_mut).unwrap();
        rtpbuf.set_seq(i as u16);

        drop(rtpbuf);

        let result = h_enc.push(buf);
        assert!(result.is_ok());
    }

    let dec = gst::ElementFactory::make("raptorqdec")
        .property("repair-window-tolerance", 1000u32)
        .build()
        .unwrap();

    let mut h_dec = gst_check::Harness::with_element(&dec, Some("sink"), Some("src"));
    let mut h_dec_fec = gst_check::Harness::with_element(&dec, Some("fec_0"), None);

    let caps = loop {
        let event = h_enc_fec.pull_event();

        if let Ok(event) = event {
            #[allow(clippy::single_match)]
            match event.view() {
                gst::EventView::Caps(c) => {
                    break c.caps_owned();
                }
                _ => (),
            }
        }
    };

    h_dec.set_src_caps_str("application/x-rtp");
    h_dec_fec.set_src_caps(caps);

    h_enc_fec.set_time(1.seconds()).unwrap();

    let result = h_enc.pull();
    assert!(result.is_ok());

    let buf = result.unwrap();
    let result = h_dec.push(buf);
    assert!(result.is_ok());

    // Push some of repair packets to decoder, just not enough to recover
    // media packets
    for _ in 0..2 {
        h_enc_fec.crank_single_clock_wait().unwrap();

        let result = h_enc_fec.pull();
        assert!(result.is_ok());

        let buf = result.unwrap();
        let result = h_dec_fec.push(buf);
        assert!(result.is_ok());
    }

    let stats = h_dec.element().unwrap().property::<gst::Structure>("stats");
    assert_eq!(
        stats
            .get::<u64>("buffered-media-packets")
            .expect("type error"),
        1
    );
    assert_eq!(
        stats
            .get::<u64>("buffered-repair-packets")
            .expect("type error"),
        2
    );

    // Media buffer is way beyond repair window which is 2 seconds,
    // (repair_window (1s) + repair_window_tolerance (1s)),
    // the decoder should drop buffered  packets as they were kept for too long.
    let mut buf = gst::Buffer::new_rtp_with_sizes(42, 0, 0).unwrap();
    let buf_mut = buf.get_mut().unwrap();
    buf_mut.set_pts(gst::ClockTime::SECOND * 10);

    let result = h_dec.push(buf);
    assert!(result.is_ok());

    let stats = h_dec.element().unwrap().property::<gst::Structure>("stats");
    assert_eq!(
        stats
            .get::<u64>("buffered-media-packets")
            .expect("type error"),
        0
    );
    assert_eq!(
        stats
            .get::<u64>("buffered-repair-packets")
            .expect("type error"),
        0
    );
}
