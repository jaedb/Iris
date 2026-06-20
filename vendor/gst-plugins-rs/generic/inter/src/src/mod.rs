// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;
use gst::glib;

mod imp;

/**
 * SECTION:element-intersrc
 *
 * #intersrc is an element that can be used to consume data from an #intersink.
 *
 * You can access the underlying appsrc element through the static name
 * "appsrc".
 *
 * #intersrc should not reside in the same pipeline as the #intersink
 * that it consumes from, here is an example of how to use those elements
 * in separate pipelines:
 *
 * {{ generic/inter/examples/basic.rs }}
 */

glib::wrapper! {
    pub struct InterSrc(ObjectSubclass<imp::InterSrc>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "intersrc",
        gst::Rank::NONE,
        InterSrc::static_type(),
    )
}
