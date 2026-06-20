// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// Example command-line:
//
// gst-launch-1.0 cccombiner name=ccc ! cea608overlay ! autovideosink \
//   videotestsrc ! video/x-raw, width=1280, height=720 ! queue ! ccc.sink \
//   filesrc location=input.srt ! subparse ! tttocea608 ! queue ! ccc.caption

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct Cea608Overlay(ObjectSubclass<imp::Cea608Overlay>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "cea608overlay",
        gst::Rank::PRIMARY,
        Cea608Overlay::static_type(),
    )
}
