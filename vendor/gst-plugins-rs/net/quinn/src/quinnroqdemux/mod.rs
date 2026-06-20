// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnroqdemux:
 * @short-description: Supports stream demultiplexing of RTP packets over QUIC
 *
 */
use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct QuinnRoqDemux(ObjectSubclass<imp::QuinnRoqDemux>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "quinnroqdemux",
        gst::Rank::NONE,
        QuinnRoqDemux::static_type(),
    )
}
