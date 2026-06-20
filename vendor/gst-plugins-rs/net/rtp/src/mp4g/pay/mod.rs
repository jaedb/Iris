// GStreamer RTP MPEG-4 Generic Payloader
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

pub mod imp;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(i32)]
#[enum_type(name = "GstRtpMpeg4GenericPayAggregateMode")]
#[non_exhaustive]
pub(crate) enum RtpMpeg4GenericPayAggregateMode {
    #[enum_value(
        name = "Automatic: zero-latency if upstream is live, otherwise aggregate elementary streams until packet is full.",
        nick = "auto"
    )]
    Auto = -1,

    #[enum_value(
        name = "Zero Latency: always send out elementary streams right away, do not wait for more elementary streams to fill a packet.",
        nick = "zero-latency"
    )]
    ZeroLatency = 0,

    #[enum_value(
        name = "Aggregate: collect elementary streams until we have a full packet or the max-ptime limit is hit (if set).",
        nick = "aggregate"
    )]
    Aggregate = 1,
}

glib::wrapper! {
    pub struct RtpMpeg4GenericPay(ObjectSubclass<imp::RtpMpeg4GenericPay>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        RtpMpeg4GenericPayAggregateMode::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpmp4gpay2",
        gst::Rank::MARGINAL,
        RtpMpeg4GenericPay::static_type(),
    )
}
