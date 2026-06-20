// GStreamer RTP Opus Payloader / Depayloader - unit tests
//
// Copyright (C) 2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::tests::{
    run_test_pipeline, run_test_pipeline_full, ExpectedBuffer, ExpectedPacket, Liveness, Source,
};
use gst::prelude::*;
use gst_check::Harness;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpopus test");
    });
}

fn make_buffer(
    data: &'static [u8],
    pts: gst::ClockTime,
    duration: gst::ClockTime,
    flags: gst::BufferFlags,
) -> gst::Buffer {
    let mut buf = gst::Buffer::from_slice(data);

    let buf_ref = buf.get_mut().unwrap();
    buf_ref.set_pts(pts);
    buf_ref.set_duration(duration);
    buf_ref.set_flags(flags);

    buf
}

// test_opus_pay_dtx
//
// Make sure payloader drops any DTX packets by the encoder (if so requested via the property)
//
#[test]
fn test_opus_pay_dtx() {
    // gst-launch-1.0 audiotestsrc wave=silence
    //  ! opusenc dtx=true bitrate-type=vbr
    //  ! fakesink silent=false dump=true
    const OPUS_BUFFER_SILENCE: &[u8] = &[0xf8, 0xff, 0xfe];
    const OPUS_BUFFER_SILENCE_DTX: &[u8] = &[0xf8];

    init();

    for dtx_prop in [false, true] {
        eprintln!("Testing rtpopuspay2 dtx={dtx_prop} ..");

        let input_caps = gst::Caps::builder("audio/x-opus")
            .field("rate", 48000i32)
            .field("channels", 1i32)
            .field("channel-mapping-family", 0i32)
            .build();

        let input_buffers = vec![
            make_buffer(
                OPUS_BUFFER_SILENCE,
                gst::ClockTime::ZERO,
                gst::ClockTime::from_useconds(13500),
                gst::BufferFlags::DISCONT,
            ),
            make_buffer(
                OPUS_BUFFER_SILENCE,
                gst::ClockTime::from_useconds(13500),
                gst::ClockTime::from_mseconds(20),
                gst::BufferFlags::empty(),
            ),
            make_buffer(
                OPUS_BUFFER_SILENCE_DTX,
                gst::ClockTime::from_useconds(33500),
                gst::ClockTime::from_mseconds(20),
                gst::BufferFlags::empty(),
            ),
        ];

        // TODO: check durations?
        let mut expected_pay = vec![
            vec![ExpectedPacket::builder()
                .pts(gst::ClockTime::ZERO)
                .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
                .pt(96)
                .rtp_time(0)
                .marker_bit(true)
                .build()],
            vec![ExpectedPacket::builder()
                .pts(gst::ClockTime::from_useconds(13500))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(648)
                .marker_bit(false)
                .build()],
            vec![ExpectedPacket::builder()
                .pts(gst::ClockTime::from_useconds(33500))
                .flags(gst::BufferFlags::empty())
                .pt(96)
                .rtp_time(648 + 960)
                .marker_bit(false)
                .build()],
        ];

        // TODO: check durations?
        let mut expected_depay = vec![
            vec![ExpectedBuffer::builder()
                .pts(gst::ClockTime::ZERO)
                .size(3)
                .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
                .build()],
            vec![ExpectedBuffer::builder()
                .pts(gst::ClockTime::from_useconds(13500))
                .size(3)
                .flags(gst::BufferFlags::empty())
                .build()],
            vec![ExpectedBuffer::builder()
                .pts(gst::ClockTime::from_useconds(33500))
                .size(1)
                .flags(gst::BufferFlags::empty())
                .build()],
        ];

        if dtx_prop {
            // Payloader should drop DTX buffer if dtx=true and not output it
            expected_pay.pop();
            expected_depay.pop();
        }

        run_test_pipeline(
            Source::Buffers(input_caps, input_buffers),
            &format!("rtpopuspay2 dtx={dtx_prop}"),
            "rtpopusdepay2",
            expected_pay,
            expected_depay,
        );
    }
}

