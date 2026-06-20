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
    pub struct RtpAmrPay(ObjectSubclass<imp::RtpAmrPay>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(i32)]
#[enum_type(name = "GstRtpAmrPayAggregateMode")]
#[non_exhaustive]
pub(crate) enum AggregateMode {
    #[enum_value(
        name = "Automatic: zero-latency if upstream is live, otherwise aggregate frames until packet is full.",
        nick = "auto"
    )]
    Auto = -1,

    #[enum_value(
        name = "Zero Latency: always send out frames right away, do not wait for more frames to fill a packet.",
        nick = "zero-latency"
    )]
    ZeroLatency = 0,

    #[enum_value(
        name = "Aggregate: collect audio frames until we have a full packet or the max-ptime limit is hit (if set).",
        nick = "aggregate"
    )]
    Aggregate = 1,
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        AggregateMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpamrpay2",
        gst::Rank::MARGINAL,
        RtpAmrPay::static_type(),
    )
}
