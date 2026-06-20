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

use crate::sink::frame::Frame;

use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

mod imp;

glib::wrapper! {
    pub struct Paintable(ObjectSubclass<imp::Paintable>)
        @implements gdk::Paintable;
}

impl Paintable {
    pub(crate) fn new(context: Option<gdk::GLContext>) -> Self {
        glib::Object::builder()
            .property("gl-context", context)
            .build()
    }
}

impl Paintable {
    pub(crate) fn handle_frame_changed(&self, sink: &crate::PaintableSink, frame: Frame) {
        self.imp().handle_frame_changed(sink, frame);
    }

    pub(crate) fn handle_flush_frames(&self) {
        self.imp().handle_flush_frames();
    }
}
