// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;
use gst::glib;

mod imp;

/**
 * SECTION:element-intersink
 *
 * #intersink is an element that can be used to produce data for
 * multiple #intersrc elements to consume.
 *
 * You can access the underlying appsink element through the static name
 * "appsink".
 *
 * #intersink should not reside in the same pipeline as the #intersrc
 * that consumes from it, here is an example of how to use those elements
 * in separate pipelines:
 *
 * {{ generic/inter/examples/basic.rs }}
 */

glib::wrapper! {
    pub struct InterSink(ObjectSubclass<imp::InterSink>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "intersink",
        gst::Rank::NONE,
        InterSink::static_type(),
    )
}
