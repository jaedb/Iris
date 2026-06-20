//
// Copyright (C) 2024 Guillaume Desmottes <guillaume@desmottes.be>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gtk::glib;

mod imp;

/// Use a simple container widget to automatically pass the window size to gtk4paintablesink.
glib::wrapper! {
    pub struct RenderWidget(ObjectSubclass<imp::RenderWidget>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RenderWidget {
    pub fn new(element: &gst::Element) -> Self {
        glib::Object::builder().property("element", element).build()
    }
}
