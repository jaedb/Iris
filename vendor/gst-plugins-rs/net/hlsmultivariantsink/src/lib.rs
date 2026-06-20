// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * plugin-hlsmultivariantsink:
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;
use gst::prelude::*;

#[allow(unused)]
mod cros_codecs;
mod imp;

#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstHlsMultivariantSinkMuxerType")]
#[non_exhaustive]
pub enum HlsMultivariantSinkMuxerType {
    #[default]
    #[enum_value(name = "cmaf", nick = "cmaf")]
    Cmaf = 0,

    #[enum_value(name = "mpegts", nick = "mpegts")]
    MpegTs = 1,
}

#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstHlsMultivariantSinkAlternativeMediaType")]
#[non_exhaustive]
pub enum HlsMultivariantSinkAlternativeMediaType {
    #[default]
    #[enum_value(name = "AUDIO", nick = "audio")]
    Audio = 0,

    #[enum_value(name = "VIDEO", nick = "video")]
    Video = 1,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstHlsMultivariantSinkPlaylistType")]
#[non_exhaustive]
pub enum HlsMultivariantSinkPlaylistType {
    #[enum_value(
        name = "Unspecified: The tag `#EXT-X-PLAYLIST-TYPE` won't be present in the playlist during the pipeline processing.",
        nick = "unspecified"
    )]
    Unspecified = 0,

    #[enum_value(
        name = "Event: No segments will be removed from the playlist. At the end of the processing, the tag `#EXT-X-ENDLIST` is added to the playlist. The tag `#EXT-X-PLAYLIST-TYPE:EVENT` will be present in the playlist.",
        nick = "event"
    )]
    Event = 1,

    #[enum_value(
        name = "Vod: The playlist behaves like the `event` option (a live event), but at the end of the processing, the playlist will be set to `#EXT-X-PLAYLIST-TYPE:VOD`.",
        nick = "vod"
    )]
    Vod = 2,
}

glib::wrapper! {
    pub struct HlsMultivariantSink(ObjectSubclass<imp::HlsMultivariantSink>) @extends gst::Bin, gst::Element, gst::Object, @implements gst::ChildProxy;

}

glib::wrapper! {
    pub(crate) struct HlsMultivariantSinkPad(ObjectSubclass<imp::HlsMultivariantSinkPad>) @extends gst::GhostPad, gst::ProxyPad, gst::Pad, gst::Object;
}

pub fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        HlsMultivariantSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        HlsMultivariantSinkMuxerType::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        HlsMultivariantSinkPlaylistType::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        HlsMultivariantSinkAlternativeMediaType::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "hlsmultivariantsink",
        gst::Rank::NONE,
        HlsMultivariantSink::static_type(),
    )
}

gst::plugin_define!(
    hlsmultivariantsink,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
