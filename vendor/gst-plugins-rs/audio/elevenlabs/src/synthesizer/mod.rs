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

glib::wrapper! {
    pub struct Synthesizer(ObjectSubclass<imp::Synthesizer>) @extends gst::Element, gst::Object;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstElevenLabsOverflow")]
#[non_exhaustive]
pub enum Overflow {
    #[enum_value(name = "Clip", nick = "clip")]
    Clip = 0,
    #[enum_value(name = "Overlap", nick = "overlap")]
    Overlap = 1,
    #[enum_value(name = "Shift", nick = "shift")]
    Shift = 2,
    #[cfg(feature = "signalsmith_stretch")]
    #[enum_value(name = "Compress", nick = "compress")]
    Compress = 3,
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        Overflow::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "elevenlabssynthesizer",
        gst::Rank::NONE,
        Synthesizer::static_type(),
    )
}
