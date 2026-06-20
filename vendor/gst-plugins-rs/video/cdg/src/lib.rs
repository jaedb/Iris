// Copyright (C) 2019 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-cdg:
 *
 * Since: plugins-rs-0.5.0
 */
use gst::glib;

mod cdgdec;
mod cdgparse;
mod constants;
mod typefind;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    cdgdec::register(plugin)?;
    cdgparse::register(plugin)?;
    typefind::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    cdg,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
