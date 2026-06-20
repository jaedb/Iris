// GStreamer RTP AC-3 Audio Payloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
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
#[enum_type(name = "GstRtpAc3PayAggregateMode")]
#[non_exhaustive]
pub(crate) enum RtpAc3PayAggregateMode {
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

glib::wrapper! {
    pub struct RtpAc3Pay(ObjectSubclass<imp::RtpAc3Pay>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        RtpAc3PayAggregateMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtpac3pay2",
        gst::Rank::MARGINAL,
        RtpAc3Pay::static_type(),
    )
}
