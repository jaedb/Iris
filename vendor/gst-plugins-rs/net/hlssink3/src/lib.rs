// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-hlssink3:
 *
 * Since: plugins-rs-0.8.0
 */
use gst::glib;

mod hlsbasesink;
pub mod hlscmafsink;
pub mod hlssink3;
mod playlist;

glib::wrapper! {
    pub struct HlsBaseSinkGioOutputStream(ObjectSubclass<hlsbasesink::HlsBaseSinkGioOutputStream>) @extends gio::OutputStream;
}

unsafe impl Send for HlsBaseSinkGioOutputStream {}
unsafe impl Sync for HlsBaseSinkGioOutputStream {}

glib::wrapper! {
    pub struct HlsBaseSink(ObjectSubclass<hlsbasesink::HlsBaseSink>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;
        HlsBaseSink::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        hlsbasesink::HlsProgramDateTimeReference::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    hlssink3::register(plugin)?;
    hlscmafsink::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    hlssink3,
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
