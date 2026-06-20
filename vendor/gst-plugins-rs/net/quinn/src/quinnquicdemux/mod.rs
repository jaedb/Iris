// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//G
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnquicdemux:
 * @short-description: Supports stream de-multiplexing in QUIC
 *
 */
use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct QuinnQuicDemux(ObjectSubclass<imp::QuinnQuicDemux>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "quinnquicdemux",
        gst::Rank::NONE,
        QuinnQuicDemux::static_type(),
    )
}
