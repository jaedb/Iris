// SPDX-License-Identifier: MPL-2.0

use gst::glib;

mod imp;

glib::wrapper! {
    pub struct InterSrc(ObjectSubclass<imp::InterSrc>) @extends gst::Element, gst::Object;
}
