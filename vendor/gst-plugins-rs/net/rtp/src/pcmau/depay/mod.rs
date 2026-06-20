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
    pub struct RtpPcmauDepay(ObjectSubclass<imp::RtpPcmauDepay>)
        @extends crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub trait RtpPcmauDepayImpl:
    crate::basedepay::RtpBaseDepay2Impl + ObjectSubclass<Type: IsA<RtpPcmauDepay>>
{
}

unsafe impl<T: RtpPcmauDepayImpl> IsSubclassable<T> for RtpPcmauDepay {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);
    }
}

glib::wrapper! {
    pub struct RtpPcmaDepay(ObjectSubclass<imp::RtpPcmaDepay>)
        @extends RtpPcmauDepay, crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

glib::wrapper! {
    pub struct RtpPcmuDepay(ObjectSubclass<imp::RtpPcmuDepay>)
        @extends RtpPcmauDepay, crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        // Make internal base class available in docs
        crate::pcmau::depay::RtpPcmauDepay::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "rtppcmadepay2",
        gst::Rank::MARGINAL,
        RtpPcmaDepay::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "rtppcmudepay2",
        gst::Rank::MARGINAL,
        RtpPcmuDepay::static_type(),
    )
}
