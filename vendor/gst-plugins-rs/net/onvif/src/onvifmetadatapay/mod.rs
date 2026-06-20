use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OnvifMetadataPay(ObjectSubclass<imp::OnvifMetadataPay>) @extends gst_rtp::RTPBasePayload, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtponvifmetadatapay",
        gst::Rank::PRIMARY,
        OnvifMetadataPay::static_type(),
    )
}
