// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
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
    pub struct Cea708Mux(ObjectSubclass<imp::Cea708Mux>) @extends gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct Cea708MuxSinkPad(ObjectSubclass<imp::Cea708MuxSinkPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    Cea708MuxSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    gst::Element::register(
        Some(plugin),
        "cea708mux",
        gst::Rank::NONE,
        Cea708Mux::static_type(),
    )
}
