// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct AudioTestSrc(ObjectSubclass<imp::AudioTestSrc>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ts-audiotestsrc",
        gst::Rank::NONE,
        AudioTestSrc::static_type(),
    )
}
