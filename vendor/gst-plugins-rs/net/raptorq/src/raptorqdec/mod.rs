// Copyright (C) 2022 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct RaptorqDec(ObjectSubclass<imp::RaptorqDec>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "raptorqdec",
        gst::Rank::MARGINAL,
        RaptorqDec::static_type(),
    )
}
