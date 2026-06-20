// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, prelude::*};

mod imp;

glib::wrapper! {
    pub struct S302MParse(ObjectSubclass<imp::S302MParse>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "s302mparse",
        gst::Rank::PRIMARY + 1,
        S302MParse::static_type(),
    )
}
