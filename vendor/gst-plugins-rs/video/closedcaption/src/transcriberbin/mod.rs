// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstTranscriberBinCaptionSource")]
pub enum CaptionSource {
    Both,
    Transcription,
    Inband,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstTranscriberBinMuxMethod")]
enum MuxMethod {
    #[default]
    Cea608,
    Cea708,
}

glib::wrapper! {
    pub struct TranscriberBin(ObjectSubclass<imp::TranscriberBin>) @extends gst::Bin, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct TranscriberSinkPad(ObjectSubclass<imp::TranscriberSinkPad>) @extends gst::GhostPad, gst::ProxyPad, gst::Pad, gst::Object;
}

glib::wrapper! {
    pub struct TranscriberSrcPad(ObjectSubclass<imp::TranscriberSrcPad>) @extends gst::GhostPad, gst::ProxyPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        CaptionSource::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        MuxMethod::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        TranscriberSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        TranscriberSrcPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "transcriberbin",
        gst::Rank::NONE,
        TranscriberBin::static_type(),
    )
}
