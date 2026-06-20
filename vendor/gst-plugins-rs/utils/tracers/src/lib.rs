// Copyright (C) 2022 OneStream Live <guillaume.desmottes@onestream.live>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-rstracers:
 *
 * Since: plugins-rs-0.9.0
 */
use gst::glib;

mod buffer_lateness;
#[cfg(feature = "v1_26")]
mod memory_tracer;
mod pad_push_timings;
mod pcap_writer;
#[cfg(unix)]
mod pipeline_snapshot;
mod queue_levels;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(unix)]
    pipeline_snapshot::register(plugin)?;
    queue_levels::register(plugin)?;
    buffer_lateness::register(plugin)?;
    pad_push_timings::register(plugin)?;
    pcap_writer::register(plugin)?;
    #[cfg(feature = "v1_26")]
    memory_tracer::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    rstracers,
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
