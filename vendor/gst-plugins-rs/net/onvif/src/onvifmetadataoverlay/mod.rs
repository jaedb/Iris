use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OnvifMetadataOverlay(ObjectSubclass<imp::OnvifMetadataOverlay>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "onvifmetadataoverlay",
        gst::Rank::PRIMARY,
        OnvifMetadataOverlay::static_type(),
    )
}
