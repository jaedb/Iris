// SPDX-License-Identifier: MPL-2.0

use crate::tests::{run_test_pipeline, ExpectedBuffer, ExpectedPacket, Source};
use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpmp4a test");
    });
}

#[test]
fn mp4a_one_frame_per_packet() {
    init();

    let src = "audiotestsrc num-buffers=100 ! audio/x-raw,rate=48000,channels=2 ! fdkaacenc";
    let pay = "rtpmp4apay2";
    let depay = "rtpmp4adepay2";

    let mut expected_pay = Vec::with_capacity(102);
    for i in 0..102 {
        let position = i * 1024;

        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_nseconds(
                position
                    .mul_div_floor(*gst::ClockTime::SECOND, 48_000)
                    .unwrap(),
            ))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
            } else {
                gst::BufferFlags::MARKER
            })
            .rtp_time((position & 0xffff_ffff) as u32)
            .build()]);
    }

    let mut expected_depay = Vec::with_capacity(101);
    for i in 0..101 {
        let position = (i + 1) * 1024;

        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_nseconds(
                position
                    .mul_div_floor(*gst::ClockTime::SECOND, 48_000)
                    .unwrap(),
            ))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            })
            .build()]);
    }

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}

#[test]
fn mp4a_fragmented() {
    init();

    let src = "audiotestsrc num-buffers=100 ! audio/x-raw,rate=48000,channels=1 ! fdkaacenc";
    let pay = "rtpmp4apay2 mtu=288";
    let depay = "rtpmp4adepay2";

    let mut expected_pay = Vec::with_capacity(102);
    for i in 0..102 {
        let position = i * 1024;

        let pts = gst::ClockTime::from_nseconds(
            position
                .mul_div_floor(*gst::ClockTime::SECOND, 48_000)
                .unwrap(),
        );
        let rtp_time = (position & 0xffff_ffff) as u32;

        expected_pay.push(vec![
            ExpectedPacket::builder()
                .pts(pts)
                .flags(if i == 0 {
                    gst::BufferFlags::DISCONT
                } else {
                    gst::BufferFlags::empty()
                })
                .rtp_time(rtp_time)
                .marker_bit(false)
                .build(),
            ExpectedPacket::builder()
                .pts(pts)
                .flags(gst::BufferFlags::MARKER)
                .rtp_time(rtp_time)
                .build(),
        ]);
    }

    let mut expected_depay = Vec::with_capacity(101);
    for i in 0..101 {
        let position = (i + 1) * 1024;

        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_nseconds(
                position
                    .mul_div_floor(*gst::ClockTime::SECOND, 48_000)
                    .unwrap(),
            ))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::empty()
            })
            .build()]);
    }

    run_test_pipeline(Source::Bin(src), pay, depay, expected_pay, expected_depay);
}
