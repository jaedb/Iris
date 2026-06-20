// Copyright (C) 2025 Carlos Bentzen <cadubentzen@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * plugin-vvdec:
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;

mod dec;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    dec::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    vvdec,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
