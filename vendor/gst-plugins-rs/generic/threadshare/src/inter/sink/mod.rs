// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2025 François Laignel <francois@centricular.com>
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;

mod imp;

glib::wrapper! {
    pub struct InterSink(ObjectSubclass<imp::InterSink>) @extends gst::Element, gst::Object;
}
