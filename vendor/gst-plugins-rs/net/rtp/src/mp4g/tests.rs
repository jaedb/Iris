// SPDX-License-Identifier: MPL-2.0

use crate::tests::{run_test_pipeline, ExpectedBuffer, ExpectedPacket, Source};
use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpmp4g test");
    });
}

#[test]
fn aac_hbr_not_fragmented() {
    init();

    let src =
        "audiotestsrc num-buffers=100 ! audio/x-raw,rate=48000,channels=2 ! fdkaacenc ! aacparse";
    let pay = "rtpmp4gpay2 seqnum-offset=1";
    let depay = "capsfilter caps=application/x-rtp,seqnum-base=(uint)1 ! rtpmp4gdepay2";

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

    let mut expected_depay = Vec::with_capacity(102);
    for i in 0..102 {
        let position = i * 1024;

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
fn aac_hbr_fragmented() {
    init();

    let src =
        "audiotestsrc num-buffers=100 ! audio/x-raw,rate=48000,channels=1 ! fdkaacenc ! aacparse";
    let pay = "rtpmp4gpay2 mtu=288 seqnum-offset=1";
    let depay = "capsfilter caps=application/x-rtp,seqnum-base=(uint)1 ! rtpmp4gdepay2";

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

    let mut expected_depay = Vec::with_capacity(102);
    for i in 0..102 {
        let position = i * 1024;

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
fn generic_not_fragmented() {
    const BUFFER_NB: usize = 4;
    const BUFFER_SIZE: usize = 600;
    const MTU: usize = 1400;
    const PACKETS_PER_BUFFER: usize = MTU / BUFFER_SIZE;
    const RTP_CLOCK_RATE: u64 = 90_000;
    const FRAME_RATE: u64 = 30;

    init();

    let codec_data = gst::Buffer::from_slice([0x00, 0x00, 0x01, 0xb0, 0x01]);
    let caps = gst::Caps::builder("video/mpeg")
        .field("mpegversion", 4i32)
        .field("systemstream", false)
        .field("codec_data", codec_data)
        .build();

    let pos_to_pts = |pos: usize| {
        1000.hours()
            + (pos as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, FRAME_RATE)
                .map(gst::ClockTime::from_nseconds)
                .unwrap()
    };

    let pos_to_rtp = |pos: usize| {
        ((pos as u64)
            .mul_div_ceil(RTP_CLOCK_RATE, FRAME_RATE)
            .unwrap()
            & 0xffff_ffff) as u32
    };

    let duration =
        gst::ClockTime::from_nseconds(1.mul_div_ceil(*gst::ClockTime::SECOND, FRAME_RATE).unwrap());

    let mut buffers = Vec::with_capacity(BUFFER_NB);
    for pos in 0..BUFFER_NB {
        let mut buffer = gst::Buffer::with_size(BUFFER_SIZE).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            let pts = pos_to_pts(pos);
            buffer.set_pts(pts);
            buffer.set_dts(match pos {
                0 => pts,
                1 | 2 => pos_to_pts(pos + 1),
                3 => pos_to_pts(pos - 2),
                _ => unreachable!(),
            });
            buffer.set_duration(duration);
            if pos == 0 {
                buffer.set_flags(gst::BufferFlags::DISCONT);
            } else {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }

        buffers.push(buffer);
    }

    let pay = format!("rtpmp4gpay2 mtu={MTU} seqnum-offset=1");
    let depay = "capsfilter caps=application/x-rtp,seqnum-base=(uint)1 ! rtpmp4gdepay2";

    let mut expected_pay = Vec::with_capacity(BUFFER_NB);
    for i in 0..PACKETS_PER_BUFFER {
        expected_pay.push(vec![ExpectedPacket::builder()
            .pts(pos_to_pts(i * PACKETS_PER_BUFFER))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER
            } else {
                gst::BufferFlags::MARKER
            })
            .rtp_time(pos_to_rtp(i * PACKETS_PER_BUFFER))
            .build()]);
    }

    let mut expected_depay = Vec::with_capacity(BUFFER_NB);
    for i in 0..BUFFER_NB {
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(
                pos_to_pts(i)
                    + if i == 3 {
                        11110.nseconds()
                    } else {
                        0.nseconds()
                    },
            )
            .dts(match i {
                0 => pos_to_pts(0),
                1 => pos_to_pts(1 + 1),
                2 => pos_to_pts(2 + 1) + 11110.nseconds(),
                3 => pos_to_pts(3 - 2) + 11111.nseconds(),
                _ => unreachable!(),
            })
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::DELTA_UNIT
            })
            .build()]);
    }

    run_test_pipeline(
        Source::Buffers(caps, buffers),
        &pay,
        depay,
        expected_pay,
        expected_depay,
    );
}

