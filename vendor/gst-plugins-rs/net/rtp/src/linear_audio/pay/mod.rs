// GStreamer RTP L8 / L16 / L24 linear raw audio payloader
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
    pub struct RtpLinearAudioPay(ObjectSubclass<imp::RtpLinearAudioPay>)
        @extends crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL8Pay(ObjectSubclass<imp::RtpL8Pay>)
        @extends RtpLinearAudioPay, crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL16Pay(ObjectSubclass<imp::RtpL16Pay>)
        @extends RtpLinearAudioPay, crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL24Pay(ObjectSubclass<imp::RtpL24Pay>)
        @extends RtpLinearAudioPay, crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        // Make internal base class available in docs
        crate::linear_audio::pay::RtpLinearAudioPay::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpL8pay2",
        gst::Rank::MARGINAL,
        RtpL8Pay::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "rtpL16pay2",
        gst::Rank::MARGINAL,
        RtpL16Pay::static_type(),
    )?;

    gst::Element::register(
        Some(plugin),
        "rtpL24pay2",
        gst::Rank::MARGINAL,
        RtpL24Pay::static_type(),
    )?;
    Ok(())
}
