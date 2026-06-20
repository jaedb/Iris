use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct DirectSink(ObjectSubclass<imp::DirectSink>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        super::SYNC_MUTEX_ELEMENT_NAME,
        gst::Rank::NONE,
        DirectSink::static_type(),
    )
}
