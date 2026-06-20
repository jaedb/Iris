// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

use crate::HlsBaseSink;
/**
 * plugin-hlssink3:
 *
 * Since: plugins-rs-0.8.0
 */
use gst::glib;
use gst::prelude::*;

mod imp;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstHlsSink3PlaylistType")]
#[non_exhaustive]
pub enum HlsSink3PlaylistType {
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
    pub struct HlsSink3(ObjectSubclass<imp::HlsSink3>) @extends HlsBaseSink, gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        HlsSink3PlaylistType::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "hlssink3",
        gst::Rank::NONE,
        HlsSink3::static_type(),
    )?;

    Ok(())
}
