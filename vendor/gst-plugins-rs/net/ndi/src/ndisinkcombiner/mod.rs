// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;

mod imp;

glib::wrapper! {
    pub struct NdiSinkCombiner(ObjectSubclass<imp::NdiSinkCombiner>) @extends gst_base::Aggregator, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndisinkcombiner",
        gst::Rank::NONE,
        NdiSinkCombiner::static_type(),
    )
}
