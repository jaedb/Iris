// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;

mod imp;

glib::wrapper! {
    pub struct NdiSink(ObjectSubclass<imp::NdiSink>) @extends gst_base::BaseSink, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndisink",
        gst::Rank::NONE,
        NdiSink::static_type(),
    )
}
