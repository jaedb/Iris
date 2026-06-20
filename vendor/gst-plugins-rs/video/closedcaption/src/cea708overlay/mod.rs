// Copyright (C) 2024 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// Example command-line:
//
// gst-launch-1.0 cccombiner name=ccc ! cea708overlay ! autovideosink \
//   videotestsrc ! video/x-raw, width=1280, height=720 ! queue ! ccc.sink \
//   filesrc location=input.srt ! subparse ! tttocea708 ! queue ! ccc.caption

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct Cea708Overlay(ObjectSubclass<imp::Cea708Overlay>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "cea708overlay",
        gst::Rank::PRIMARY,
        Cea708Overlay::static_type(),
    )
}
