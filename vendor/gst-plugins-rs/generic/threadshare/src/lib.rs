// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

//! A collection of GStreamer plugins which leverage the `threadshare` [`runtime`].
//!
//! [`runtime`]: runtime/index.html
/**
 * plugin-threadshare:
 *
 * Since: plugins-rs-0.4.0
 */
#[macro_use]
pub mod runtime;

mod appsrc;
mod audiotestsrc;
mod blocking_adapter;
pub mod dataqueue;
mod inputselector;
mod inter;
mod proxy;
mod queue;
mod rtpdtmfsrc;
pub mod socket;
mod tcpclientsrc;
mod udpsink;
mod udpsrc;

pub mod net;

use gst::glib;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    appsrc::register(plugin)?;
    audiotestsrc::register(plugin)?;
    blocking_adapter::register(plugin)?;
    inputselector::register(plugin)?;
    inter::register(plugin)?;
    proxy::register(plugin)?;
    queue::register(plugin)?;
    rtpdtmfsrc::register(plugin)?;
    tcpclientsrc::register(plugin)?;
    udpsink::register(plugin)?;
    udpsrc::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    threadshare,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    // FIXME: MPL-2.0 is only allowed since GStreamer 1.18.3 (as unknown) and 1.20 (as known)
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
