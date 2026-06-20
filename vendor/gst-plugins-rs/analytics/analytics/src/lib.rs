// Copyright (C) 2024 Benjamin Gaignard <benjamin.gaignard@collabora.com>
// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-rsanalytics:
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;

pub(crate) const ONVIF_METADATA_SCHEMA: &str = "http://www.onvif.org/ver10/schema";
pub(crate) const ONVIF_METADATA_PREFIX: &str = "tt";

mod onvifmeta2relationmeta;
mod relationmeta2onvifmeta;

#[cfg(feature = "v1_28")]
mod combiner;
#[cfg(feature = "v1_28")]
mod splitter;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    relationmeta2onvifmeta::register(plugin)?;
    onvifmeta2relationmeta::register(plugin)?;

    if !gst::meta::CustomMeta::is_registered("OnvifXMLFrameMeta") {
        gst::meta::CustomMeta::register("OnvifXMLFrameMeta", &[]);
    }

    #[cfg(feature = "v1_28")]
    {
        combiner::register(plugin)?;
        splitter::register(plugin)?;
    }

    Ok(())
}

gst::plugin_define!(
    rsanalytics,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
