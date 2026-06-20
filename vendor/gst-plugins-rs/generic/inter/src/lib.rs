// Copyright (C) 2023 Mathieu Duponchelle <mathieu@centricular.com>
//
// Take a look at the license at the top of the repository in the LICENSE file.
#![allow(unused_doc_comments)]

//! GStreamer elements for connecting pipelines in the same process

mod sink;
mod src;
mod streamproducer;
/**
 * plugin-rsinter:
 * @title: Rust inter elements
 * @short_description: A set of elements for transferring data between pipelines
 *
 * This plugin exposes two elements, `intersink` and `intersrc`, that can be
 * used to transfer data from one pipeline to multiple others in the same
 * process.
 *
 * The elements are implemented using the `StreamProducer` API from
 * gstreamer-utils.
 *
 * Since: plugins-rs-0.11.0
 */
use gst::glib;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    sink::register(plugin)?;
    src::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    rsinter,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
