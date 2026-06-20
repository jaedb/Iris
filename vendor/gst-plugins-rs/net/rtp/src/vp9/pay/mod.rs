//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
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
    pub struct RtpVp9Pay(ObjectSubclass<imp::RtpVp9Pay>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        PictureIdMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpvp9pay2",
        gst::Rank::MARGINAL,
        RtpVp9Pay::static_type(),
    )
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, glib::Enum, Default)]
#[enum_type(name = "GstRtpVp9Pay2PictureIdMode")]
#[repr(i32)]
pub enum PictureIdMode {
    #[default]
    #[enum_value(name = "No Picture ID", nick = "none")]
    None,
    #[enum_value(name = "7-bit PictureID", nick = "7-bit")]
    SevenBit,
    #[enum_value(name = "15-bit Picture ID", nick = "15-bit")]
    FifteenBit,
}
