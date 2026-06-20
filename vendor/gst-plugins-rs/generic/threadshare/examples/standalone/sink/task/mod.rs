use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct TaskSink(ObjectSubclass<imp::TaskSink>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        super::TASK_ELEMENT_NAME,
        gst::Rank::NONE,
        TaskSink::static_type(),
    )
}
