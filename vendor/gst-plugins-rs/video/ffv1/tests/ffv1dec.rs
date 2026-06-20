// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2021 Arun Raghavan <arun@asymptotic.io>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;

use std::fs;
use std::path::PathBuf;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstffv1::plugin_register_static().expect("ffv1 test");
    });
}

#[test]
fn test_decode_yuv420p() {
    init();
    test_decode("yuv420p");
}

fn test_decode(name: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("tests/ffv1_v3_{name}.mkv"));

    let bin = gst::parse::bin_from_description(
        &format!(
            "filesrc location={path:?} ! matroskademux name=m m.video_0 ! ffv1dec name=ffv1dec"
        ),
        false,
    )
    .unwrap();

    let srcpad = bin.by_name("ffv1dec").unwrap().static_pad("src").unwrap();
    let _ = bin.add_pad(&gst::GhostPad::with_target(&srcpad).unwrap());

    let mut h = gst_check::Harness::with_element(&bin, None, Some("src"));

    h.play();

    let buf = h.pull().unwrap();
    let frame = buf.into_mapped_buffer_readable().unwrap();

    let mut refpath = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    refpath.push(format!("tests/ffv1_v3_{name}.ref"));

    let ref_frame = fs::read(refpath).unwrap();

    assert_eq!(frame.len(), ref_frame.len());
    assert_eq!(frame.as_slice(), glib::Bytes::from_owned(ref_frame));
}
