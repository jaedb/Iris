use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct AsyncMutexSink(ObjectSubclass<imp::AsyncMutexSink>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        super::ASYNC_MUTEX_ELEMENT_NAME,
        gst::Rank::NONE,
        AsyncMutexSink::static_type(),
    )
}
