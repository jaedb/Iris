// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use pretty_assertions::assert_eq;

use cea708_types::tables::*;
use cea708_types::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().unwrap();
    });
}

struct TestState {
    cc_data_parser: CCDataParser,
    h: gst_check::Harness,
}

impl TestState {
    fn new() -> Self {
        let mut h = gst_check::Harness::new_parse("cea608tocea708");
        h.set_src_caps_str("closedcaption/x-cea-608,format=raw,field=0,framerate=25/1");
        h.set_sink_caps_str("closedcaption/x-cea-708,format=cc_data");

        Self {
            cc_data_parser: CCDataParser::new(),
            h,
        }
    }

    fn push_data(&mut self, input: &[u8], pts: gst::ClockTime, output: &[Code]) {
        let mut buf = gst::Buffer::from_mut_slice(input.to_vec());
        {
            let buf = buf.get_mut().unwrap();
            buf.set_pts(pts);
        }
        assert_eq!(self.h.push(buf), Ok(gst::FlowSuccess::Ok));
        let out_buf = self.h.try_pull().unwrap();
        let data = out_buf.map_readable().unwrap();

        // construct the two byte header for cc_data that GStreamer doesn't write
        let mut complete_cc_data = vec![];
        complete_cc_data.extend([0x80 | 0x40 | ((data.len() / 3) & 0x1f) as u8, 0xff]);
        complete_cc_data.extend(&*data);
        println!("{}, {input:X?} {complete_cc_data:X?}", pts.display());

        self.cc_data_parser.push(&complete_cc_data).unwrap();
        let mut output_iter = output.iter();
        while let Some(packet) = self.cc_data_parser.pop_packet() {
            for service in packet.services() {
                for (ci, code) in service.codes().iter().enumerate() {
                    println!(
                        "{}, P{}: S{}: C{ci}: {code:?}",
                        pts.display(),
                        packet.sequence_no(),
                        service.number()
                    );
                    assert_eq!(output_iter.next(), Some(code));
                }
            }
        }
        assert_eq!(out_buf.pts(), Some(pts));
    }
}

#[test]
fn test_single_char() {
    init();

    let test_data = [([0xC1, 0x80], vec![Code::LatinCapitalA])];

    let mut state = TestState::new();

    for (i, d) in test_data.iter().enumerate() {
        let ts = gst::ClockTime::from_mseconds((i as u64) * 13);
        state.push_data(&d.0, ts, &d.1);
    }

    let caps = state
        .h
        .sinkpad()
        .expect("harness has no sinkpad")
        .current_caps()
        .expect("pad has no caps");
    assert_eq!(
        caps,
        gst::Caps::builder("closedcaption/x-cea-708")
            .field("format", "cc_data")
            .field("framerate", gst::Fraction::new(25, 1))
            .build()
    );
}

#[test]
fn test_rollup() {
    init();

    let test_data = [
        ([0x94, 0x2C], vec![Code::ClearWindows(WindowBits::ZERO)]), // EDM -> ClearWindows(1)
        (
            [0x94, 0x26],
            vec![
                Code::DeleteWindows(!WindowBits::ZERO),
                Code::DefineWindow(DefineWindowArgs::new(
                    0,
                    0,
                    Anchor::BottomMiddle,
                    true,
                    100,
                    50,
                    2,
                    31,
                    true,
                    true,
                    true,
                    2,
                    1,
                )),
                Code::SetPenLocation(SetPenLocationArgs::new(2, 0)),
                Code::ETX,
            ],
        ), // RU3 -> DeleteWindows(!0), DefineWindow(0...), SetPenLocation(bottom-row)
        ([0x94, 0xAD], vec![Code::CR, Code::ETX]),                  // CR -> CR
        ([0x94, 0x70], vec![Code::ETX]), // PAC to bottom left -> (pen already there) -> nothing to do
        (
            [0xA8, 0x43],
            vec![Code::LeftParenthesis, Code::LatinCapitalC, Code::ETX],
        ), // text: (C -> (C
        (
            [0x94, 0x26],
            vec![
                Code::DeleteWindows(!WindowBits::ZERO),
                Code::DefineWindow(DefineWindowArgs::new(
                    0,
                    0,
                    Anchor::BottomMiddle,
                    true,
                    100,
                    50,
                    2,
                    31,
                    true,
                    true,
                    true,
                    2,
                    1,
                )),
                Code::SetPenLocation(SetPenLocationArgs::new(2, 0)),
                Code::ETX,
            ],
        ), // RU3
        ([0x94, 0xAD], vec![Code::CR, Code::ETX]), // CR -> CR
        ([0x94, 0x70], vec![Code::ETX]), // PAC to bottom left -> SetPenLocation(...)
        (
            [0xF2, 0xEF],
            vec![Code::LatinLowerR, Code::LatinLowerO, Code::ETX],
        ), // ro
    ];

    let mut state = TestState::new();

    for (i, d) in test_data.iter().enumerate() {
        let ts = gst::ClockTime::from_mseconds((i as u64) * 13);
        state.push_data(&d.0, ts, &d.1);
    }
}
