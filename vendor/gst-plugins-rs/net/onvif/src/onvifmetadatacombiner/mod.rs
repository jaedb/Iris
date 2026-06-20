use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OnvifMetadataCombiner(ObjectSubclass<imp::OnvifMetadataCombiner>) @extends gst_base::Aggregator, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "onvifmetadatacombiner",
        gst::Rank::PRIMARY,
        OnvifMetadataCombiner::static_type(),
    )
}
