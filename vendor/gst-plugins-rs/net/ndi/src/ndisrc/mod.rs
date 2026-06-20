// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;

mod imp;
mod receiver;

glib::wrapper! {
    pub struct NdiSrc(ObjectSubclass<imp::NdiSrc>) @extends gst_base::BaseSrc, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndisrc",
        gst::Rank::NONE,
        NdiSrc::static_type(),
    )
}
