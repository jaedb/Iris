// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(
    clippy::non_send_fields_in_send_ty,
    unused_doc_comments,
    unused_imports
)]

/**
 * plugin-quinn:
 *
 * Since: plugins-rs-0.13.0
 */
use gst::glib;
use gst::prelude::*;
mod common;
mod quinnconnection;
mod quinnquicdemux;
pub mod quinnquicmeta;
mod quinnquicmux;
pub mod quinnquicquery;
mod quinnquicsink;
mod quinnquicsrc;
mod quinnroqdemux;
mod quinnroqmux;
mod quinnwtclientsrc;
mod quinnwtserversink;
mod utils;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    env_logger::init();
    #[cfg(feature = "doc")]
    {
        common::QuinnQuicRole::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    quinnquicdemux::register(plugin)?;
    quinnquicmux::register(plugin)?;
    quinnroqmux::register(plugin)?;
    quinnroqdemux::register(plugin)?;
    quinnquicsink::register(plugin)?;
    quinnquicsrc::register(plugin)?;
    quinnwtclientsrc::register(plugin)?;
    quinnwtserversink::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    quinn,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
