// Copyright (C) 2017 Author: Arun Raghavan <arun@arunraghavan.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-aws:
 *
 * Since: plugins-rs-0.1
 */
use gst::glib;

mod polly;
mod s3arn;
mod s3hlssink;
mod s3sink;
mod s3src;
mod s3url;
pub mod s3utils;
mod transcribe_parse;
mod transcriber;
mod transcriber2;
mod translate;

pub use transcriber::AwsTranscriberResultStability;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    s3sink::register(plugin)?;
    s3src::register(plugin)?;
    transcribe_parse::register(plugin)?;
    transcriber::register(plugin)?;
    transcriber2::register(plugin)?;
    translate::register(plugin)?;
    s3hlssink::register(plugin)?;
    polly::register(plugin)?;

    if !gst::meta::CustomMeta::is_registered("AWSTranscribeItemMeta") {
        gst::meta::CustomMeta::register("AWSTranscribeItemMeta", &[]);
    }

    Ok(())
}

gst::plugin_define!(
    aws,
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
