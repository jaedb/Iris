// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;

mod imp;

glib::wrapper! {
    pub struct DeviceProvider(ObjectSubclass<imp::DeviceProvider>) @extends gst::DeviceProvider, gst::Object;
}

glib::wrapper! {
    pub struct Device(ObjectSubclass<imp::Device>) @extends gst::Device, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::DeviceProvider::register(
        Some(plugin),
        "ndideviceprovider",
        gst::Rank::PRIMARY,
        DeviceProvider::static_type(),
    )
}
