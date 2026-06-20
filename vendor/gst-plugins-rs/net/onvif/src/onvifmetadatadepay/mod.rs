use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OnvifMetadataDepay(ObjectSubclass<imp::OnvifMetadataDepay>) @extends gst_rtp::RTPBaseDepayload, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtponvifmetadatadepay",
        gst::Rank::PRIMARY,
        OnvifMetadataDepay::static_type(),
    )
}
