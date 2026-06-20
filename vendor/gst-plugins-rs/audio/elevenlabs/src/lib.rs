// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]
#![recursion_limit = "128"]

use gst::glib;
/**
 * plugin-elevenlabs:
 *
 * Since: plugins-rs-0.14.0
 */
use std::sync::LazyLock;
use tokio::runtime;

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

mod cloner;
mod synthesizer;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    synthesizer::register(plugin)?;
    cloner::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    elevenlabs,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "Proprietary",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
