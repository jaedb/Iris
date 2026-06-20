// Copyright (C) 2021 Guillaume Desmottes <guillaume@desmottes.be>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use std::sync::LazyLock;

mod imp;

static LOGGER: LazyLock<gst::DebugCategoryLogger> = LazyLock::new(|| {
    gst::DebugCategoryLogger::new(gst::DebugCategory::new(
        "librespot",
        gst::DebugColorFlags::empty(),
        Some("Spotify audio source librespot"),
    ))
});

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, Default, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRsSpotifyBitrate")]
enum Bitrate {
    #[enum_value(name = "96 kbit/s", nick = "96")]
    B96,
    #[default]
    #[enum_value(name = "160 kbit/s", nick = "160")]
    B160,
    #[enum_value(name = "320 kbit/s", nick = "320")]
    B320,
}

impl From<Bitrate> for librespot_playback::config::Bitrate {
    fn from(value: Bitrate) -> Self {
        match value {
            Bitrate::B96 => Self::Bitrate96,
            Bitrate::B160 => Self::Bitrate160,
            Bitrate::B320 => Self::Bitrate320,
        }
    }
}

glib::wrapper! {
    pub struct SpotifyAudioSrc(ObjectSubclass<imp::SpotifyAudioSrc>) @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object, @implements gst::URIHandler;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    Bitrate::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    if let Ok(ref mut filters) = std::env::var("GST_DEBUG_LIBRESPOT") {
        if filters.is_empty() {
            *filters = "librespot=DEBUG".to_string();
        }
        let log_filter = env_filter::Builder::new().parse(filters).build();
        let filtered_logger = env_filter::FilteredLog::new(&(*LOGGER), log_filter);
        log::set_boxed_logger(Box::new(filtered_logger))
            .map(|_| log::set_max_level(log::LevelFilter::Trace))
            .expect("Failed to set librespot category logger");
    }

    gst::Element::register(
        Some(plugin),
        "spotifyaudiosrc",
        gst::Rank::PRIMARY,
        SpotifyAudioSrc::static_type(),
    )
}
