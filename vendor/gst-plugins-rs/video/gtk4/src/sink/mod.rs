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

/**
 * SECTION:element-gtk4paintablesink
 *
 * GTK 4 provides `gtk::Video` & `gtk::Picture` for rendering media such as videos. As the default
 * `gtk::Video` widget doesn't offer the possibility to use a custom `gst::Pipeline`. The plugin
 * provides a `gst_video::VideoSink` along with a `gdk::Paintable` that's capable of rendering the
 * sink's frames.
 *
 * The sink can generate GL Textures if the system is capable of it, but it needs to be compiled
 * with either `wayland`, `x11glx` or `x11egl` cargo features. On Windows and macOS this is enabled
 * by default.
 *
 * Additionally, the sink can render DMABufs directly on Linux if GTK 4.14 or newer is used. For
 * this the `dmabuf` feature needs to be enabled.
 *
 * Depending on the GTK version that is used and should be supported as minimum, new features or
 * more efficient processing can be opted in with the `gtk_v4_10`, `gtk_v4_12` and `gtk_v4_14`
 * features. The minimum GTK version required by the sink is GTK 4.4 on Linux without GL support,
 * and 4.6 on Windows and macOS, and on Linux with GL support.
 *
 * The sink will provides a simple test window when launched via `gst-launch-1.0` or `gst-play-1.0`
 * or if the environment variable `GST_GTK4_WINDOW=1` is set. Setting `GST_GTK4_WINDOW_FULLSCREEN=1`
 * will make the window launch in fullscreen mode.
 *
 * {{ videos/gtk4/examples/gtksink.rs }}
 */
use gtk::glib;
use gtk::glib::prelude::*;

pub(super) mod frame;
pub(super) mod imp;
pub(super) mod paintable;
pub(super) mod render_widget;

enum SinkEvent {
    FrameChanged,
}

glib::wrapper! {
    pub struct PaintableSink(ObjectSubclass<imp::PaintableSink>)
        @extends gst_video::VideoSink, gst_base::BaseSink, gst::Element, gst::Object,
        @implements gst::ChildProxy;
}

impl PaintableSink {
    pub fn new(name: Option<&str>) -> Self {
        gst::Object::builder().name_if_some(name).build().unwrap()
    }
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "gtk4paintablesink",
        gst::Rank::NONE,
        PaintableSink::static_type(),
    )
}
