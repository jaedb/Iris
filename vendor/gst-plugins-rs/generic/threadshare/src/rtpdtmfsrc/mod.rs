// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct RTPDTMFSrc(ObjectSubclass<imp::RTPDTMFSrc>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ts-rtpdtmfsrc",
        gst::Rank::NONE,
        RTPDTMFSrc::static_type(),
    )
}
