//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::tests::{run_test_pipeline, ExpectedBuffer, ExpectedPacket, Source};

use std::cmp;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtppcmau test");
    });
}

#[test]
fn test_pcma() {
    init();

    let src = "audiotestsrc num-buffers=100 samplesperbuffer=400 ! audio/x-raw,rate=8000,channels=1 ! alawenc";
    let pay = "rtppcmapay2";
    let depay = "rtppcmadepay2";

    let mut expected_pay = Vec::with_capacity(100);
    for i in 0..100 {
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(i * 50))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
            } else {
                gst::BufferFlags::empty()
            })
            .pt(8)
            .rtp_time(((i * 400) & 0xffff_ffff) as u32)
            .marker_bit(i == 0)
            .build()]);
    }

    let mut expected_depay = Vec::with_capacity(100);
    for i in 0..100 {
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(i * 50))
            .size(400)
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC
            } else {
                gst::BufferFlags::empty()
            })
            .build()]);
    }

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn test_pcma_splitting() {
    init();

    let src = "audiotestsrc num-buffers=100 samplesperbuffer=480 ! audio/x-raw,rate=8000,channels=1 ! alawenc";
    let pay = "rtppcmapay2 min-ptime=25000000 max-ptime=50000000";
    let depay = "rtppcmadepay2";

    // Every input buffer is 480 samples, every packet can contain between 200 and 400 samples
    // so a bit of splitting is necessary and sometimes the remaining queued data ends up filling
    // one additional packet
    let mut expected_pay = Vec::with_capacity(134);
    let mut queued = 0;
    let mut pos = 0;
    for i in 0..100 {
        queued += 480;

        while queued >= 200 || (i == 99 && queued > 0) {
            let size = cmp::min(queued, 400);
            queued -= size;

            expected_pay.push(vec![ExpectedPacket::builder()
                .pts(gst::ClockTime::from_mseconds(pos / 8))
                .size(size as usize + 12)
                .flags(if pos == 0 {
                    gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
                } else {
                    gst::BufferFlags::empty()
                })
                .pt(8)
                .rtp_time((pos & 0xffff_ffff) as u32)
                .marker_bit(pos == 0)
                .build()]);

            pos += size;
        }
    }

    let mut expected_depay = Vec::with_capacity(134);
    for packets in &expected_pay {
        for packet in packets {
            expected_depay.push(vec![ExpectedBuffer::builder()
                .pts(packet.pts)
                .maybe_size(packet.size.map(|size| size - 12))
                .flags(if packet.pts.is_zero() {
                    gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC
                } else {
                    gst::BufferFlags::empty()
                })
                .build()]);
        }
    }

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn test_pcma_discont() {
    init();

    let caps = gst::Caps::builder("audio/x-alaw")
        .field("channels", 1)
        .field("rate", 8000i32)
        .build();

    let mut buffers = Vec::with_capacity(10);
    let mut pos = 0;
    // First 5 buffers are normal, then a 10s jump
    for _ in 0..10 {
        let mut buffer = gst::Buffer::with_size(400).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(gst::ClockTime::from_mseconds(pos / 8));
        }

        buffers.push(buffer);

        pos += 400;
        if pos == 2000 {
            pos += 80000;
        }
    }

    let pay = "rtppcmapay2 discont-wait=25000000";
    let depay = "rtppcmadepay2";

    let mut expected_pay = Vec::with_capacity(10);
    let mut pos = 0;
    for _ in 0..10 {
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(pos / 8))
            .size(412)
            .flags(if pos == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
            } else if pos == 82000 {
                gst::BufferFlags::MARKER
            } else {
                gst::BufferFlags::empty()
            })
            .pt(8)
            .rtp_time((pos & 0xffff_ffff) as u32)
            .marker_bit(pos == 0 || pos == 82000)
            .build()]);

        pos += 400;
        if pos == 2000 {
            pos += 80000;
        }
    }

    let mut expected_depay = Vec::with_capacity(10);
    for packets in &expected_pay {
        for packet in packets {
            expected_depay.push(vec![ExpectedBuffer::builder()
                .pts(packet.pts)
                .maybe_size(packet.size.map(|size| size - 12))
                .flags(if packet.pts.is_zero() {
                    gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC
                } else if packet.flags.contains(gst::BufferFlags::MARKER) {
                    gst::BufferFlags::RESYNC
                } else {
                    gst::BufferFlags::empty()
                })
                .build()]);
        }
    }

    run_test_pipeline(
        Source::Buffers(caps, buffers),
        pay,
        depay,
        expected_pay,
        expected_depay,
    );
}

#[test]
fn test_pcmu() {
    init();

    let src = "audiotestsrc num-buffers=100 samplesperbuffer=400 ! audio/x-raw,rate=8000,channels=1 ! mulawenc";
    let pay = "rtppcmupay2";
    let depay = "rtppcmudepay2";

    let mut expected_pay = Vec::with_capacity(100);
    for i in 0..100 {
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(i * 50))
            .size(412)
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
            } else {
                gst::BufferFlags::empty()
            })
            .pt(0)
            .rtp_time(((i * 400) & 0xffff_ffff) as u32)
            .marker_bit(i == 0)
            .build()]);
    }

    let mut expected_depay = Vec::with_capacity(100);
    for i in 0..100 {
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(i * 50))
            .size(400)
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC
            } else {
                gst::BufferFlags::empty()
            })
            .build()]);
    }

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}
