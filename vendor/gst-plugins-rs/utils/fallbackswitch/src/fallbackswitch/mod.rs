// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2021 Jan Schmidt <jan@centricular.com>
// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2022 Vivia Nikolaidou <vivia.nikolaidou@ltnglobal.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct FallbackSwitch(ObjectSubclass<imp::FallbackSwitch>) @extends gst::Element, gst::Object, @implements gst::ChildProxy;
}

// The public Rust wrapper type for our sink pad
glib::wrapper! {
    pub struct FallbackSwitchSinkPad(ObjectSubclass<imp::FallbackSwitchSinkPad>) @extends gst::Pad, gst::Object;
}

// Registers the type for our element, and then registers in GStreamer under
// the name "fallbackswitch" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    FallbackSwitchSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    gst::Element::register(
        Some(plugin),
        "fallbackswitch",
        gst::Rank::NONE,
        FallbackSwitch::static_type(),
    )
}
