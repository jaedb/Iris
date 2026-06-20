// GStreamer RTP L8 / L16 / L24 linear raw audio depayloader
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
    pub struct RtpLinearAudioDepay(ObjectSubclass<imp::RtpLinearAudioDepay>)
        @extends crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL8Depay(ObjectSubclass<imp::RtpL8Depay>)
        @extends RtpLinearAudioDepay, crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL16Depay(ObjectSubclass<imp::RtpL16Depay>)
        @extends RtpLinearAudioDepay, crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub(crate) struct RtpL24Depay(ObjectSubclass<imp::RtpL24Depay>)
        @extends RtpLinearAudioDepay, crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        // Make internal base class available in docs
        crate::linear_audio::depay::RtpLinearAudioDepay::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpL8depay2",
        gst::Rank::MARGINAL,
        RtpL8Depay::static_type(),
    )?;

    gst::Element::register(
        Some(plugin),
        "rtpL16depay2",
        gst::Rank::MARGINAL,
        RtpL16Depay::static_type(),
    )?;

    gst::Element::register(
        Some(plugin),
        "rtpL24depay2",
        gst::Rank::MARGINAL,
        RtpL24Depay::static_type(),
    )?;

    Ok(())
}
