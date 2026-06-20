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
    pub struct TranslationBin(ObjectSubclass<imp::TranslationBin>) @extends gst::Bin, gst::Element, gst::Object;
}

glib::wrapper! {
    pub struct TranslationSrcPad(ObjectSubclass<imp::TranslationSrcPad>) @extends gst::GhostPad, gst::ProxyPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        TranslationSrcPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "translationbin",
        gst::Rank::NONE,
        TranslationBin::static_type(),
    )
}
