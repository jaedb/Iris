// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
mod imp;

glib::wrapper! {
    pub struct BandwidthEstimator(ObjectSubclass<imp::BandwidthEstimator>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        imp::Estimator::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpgccbwe",
        gst::Rank::NONE,
        BandwidthEstimator::static_type(),
    )
}
