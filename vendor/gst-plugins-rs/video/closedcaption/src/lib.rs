// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]
#![recursion_limit = "128"]

/**
 * plugin-rsclosedcaption:
 *
 * Since: plugins-rs-0.4.0
 */
use gst::glib;
#[cfg(feature = "doc")]
use gst::prelude::*;

mod ccdetect;
mod cctost2038anc;
mod ccutils;
mod cdpserviceinject;
mod cea608overlay;
mod cea608tocea708;
mod cea608tojson;
mod cea608tott;
mod cea608utils;
mod cea708mux;
mod cea708overlay;
mod cea708utils;
mod jsontovtt;
mod line_reader;
mod mcc_enc;
mod mcc_parse;
mod parser_utils;
mod scc_enc;
mod scc_parse;
mod st2038anc_utils;
mod st2038ancdemux;
mod st2038ancmux;
mod st2038anctocc;
mod transcriberbin;
mod translationbin;
mod tttocea608;
mod tttocea708;
mod tttojson;
mod ttutils;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        cea608utils::Cea608Mode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        cea708utils::Cea708Mode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    mcc_parse::register(plugin)?;
    mcc_enc::register(plugin)?;
    scc_parse::register(plugin)?;
    scc_enc::register(plugin)?;
    cea608tott::register(plugin)?;
    tttocea608::register(plugin)?;
    cea608overlay::register(plugin)?;
    ccdetect::register(plugin)?;
    tttojson::register(plugin)?;
    cea608tojson::register(plugin)?;
    jsontovtt::register(plugin)?;
    transcriberbin::register(plugin)?;
    translationbin::register(plugin)?;
    cea608tocea708::register(plugin)?;
    cea708mux::register(plugin)?;
    tttocea708::register(plugin)?;
    cea708overlay::register(plugin)?;
    st2038ancdemux::register(plugin)?;
    st2038ancmux::register(plugin)?;
    st2038anctocc::register(plugin)?;
    cctost2038anc::register(plugin)?;
    cdpserviceinject::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    rsclosedcaption,
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
