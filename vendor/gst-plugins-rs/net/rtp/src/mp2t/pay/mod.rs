// GStreamer RTP MPEG-TS Payloader
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
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
    pub struct RtpMP2TPay(ObjectSubclass<imp::RtpMP2TPay>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtpmp2tpay2",
        gst::Rank::MARGINAL,
        RtpMP2TPay::static_type(),
    )
}
