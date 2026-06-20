// Copyright (C) 2025 Carlos Bentzen <cadubentzen@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use std::fs;
use std::path::PathBuf;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstvvdec::plugin_register_static().expect("vvdec test");
    });
}

#[test]
fn test_decode_yuv420p10le() {
    init();
    test_decode("yuv420p10le");
}

fn test_decode(name: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("tests/vvc_{name}.mkv"));

    let bin = gst::parse::bin_from_description(
        &format!("filesrc location={path:?} ! matroskademux ! h266parse ! vvdec name=vvdec"),
        false,
    )
    .unwrap();

    let srcpad = bin.by_name("vvdec").unwrap().static_pad("src").unwrap();
    let _ = bin.add_pad(&gst::GhostPad::with_target(&srcpad).unwrap());

    let mut h = gst_check::Harness::with_element(&bin, None, Some("src"));

    h.play();

    let buf = h.pull().unwrap();
    let frame = buf.into_mapped_buffer_readable().unwrap();

    let mut refpath = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    refpath.push(format!("tests/vvc_{name}.ref"));

    let ref_frame = fs::read(refpath).unwrap();

    assert_eq!(frame.len(), ref_frame.len());
    assert_eq!(frame.as_slice(), glib::Bytes::from_owned(ref_frame));
}
