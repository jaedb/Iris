// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;

mod imp;

glib::wrapper! {
    pub struct CustomSource(ObjectSubclass<imp::CustomSource>) @extends gst::Bin, gst::Element, gst::Object;
}

impl CustomSource {
    pub fn new(source: &gst::Element) -> CustomSource {
        gst::Object::builder()
            .property("source", source)
            .build()
            .unwrap()
    }
}
