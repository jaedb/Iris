//
// Copyright (C) 2021 Bilal Elmoussaoui <bil.elmoussaoui@gmail.com>
// Copyright (C) 2021 Jordan Petridis <jordan@centricular.com>
// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-gtk4:
 *
 * Since: plugins-rs-0.8.0
 */
use gst::glib;

mod sink;
mod utils;
pub use sink::frame::Orientation;
pub use sink::paintable::Paintable;
// The widget needs to be public so it can be used by the example and element debug window but
// we don't want it be part of the official API for now.
#[doc(hidden)]
pub use sink::render_widget::RenderWidget;
pub use sink::PaintableSink;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        sink::paintable::Paintable::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        sink::frame::Orientation::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    #[cfg(not(feature = "gtk_v4_10"))]
    {
        if gtk::micro_version() >= 13 {
            gst::warning!(sink::imp::CAT, obj = plugin, "GTK 4.13 or newer detected but plugin not compiled with support for this version. Rendering of video frames with alpha will likely be wrong");
        }
    }

    sink::register(plugin)
}

gst::plugin_define!(
    gtk4,
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