#[test]
fn generic_fragmented() {
    const BUFFER_NB: usize = 4;
    const BUFFER_SIZE: usize = 2000;
    const MTU: usize = 1400;
    // Enough overhead in the MTU to use this approximation:
    const FRAGMENTS_PER_BUFFER: usize = BUFFER_SIZE.div_ceil(MTU);
    const RTP_CLOCK_RATE: u64 = 90_000;
    const LAST_FRAGMENT: usize = FRAGMENTS_PER_BUFFER - 1;
    const FRAME_RATE: u64 = 30;

    init();

    let codec_data = gst::Buffer::from_slice([0x00, 0x00, 0x01, 0xb0, 0x01]);
    let caps = gst::Caps::builder("video/mpeg")
        .field("mpegversion", 4i32)
        .field("systemstream", false)
        .field("codec_data", codec_data)
        .build();

    let pos_to_pts = |pos: usize| {
        1000.hours()
            + (pos as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, FRAME_RATE)
                .map(gst::ClockTime::from_nseconds)
                .unwrap()
    };

    let pos_to_rtp = |pos: usize| {
        ((pos as u64)
            .mul_div_ceil(RTP_CLOCK_RATE, FRAME_RATE)
            .unwrap()
            & 0xffff_ffff) as u32
    };

    let duration =
        gst::ClockTime::from_nseconds(1.mul_div_ceil(*gst::ClockTime::SECOND, FRAME_RATE).unwrap());

    let mut buffers = Vec::with_capacity(BUFFER_NB);
    for pos in 0..BUFFER_NB {
        let mut buffer = gst::Buffer::with_size(BUFFER_SIZE).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            let pts = pos_to_pts(pos);
            buffer.set_pts(pts);
            buffer.set_dts(match pos {
                0 => pts,
                1 | 2 => pos_to_pts(pos + 1),
                3 => pos_to_pts(pos - 2),
                _ => unreachable!(),
            });
            buffer.set_duration(duration);
            if pos == 0 {
                buffer.set_flags(gst::BufferFlags::DISCONT);
            } else {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }

        buffers.push(buffer);
    }

    let pay = format!("rtpmp4gpay2 mtu={MTU} seqnum-offset=1");
    let depay = "capsfilter caps=application/x-rtp,seqnum-base=(uint)1 ! rtpmp4gdepay2";

    let mut expected_pay = Vec::with_capacity(BUFFER_NB);
    for i in 0..BUFFER_NB {
        expected_pay.push({
            let mut packets = Vec::with_capacity(FRAGMENTS_PER_BUFFER);
            for frag in 0..FRAGMENTS_PER_BUFFER {
                packets.push(
                    ExpectedPacket::builder()
                        .pts(pos_to_pts(i))
                        .flags(match (i, frag) {
                            (0, 0) => gst::BufferFlags::DISCONT,
                            (_, LAST_FRAGMENT) => gst::BufferFlags::MARKER,
                            _ => gst::BufferFlags::empty(),
                        })
                        .rtp_time(pos_to_rtp(i))
                        .marker_bit(frag == LAST_FRAGMENT)
                        .build(),
                );
            }

            packets
        });
    }

    let mut expected_depay = Vec::with_capacity(BUFFER_NB);
    for i in 0..BUFFER_NB {
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(pos_to_pts(i))
            .dts(match i {
                0 => pos_to_pts(0),
                1 => pos_to_pts(1 + 1),
                2 => pos_to_pts(2 + 1) + 11110.nseconds(),
                3 => pos_to_pts(3 - 2) + 1.nseconds(),
                _ => unreachable!(),
            })
            .size(BUFFER_SIZE)
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::DELTA_UNIT
            })
            .build()]);
    }

    run_test_pipeline(
        Source::Buffers(caps, buffers),
        &pay,
        depay,
        expected_pay,
        expected_depay,
    );
}