// test_opus_pay_depay
//
// Check basic payloading/depayloading
//
#[test]
fn test_opus_pay_depay() {
    // gst-launch-1.0 audiotestsrc ! opusenc ! multifilesink
    const OPUS_BUFFERS: &[&[u8]] = &[
        include_bytes!("audiotestsrc-1ch-48kHz-000.opus").as_slice(),
        include_bytes!("audiotestsrc-1ch-48kHz-001.opus").as_slice(),
        include_bytes!("audiotestsrc-1ch-48kHz-002.opus").as_slice(),
    ];

    init();

    let input_caps = gst::Caps::builder("audio/x-opus")
        .field("rate", 48000i32)
        .field("channels", 1i32)
        .field("channel-mapping-family", 0i32)
        .field("stream-count", 1i32)
        .field("coupled-count", 0i32)
        .build();

    let input_buffers = vec![
        make_buffer(
            OPUS_BUFFERS[0],
            gst::ClockTime::ZERO,
            gst::ClockTime::from_mseconds(20), // Note: no ClippingMeta for lead-in here unlike opusenc
            gst::BufferFlags::DISCONT,
        ),
        make_buffer(
            OPUS_BUFFERS[1],
            gst::ClockTime::from_mseconds(20),
            gst::ClockTime::from_mseconds(20),
            gst::BufferFlags::empty(),
        ),
        make_buffer(
            OPUS_BUFFERS[2],
            gst::ClockTime::from_mseconds(40),
            gst::ClockTime::from_mseconds(20),
            gst::BufferFlags::empty(),
        ),
    ];

    // TODO: check durations?
    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960 + 960)
            .marker_bit(false)
            .build()],
    ];

    // TODO: check durations?
    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(253)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(168)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(166)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    run_test_pipeline(
        Source::Buffers(input_caps, input_buffers),
        "rtpopuspay2",
        "rtpopusdepay2",
        expected_pay,
        expected_depay,
    );
}

// test_opus_depay_pay
//
// Check basic depayloader ! payloader compatibility
//
#[test]
fn test_opus_depay_pay() {
    // gst-launch-1.0 audiotestsrc
    //  ! opusenc bitrate-type=vbr frame-size=2
    //  ! rtpopuspay ! fakesink dump=true
    const OPUS_RTP_BUFFER: &[u8] = &[
        0x80, 0xe0, 0x6c, 0xd6, 0x5f, 0x7a, 0xdd, 0xae, 0xa6, 0x79, 0xe0, 0xc9, 0xe0, 0xff, 0xfe,
    ];

    init();

    let mut h = Harness::new_parse("rtpopusdepay2 ! rtpopuspay2");

    let input_caps = gst::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("encoding-name", "OPUS")
        .field("clock-rate", 48000i32)
        .field("payload", 96i32)
        .build();

    h.set_src_caps(input_caps);

    let input_buffer = make_buffer(
        OPUS_RTP_BUFFER,
        gst::ClockTime::ZERO,
        gst::ClockTime::from_mseconds(20),
        gst::BufferFlags::DISCONT,
    );

    h.push(input_buffer)
        .expect("Got error flow when pushing buffer");

    let _output_buffer = h.pull().expect("Didn't get output buffer");
}

