// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;
mod pad;

#[cfg(feature = "doc")]
pub use imp::Background;

#[cfg(feature = "doc")]
pub use pad::Operator;

glib::wrapper! {
    pub struct SkiaCompositor(ObjectSubclass<imp::SkiaCompositor>) @extends gst_video::VideoAggregator, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct SkiaCompositorPad(ObjectSubclass<pad::SkiaCompositorPad>) @extends gst_video::VideoAggregatorConvertPad, gst_video::VideoAggregatorPad, gst_base::AggregatorPad, gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "skiacompositor",
        gst::Rank::SECONDARY,
        SkiaCompositor::static_type(),
    )
}