#[test]
fn generic_variable_au_size() {
    const MTU: usize = 1400;
    const AU_NB: usize = 5;
    const SMALL_AU_SIZE: usize = 500;
    const LARGE_AU_SIZE: usize = 2000;
    const FRAGMENTS_PER_LARGE_BUFFER: usize = LARGE_AU_SIZE.div_ceil(MTU);
    const LAST_FRAGMENT: usize = FRAGMENTS_PER_LARGE_BUFFER - 1;
    const RTP_CLOCK_RATE: u64 = 90_000;
    const FRAME_RATE: u64 = 30;

    init();

    let codec_data = gst::Buffer::from_slice([0x00, 0x00, 0x01, 0xb0, 0x01]);
    let caps = gst::Caps::builder("video/mpeg")
        .field("mpegversion", 4i32)
        .field("systemstream", false)
        .field("codec_data", codec_data)
        .build();

    let pos_to_pts = |pos: usize| {
        1000.hours()
            + (pos as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, FRAME_RATE)
                .map(gst::ClockTime::from_nseconds)
                .unwrap()
    };

    let pos_to_rtp = |pos: usize| {
        ((pos as u64)
            .mul_div_ceil(RTP_CLOCK_RATE, FRAME_RATE)
            .unwrap()
            & 0xffff_ffff) as u32
    };

    let duration =
        gst::ClockTime::from_nseconds(1.mul_div_ceil(*gst::ClockTime::SECOND, FRAME_RATE).unwrap());

    let is_large_au = |pos| pos % 4 == 0;
    let au_size = |pos| {
        if is_large_au(pos) {
            LARGE_AU_SIZE
        } else {
            SMALL_AU_SIZE
        }
    };

    let mut buffers = Vec::with_capacity(AU_NB);
    for pos in 0..AU_NB {
        let mut buffer = gst::Buffer::with_size(au_size(pos)).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            let pts = pos_to_pts(pos);
            buffer.set_pts(pts);
            buffer.set_dts(match pos % 4 {
                0 => pts,
                1 | 2 => pos_to_pts(pos + 1),
                3 => pos_to_pts(pos - 2),
                _ => unreachable!(),
            });
            buffer.set_duration(duration);
            if pos == 0 {
                buffer.set_flags(gst::BufferFlags::DISCONT);
            } else {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
        }

        buffers.push(buffer);
    }

    let pay = format!("rtpmp4gpay2 mtu={MTU} seqnum-offset=1");
    let depay = "capsfilter caps=application/x-rtp,seqnum-base=(uint)1 ! rtpmp4gdepay2";

    let mut expected_pay = Vec::with_capacity(AU_NB);
    let mut pending_size = 0;
    let mut pending_packet = None;
    for i in 0..AU_NB {
        let size = au_size(i);
        if size > MTU {
            // Incoming AU to fragment
            let mut packet_list = Vec::with_capacity(3);

            if let Some(pending) = pending_packet.take() {
                // and there are pending AUs => push them first
                packet_list.push(pending);
                pending_size = 0;
            }

            // Then push the fragments for current AU
            for f in 0..FRAGMENTS_PER_LARGE_BUFFER {
                packet_list.push(
                    ExpectedPacket::builder()
                        .pts(pos_to_pts(i))
                        .flags(match (i, f) {
                            (0, 0) => gst::BufferFlags::DISCONT,
                            (_, 0) => gst::BufferFlags::empty(),
                            (_, LAST_FRAGMENT) => gst::BufferFlags::MARKER,
                            _ => unreachable!(),
                        })
                        .rtp_time(pos_to_rtp(i))
                        .marker_bit(f == LAST_FRAGMENT)
                        .build(),
                )
            }

            expected_pay.push(packet_list);
        } else {
            let must_push =
                if i + 1 < AU_NB && pending_size + size + au_size(i + 1) > MTU || i + 1 == AU_NB {
                    // Next will overflow => push now
                    // or last AU and not a fragmented one, will be pushed with time deadline
                    true
                } else {
                    false
                };

            if must_push {
                if let Some(pending) = pending_packet.take() {
                    expected_pay.push(vec![pending]);
                    pending_size = 0;
                } else {
                    // Last AU
                    expected_pay.push(vec![ExpectedPacket::builder()
                        .pts(pos_to_pts(i))
                        .flags(gst::BufferFlags::MARKER)
                        .rtp_time(pos_to_rtp(i))
                        .build()]);
                }
            } else if pending_packet.is_none() {
                // Wait for more payload
                pending_packet = Some(
                    ExpectedPacket::builder()
                        .pts(pos_to_pts(i))
                        .flags(gst::BufferFlags::MARKER)
                        .rtp_time(pos_to_rtp(i))
                        .build(),
                );
                pending_size = size;
            } else {
                // There's already a pending packet
                pending_size += size;
            }
        }
    }

    let mut expected_depay = Vec::with_capacity(AU_NB);
    for i in 0..AU_NB {
        expected_depay.push(vec![ExpectedBuffer::builder()
            .pts(pos_to_pts(i))
            .dts(match i % 4 {
                0 => pos_to_pts(0),
                1 => pos_to_pts(1 + 1),
                2 => pos_to_pts(2 + 1) + 11110.nseconds(),
                3 => pos_to_pts(3 - 2) + 11111.nseconds(),
                _ => unreachable!(),
            })
            .size(au_size(i))
            .flags(if i == 0 {
                gst::BufferFlags::DISCONT
            } else {
                gst::BufferFlags::DELTA_UNIT
            })
            .build()]);
    }

    run_test_pipeline(
        Source::Buffers(caps, buffers),
        &pay,
        depay,
        expected_pay,
        expected_depay,
    );
}
