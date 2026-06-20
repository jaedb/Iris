// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-textaccumulate
 *
 * `textaccumulate` is an element that can be used to accumulate text prior to
 * translation or synthesis.
 *
 * When working with live data, the element will accumulate input text until
 * the deadline is reached, comparing the current running time with the running
 * time of the input items and the upstream latency.
 *
 * At this point the input items will be drained up until a configurable pattern
 * is encountered, see the #GstTextAccumulate:timeout-terminators for more information.
 *
 * If the pattern does not match, the entire accumulator is drained.
 *
 * The accumulator will also be drained upon receiving `rstranscribe/final-transcript`
 * and `rstranscribe/speaker-change` custom events.
 *
 * When the user wants to use a very low / no latency upstream, and is willing to
 * accept desynchronization in order to build up long-enough sentences for translation,
 * they can set the #GstTextAccumulate:lateness property to shift the input
 * timestamps forward when accumulating.
 */
use gst::glib;
use gst::prelude::*;

mod imp;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "textaccumulate",
        gst::DebugColorFlags::empty(),
        Some("Text accumulator element"),
    )
});

glib::wrapper! {
    pub struct Accumulate(ObjectSubclass<imp::Accumulate>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "textaccumulate",
        gst::Rank::NONE,
        Accumulate::static_type(),
    )
}
