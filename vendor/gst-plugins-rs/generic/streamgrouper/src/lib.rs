// Copyright (C) 2024 Igalia S.L. <aboya@igalia.com>
// Copyright (C) 2024 Comcast <aboya@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(unused_doc_comments)]
use gst::glib;

mod streamgrouper;

/**
 * plugin-streamgrouper:
 *
 * Since: plugins-rs-0.14.0
 */

gst::plugin_define!(
    streamgrouper,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    streamgrouper::register(plugin).unwrap();
    Ok(())
}
