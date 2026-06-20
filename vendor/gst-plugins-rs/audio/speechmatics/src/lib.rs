// Copyright (C) 2024 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]
#![recursion_limit = "128"]

/**
 * plugin-speechmatics:
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;

mod transcriber;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    transcriber::register(plugin)?;

    if !gst::meta::CustomMeta::is_registered("SpeechmaticsItemMeta") {
        gst::meta::CustomMeta::register("SpeechmaticsItemMeta", &[]);
    }

    Ok(())
}

gst::plugin_define!(
    speechmatics,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "Proprietary",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
