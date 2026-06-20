// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

// This enum may be used to control what type of output the progressbin should produce.
// It also serves the secondary purpose of illustrating how to add enum-type properties
// to a plugin written in rust.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstProgressBinOutput")]
pub enum ProgressBinOutput {
    #[enum_value(
        name = "Println: Outputs the progress using a println! macro.",
        nick = "println"
    )]
    Println = 0,
    #[enum_value(
        name = "Debug Category: Outputs the progress as info logs under the element's debug category.",
        nick = "debug-category"
    )]
    DebugCategory = 1,
}

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct ProgressBin(ObjectSubclass<imp::ProgressBin>) @extends gst::Bin, gst::Element, gst::Object;
}

// Registers the type for our element, and then registers in GStreamer under
// the name "rsprogressbin" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rsprogressbin",
        gst::Rank::NONE,
        ProgressBin::static_type(),
    )
}
