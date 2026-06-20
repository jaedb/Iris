//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, prelude::*, subclass::prelude::*};

pub mod imp;

glib::wrapper! {
    pub struct RtpPcmauPay(ObjectSubclass<imp::RtpPcmauPay>)
        @extends crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub trait RtpPcmauPayImpl:
    crate::baseaudiopay::RtpBaseAudioPay2Impl + ObjectSubclass<Type: IsA<RtpPcmauPay>>
{
}

unsafe impl<T: RtpPcmauPayImpl> IsSubclassable<T> for RtpPcmauPay {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);
    }
}

glib::wrapper! {
    pub struct RtpPcmaPay(ObjectSubclass<imp::RtpPcmaPay>)
        @extends RtpPcmauPay, crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub struct RtpPcmuPay(ObjectSubclass<imp::RtpPcmuPay>)
        @extends RtpPcmauPay, crate::baseaudiopay::RtpBaseAudioPay2, crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        // Make internal base class available in docs
        crate::pcmau::pay::RtpPcmauPay::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtppcmapay2",
        gst::Rank::MARGINAL,
        RtpPcmaPay::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "rtppcmupay2",
        gst::Rank::MARGINAL,
        RtpPcmuPay::static_type(),
    )
}
