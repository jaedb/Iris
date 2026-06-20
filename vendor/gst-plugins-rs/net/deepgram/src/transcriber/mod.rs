// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "deepgramtranscriber",
        gst::DebugColorFlags::empty(),
        Some("Deepgram transcribe element"),
    )
});

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstDeepgramInterimStrategy")]
#[non_exhaustive]
pub enum DeepgramInterimStrategy {
    #[enum_value(name = "Disabled: don't use interim results", nick = "disabled")]
    Disabled = 0,
    #[enum_value(
        name = "Index: track current word in interim results by index",
        nick = "index"
    )]
    Index = 1,
    #[enum_value(
        name = "Timing: track current word in interim results by timing",
        nick = "timing"
    )]
    Timing = 2,
}

glib::wrapper! {
    pub struct Transcriber(ObjectSubclass<imp::Transcriber>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        DeepgramInterimStrategy::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "deepgramtranscriber",
        gst::Rank::NONE,
        Transcriber::static_type(),
    )
}