// test_opus_payloader_get_caps
//
// Check that a caps query on payloader sink pad reflects downstream RTP caps requirements.
//
#[test]
fn test_opus_payloader_get_caps() {
    init();

    fn get_allowed_opus_caps_for_rtp_caps_string(recv_caps_str: &str) -> gst::Caps {
        let src = gst::ElementFactory::make("appsrc").build().unwrap();
        let pay = gst::ElementFactory::make("rtpopuspay2").build().unwrap();
        let sink = gst::ElementFactory::make("appsink")
            .property_from_str("caps", recv_caps_str)
            .build()
            .unwrap();

        gst::Element::link_many([&src, &pay, &sink]).unwrap();

        let pad = src.static_pad("src").unwrap();

        pad.allowed_caps().unwrap()
    }

    fn get_allowed_opus_caps_for_rtp_caps(flavour: &str, recv_props: &str) -> gst::Caps {
        get_allowed_opus_caps_for_rtp_caps_string(&format!(
            "application/x-rtp, encoding-name={flavour}, {recv_props}"
        ))
    }

    let stereo_caps = gst::Caps::builder("audio/x-opus")
        .field("channels", 2i32)
        .build();

    let mono_caps = gst::Caps::builder("audio/x-opus")
        .field("channels", 1i32)
        .build();

    // "stereo" is just a hint from receiver that sender can use to avoid unnecessary processing,
    // but it doesn't signal a hard requirement. So both mono and stereo should always be allowed
    // as input, but the channel preference should be relayed in the first caps structure.
    {
        let allowed_opus_caps = get_allowed_opus_caps_for_rtp_caps("OPUS", "stereo=(string)0");

        eprintln!("OPUS stereo=0 => {allowed_opus_caps:?}");

        assert_eq!(allowed_opus_caps.size(), 2);

        // First caps structure should have channels=1 for receiver preference stereo=0
        let s = allowed_opus_caps.structure(0).unwrap();
        assert_eq!(s.get::<i32>("channels"), Ok(1));

        // ... but stereo should still be allowed
        assert!(allowed_opus_caps.can_intersect(&stereo_caps));

        // Make sure it's channel-mapping-family=0
        for i in 0..2 {
            let s = allowed_opus_caps.structure(i).unwrap();
            assert!(s.has_name("audio/x-opus"));
            assert_eq!(s.get::<i32>("channel-mapping-family"), Ok(0));
        }
    }

    {
        let allowed_opus_caps = get_allowed_opus_caps_for_rtp_caps("OPUS", "stereo=(string)1");

        eprintln!("OPUS stereo=1 => {allowed_opus_caps:?}");

        assert_eq!(allowed_opus_caps.size(), 2);

        // First caps structure should have channels=2 for receiver preference stereo=1
        let s = allowed_opus_caps.structure(0).unwrap();
        assert_eq!(s.get::<i32>("channels"), Ok(2));

        // ... but mono should still be allowed
        assert!(allowed_opus_caps.can_intersect(&mono_caps));

        // Make sure it's channel-mapping-family=0
        for i in 0..2 {
            let s = allowed_opus_caps.structure(i).unwrap();
            assert!(s.has_name("audio/x-opus"));
            assert_eq!(s.get::<i32>("channel-mapping-family"), Ok(0));
        }
    }

    // Make sure that with MULTIOPUS flavour the stereo hint is ignored entirely
    for stereo_str in &[
        &"stereo=(string)0",
        &"stereo=(string)1",
        &"description=none", // no stereo attribute present
    ] {
        let allowed_opus_caps = get_allowed_opus_caps_for_rtp_caps("MULTIOPUS", stereo_str);

        eprintln!("MULTIOPUS {stereo_str} => {allowed_opus_caps:?}");

        assert_eq!(allowed_opus_caps.size(), 1);

        // Neither mono nor stereo should be allowed for MULTIOPUS
        let mono_stereo_caps = gst::Caps::builder("audio/x-opus")
            .field("channels", gst::IntRange::new(1, 2))
            .build();
        assert!(!allowed_opus_caps.can_intersect(&mono_stereo_caps));

        // Make sure it's channel-mapping-family=1
        let s = allowed_opus_caps.structure(0).unwrap();
        assert!(s.has_name("audio/x-opus"));
        assert_eq!(s.get::<i32>("channel-mapping-family"), Ok(1));
        assert_eq!(
            s.get::<gst::IntRange::<i32>>("channels"),
            Ok(gst::IntRange::new(3, 255))
        );
    }

    // If receiver supports both OPUS and MULTIOPUS ..
    {
        let allowed_opus_caps = get_allowed_opus_caps_for_rtp_caps_string(
            "\
            application/x-rtp, encoding-name=OPUS, stereo=(string)0; \
            application/x-rtp, encoding-name=MULTIOPUS",
        );

        eprintln!("OPUS,stereo=0; MULTIOPUS => {allowed_opus_caps:?}");

        // Mono should be in there of course
        assert!(allowed_opus_caps.can_intersect(&mono_caps));

        // Make sure mono is the first structure to indicate preference as per receiver caps
        let s = allowed_opus_caps.structure(0).unwrap();
        assert_eq!(s.get::<i32>("channels"), Ok(1));

        // Stereo should still be allowed, since stereo=0 is just a hint
        assert!(allowed_opus_caps.can_intersect(&stereo_caps));

        // Multichannel of course
        let multich_caps = gst::Caps::builder("audio/x-opus")
            .field("channel-mapping-family", 1i32)
            .field("channels", gst::IntRange::new(3, 255))
            .build();

        assert!(allowed_opus_caps.can_intersect(&multich_caps));

        // Make sure it's channel-mapping-family=0 in the first two structs
        for i in 0..3 {
            let s = allowed_opus_caps.structure(i).unwrap();
            assert!(s.has_name("audio/x-opus"));
            assert_eq!(
                s.get::<i32>("channel-mapping-family"),
                if i < 2 { Ok(0) } else { Ok(1) }
            );
        }
    }

    // Not testing MULTIOPUS, OPUS preference order, doesn't seem very interesting in practice.
}

