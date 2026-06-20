// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-awstranslate
 *
 * `awstranslate` is an element that can be used to translate text from one
 * language to another.
 */
use gst::glib;
use gst::prelude::*;

mod imp;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "awstranslate",
        gst::DebugColorFlags::empty(),
        Some("AWS translate element"),
    )
});

glib::wrapper! {
    pub struct Translate(ObjectSubclass<imp::Translate>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "awstranslate",
        gst::Rank::NONE,
        Translate::static_type(),
    )
}
