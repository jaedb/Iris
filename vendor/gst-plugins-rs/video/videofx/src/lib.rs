// Copyright (C) 2021, Daily
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-rsvideofx:
 *
 * Since: plugins-rs-0.8.0
 */
#[cfg(feature = "doc")]
use gst::prelude::*;

mod border;
mod colordetect;
mod videocompare;

pub use videocompare::{HashAlgorithm, PadDistance, VideoCompareMessage};

fn plugin_init(plugin: &gst::Plugin) -> Result<(), gst::glib::BoolError> {
    #[cfg(feature = "doc")]
    HashAlgorithm::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    border::register(plugin)?;
    colordetect::register(plugin)?;
    videocompare::register(plugin)
}

gst::plugin_define!(
    rsvideofx,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    // FIXME: MPL-2.0 is only allowed since 1.18.3 (as unknown) and 1.20 (as known)
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
