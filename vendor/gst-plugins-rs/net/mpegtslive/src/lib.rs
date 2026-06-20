// Copyright (C) 2024 Edward Hervey <edward@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-mpegtslive:
 *
 * Since: plugins-rs-0.13.0
 */
use gst::glib;

mod mpegtslive;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    mpegtslive::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    mpegtslive,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
