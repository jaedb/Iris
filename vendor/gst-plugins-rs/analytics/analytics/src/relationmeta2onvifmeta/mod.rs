// Copyright (C) 2024 Benjamin Gaignard <benjamin.gaignard@collabora.com>
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
    pub struct RelationMeta2OnvifMeta(ObjectSubclass<imp::RelationMeta2OnvifMeta>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    imp::TimeSource::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    gst::Element::register(
        Some(plugin),
        "relationmeta2onvifmeta",
        gst::Rank::NONE,
        RelationMeta2OnvifMeta::static_type(),
    )
}
