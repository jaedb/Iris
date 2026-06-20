// GStreamer RTP MPEG-4 Audio Depayloader
//
// Copyright (C) 2023 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct RtpMpeg4AudioDepay(ObjectSubclass<imp::RtpMpeg4AudioDepay>)
        @extends crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtpmp4adepay2",
        gst::Rank::MARGINAL,
        RtpMpeg4AudioDepay::static_type(),
    )
}