// test_opus_pay_depay_multichannel
//
// Check basic payloading/depayloading with multichannel audio (MULTIOPUS extension)
//
#[test]
fn test_opus_pay_depay_multichannel() {
    // gst-launch-1.0 audiotestsrc wave=ticks ! audio/x-raw,channels=6 ! opusenc ! multifilesink
    const OPUS_BUFFERS: &[&[u8]] = &[
        include_bytes!("audiotestsrc-6ch-48kHz-000.opus").as_slice(),
        include_bytes!("audiotestsrc-6ch-48kHz-001.opus").as_slice(),
        include_bytes!("audiotestsrc-6ch-48kHz-002.opus").as_slice(),
    ];

    init();

    let input_caps = gst::Caps::builder("audio/x-opus")
        .field("rate", 48000i32)
        .field("channels", 6i32)
        .field("channel-mapping-family", 1i32)
        .field("stream-count", 4i32)
        .field("coupled-count", 2i32)
        .field("channel-mapping", gst::Array::new([0i32, 4, 1, 2, 3, 5]))
        .build();

    let input_buffers = vec![
        make_buffer(
            OPUS_BUFFERS[0],
            gst::ClockTime::ZERO,
            gst::ClockTime::from_mseconds(20), // Note: no ClippingMeta for lead-in here unlike opusenc
            gst::BufferFlags::DISCONT,
        ),
        make_buffer(
            OPUS_BUFFERS[1],
            gst::ClockTime::from_mseconds(20),
            gst::ClockTime::from_mseconds(20),
            gst::BufferFlags::empty(),
        ),
        make_buffer(
            OPUS_BUFFERS[2],
            gst::ClockTime::from_mseconds(40),
            gst::ClockTime::from_mseconds(20),
            gst::BufferFlags::empty(),
        ),
    ];

    // TODO: check durations?
    let expected_pay = vec![
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::ZERO)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::MARKER)
            .pt(96)
            .rtp_time(0)
            .marker_bit(true)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960)
            .marker_bit(false)
            .build()],
        vec![ExpectedPacket::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .flags(gst::BufferFlags::empty())
            .pt(96)
            .rtp_time(960 + 960)
            .marker_bit(false)
            .build()],
    ];

    // TODO: check durations?
    let expected_depay = vec![
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::ZERO)
            .size(143)
            .flags(gst::BufferFlags::DISCONT | gst::BufferFlags::RESYNC)
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(20))
            .size(129)
            .flags(gst::BufferFlags::empty())
            .build()],
        vec![ExpectedBuffer::builder()
            .pts(gst::ClockTime::from_mseconds(40))
            .size(133)
            .flags(gst::BufferFlags::empty())
            .build()],
    ];

    let expected_depay_caps = input_caps.clone();

    run_test_pipeline_full(
        Source::Buffers(input_caps, input_buffers),
        "rtpopuspay2",
        "rtpopusdepay2",
        expected_pay,
        expected_depay,
        Some(expected_depay_caps),
        Liveness::NonLive,
    );
}

// test_opus_depay_pay_multichannel
//
// Check basic depayloader ! payloader compatibility, MULTIOPUS edition
//
#[test]
fn test_opus_depay_pay_multichannel() {
    // gst-launch-1.0 audiotestsrc wave=silence
    //  ! audio/x-raw,channels=6
    //  ! opusenc frame-size=5
    //  ! rtpopuspay ! fakesink dump=true
    const OPUS_RTP_BUFFER: &[u8] = include_bytes!("audiotestsrc-6ch-48kHz-5ms-000.rtp").as_slice();

    init();

    let mut h = Harness::new_parse("rtpopusdepay2 ! rtpopuspay2");

    let input_caps = gst::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("encoding-name", "MULTIOPUS")
        .field("clock-rate", 48000i32)
        .field("payload", 96i32)
        .field("num_streams", "4")
        .field("coupled_streams", "2")
        .field("encoding-params", "6")
        .field("sprop-maxcapturerate", "48000")
        .field("channel_mapping", "0,4,1,2,3,5")
        .build();

    let expected_output_caps = input_caps.clone();

    h.set_src_caps(input_caps);

    let input_buffer = make_buffer(
        OPUS_RTP_BUFFER,
        gst::ClockTime::ZERO,
        gst::ClockTime::from_mseconds(5),
        gst::BufferFlags::DISCONT,
    );

    h.push(input_buffer)
        .expect("Got error flow when pushing buffer");

    let _output_buffer = h.pull().expect("Didn't get output buffer");

    let output_caps = h
        .srcpad()
        .expect("harness srcpad")
        .current_caps()
        .expect("output caps");

    eprintln!("Output caps: {output_caps}");

    assert!(output_caps.can_intersect(&expected_output_caps));
}
