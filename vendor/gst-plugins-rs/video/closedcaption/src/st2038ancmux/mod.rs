// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
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
    pub struct St2038AncMux(ObjectSubclass<imp::St2038AncMux>) @extends gst_base::Aggregator, gst::Element, gst::Object;
}

glib::wrapper! {
    pub struct St2038AncMuxSinkPad(ObjectSubclass<imp::St2038AncMuxSinkPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    St2038AncMuxSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    gst::Element::register(
        Some(plugin),
        "st2038ancmux",
        gst::Rank::NONE,
        St2038AncMux::static_type(),
    )
}
