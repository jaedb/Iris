// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use crate::mparobust::{peek_frame_header, FrameHeader};

use std::sync::LazyLock;

static ONE_CHAN_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    gst::Caps::builder("audio/mpeg")
        .field("mpegversion", 1i32)
        .field("mpegaudioversion", 1i32)
        .field("layer", 3i32)
        .field("rate", 44_100i32)
        .field("channels", 1i32)
        .field("parsed", true)
        .build()
});

static TWO_CHANS_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    gst::Caps::builder("audio/mpeg")
        .field("mpegversion", 1i32)
        .field("mpegaudioversion", 1i32)
        .field("layer", 3i32)
        .field("rate", 44_100i32)
        .field("channels", 2i32)
        .field("parsed", true)
        .build()
});

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        crate::plugin_register_static().expect("rtpmparobust test");
    });
}

fn run_test(stream: &'static [u8], caps: &gst::Caps, frame_checker: impl Fn(i32, FrameHeader)) {
    let pipe = gst::parse::launch(
        r#"appsrc name=src is-live=true emit-signals=false ! gdpdepay
        ! rtpmparobustdepay2
        ! appsink name=sink sync=false
        "#,
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    let src = pipe
        .by_name("src")
        .and_downcast::<gst_app::AppSrc>()
        .unwrap();

    let sink = pipe
        .by_name("sink")
        .and_downcast::<gst_app::AppSink>()
        .unwrap();

    pipe.set_state(gst::State::Playing).unwrap();

    src.push_buffer(gst::Buffer::from_slice(stream)).unwrap();
    src.end_of_stream().unwrap();

    let info = gst_audio::AudioInfo::from_caps(caps).unwrap();
    let s = caps.structure(0).unwrap();
    let layer = s.get::<i32>("layer").unwrap();
    let mpegversion = s.get::<i32>("mpegversion").unwrap();

    let mut next_pts = gst::ClockTime::ZERO;
    let mut i = 0;
    loop {
        if sink.is_eos() {
            break;
        }

        let Some(obj) = sink.try_pull_object(gst::ClockTime::NONE) else {
            continue;
        };
        let Some(sample) = obj.downcast_ref::<gst::Sample>() else {
            continue;
        };
        let buf = sample.buffer().unwrap();
        let cur_pts = buf.pts().unwrap();

        if i == 0 {
            assert_eq!(sample.caps().unwrap(), caps);
        } else {
            assert_eq!(cur_pts, next_pts);
        }

        next_pts = cur_pts + buf.duration().unwrap();

        let data = buf.map_readable().unwrap();
        let hdr = peek_frame_header(data.as_slice()).unwrap();

        assert_eq!(hdr.sample_rate as u32, info.rate());
        assert_eq!(hdr.channels as u32, info.channels());
        assert_eq!(hdr.layer as i32, layer);
        assert_eq!(hdr.version as i32, mpegversion);

        frame_checker(i, hdr);

        i += 1;
    }

    pipe.set_state(gst::State::Null).unwrap();
}

#[test]
fn mpa_robust_1ch() {
    init();

    run_test(
        include_bytes!("files/mpa_robust.sin.1ch.gdp"),
        &ONE_CHAN_CAPS,
        |i, hdr| {
            assert_eq!(hdr.samples_per_frame, 1152u16);

            if hdr.frame_len
                != match i {
                    56 => 313,
                    57 => 731,
                    58 => 130,
                    _ => 104,
                }
            {
                panic!("Unexpected frame len {} for {i}", hdr.frame_len);
            }
        },
    );
}

#[test]
fn mpa_robust_1ch_interleaved() {
    init();

    run_test(
        include_bytes!("files/mpa_robust.sin.1ch.interleaved.gdp"),
        &ONE_CHAN_CAPS,
        |i, hdr| {
            assert_eq!(hdr.samples_per_frame, 1152u16);

            if hdr.frame_len != 104 {
                panic!("Unexpected frame len {} for {i}", hdr.frame_len);
            }
        },
    );
}

fn two_chan_checker(i: i32, hdr: FrameHeader) {
    assert_eq!(hdr.samples_per_frame, 1152u16);

    if hdr.frame_len
        != match i {
            37 => 261,
            38 => 626,
            39 => 130,
            76 => 626,
            77 => 261,
            114 => 731,
            115 => 365,
            152 => 417,
            153 => 626,
            190 => 261,
            191 => 731,
            192 => 130,
            229 => 731,
            230 => 313,
            267 => 522,
            268 => 522,
            305 => 261,
            306 => 626,
            344 => 835,
            _ => 104,
        }
    {
        panic!("Unexpected frame len {} for {i}", hdr.frame_len);
    }
}

#[test]
fn mpa_robust_2ch() {
    init();

    run_test(
        include_bytes!("files/mpa_robust.2ch.gdp"),
        &TWO_CHANS_CAPS,
        two_chan_checker,
    );
}

#[test]
fn mpa_robust_2ch_interleaved() {
    init();

    run_test(
        include_bytes!("files/mpa_robust.2ch.interleaved.gdp"),
        &TWO_CHANS_CAPS,
        two_chan_checker,
    );
}
