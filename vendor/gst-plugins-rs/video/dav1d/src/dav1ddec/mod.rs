// Copyright (C) 2019 Philippe Normand <philn@igalia.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct Dav1dDec(ObjectSubclass<imp::Dav1dDec>) @extends gst_video::VideoDecoder, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    imp::InloopFilterType::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    let rank = if gst::version() >= (1, 21, 2, 1) {
        // AOM av1dec rank was demoted in 1.22 dev cycle
        // https://gitlab.freedesktop.org/gstreamer/gstreamer/-/merge_requests/3287
        gst::Rank::PRIMARY
    } else {
        gst::Rank::PRIMARY + 1
    };

    gst::Element::register(Some(plugin), "dav1ddec", rank, Dav1dDec::static_type())
}
