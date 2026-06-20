// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//G
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnquicmux:
 * @short-description: Supports stream multiplexing in QUIC
 *
 */
use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct QuinnQuicMux(ObjectSubclass<imp::QuinnQuicMux>) @extends gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct QuinnQuicMuxPad(ObjectSubclass<imp::QuinnQuicMuxPad>) @extends gst_base::AggregatorPad, gst::ProxyPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        QuinnQuicMuxPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "quinnquicmux",
        gst::Rank::NONE,
        QuinnQuicMux::static_type(),
    )
}
