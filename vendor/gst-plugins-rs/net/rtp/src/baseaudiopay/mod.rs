//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, prelude::*, subclass::prelude::*};

use crate::basepay::RtpBasePay2Impl;

pub mod imp;

glib::wrapper! {
    pub struct RtpBaseAudioPay2(ObjectSubclass<imp::RtpBaseAudioPay2>)
        @extends crate::basepay::RtpBasePay2, gst::Element, gst::Object;
}

/// Trait containing extension methods for `RtpBaseAudioPay2`.
pub trait RtpBaseAudioPay2Ext: IsA<RtpBaseAudioPay2> + 'static {
    /// Sets the number of bytes per frame.
    ///
    /// Should always be called together with `RtpBasePay2Ext::set_src_caps()`.
    fn set_bpf(&self, bpf: usize) {
        self.upcast_ref::<RtpBaseAudioPay2>().imp().set_bpf(bpf)
    }
}

impl<O: IsA<RtpBaseAudioPay2>> RtpBaseAudioPay2Ext for O {}

/// Trait to implement in `RtpBaseAudioPay2` subclasses.
pub trait RtpBaseAudioPay2Impl:
    RtpBasePay2Impl + ObjectSubclass<Type: IsA<RtpBaseAudioPay2>>
{
}

unsafe impl<T: RtpBaseAudioPay2Impl> IsSubclassable<T> for RtpBaseAudioPay2 {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);
    }
}
