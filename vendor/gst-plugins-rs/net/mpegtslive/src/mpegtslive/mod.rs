// Copyright (C) 2024 Edward Hervey <edward@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;
mod parser;

glib::wrapper! {
    pub struct MpegTsLiveSource(ObjectSubclass<imp::MpegTsLiveSource>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "mpegtslivesrc",
        gst::Rank::NONE,
        MpegTsLiveSource::static_type(),
    )
}
