// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;

mod imp;

glib::wrapper! {
    pub struct NdiSrcDemux(ObjectSubclass<imp::NdiSrcDemux>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndisrcdemux",
        gst::Rank::PRIMARY,
        NdiSrcDemux::static_type(),
    )
}
