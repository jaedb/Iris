// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// Example command-line:
//
// gst-launch-1.0 filesrc location=animated.webp ! rswebpdec ! videoconvert ! autovideosink

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct WebPDec(ObjectSubclass<imp::WebPDec>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rswebpdec",
        gst::Rank::PRIMARY,
        WebPDec::static_type(),
    )
}
