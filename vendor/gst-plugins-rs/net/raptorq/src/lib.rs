// Copyright (C) 2022 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]
#![doc = include_str!("../README.md")]

/**
 * plugin-raptorq:
 *
 * Since: plugins-rs-0.9.0
 */
use gst::glib;

mod fecscheme;
mod raptorqdec;
mod raptorqenc;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    raptorqdec::register(plugin)?;
    raptorqenc::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    raptorq,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
