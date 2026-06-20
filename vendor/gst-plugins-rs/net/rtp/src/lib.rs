//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
// Copyright (C) 2022-24 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(unused_doc_comments)]

/**
 * plugin-rsrtp:
 *
 * Since: plugins-rs-0.9.0
 */
#[macro_use]
extern crate log;

use gst::glib;

#[macro_use]
mod utils;

mod gcc;
mod rtpbin2;

mod audio_discont;
mod baseaudiopay;
mod basedepay;
mod basepay;

mod ac3;
mod amr;
mod av1;
mod jpeg;
mod klv;
mod linear_audio;
mod mp2t;
mod mp4a;
mod mp4g;
mod mparobust;
mod opus;
mod pcmau;
mod smpte291;
mod vp8;
mod vp9;

#[cfg(test)]
mod tests;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gcc::register(plugin)?;
    rtpbin2::register(plugin)?;

    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        crate::basepay::RtpBasePay2::static_type() // make base classes available in docs
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        crate::basedepay::RtpBaseDepay2::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        crate::baseaudiopay::RtpBaseAudioPay2::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    ac3::depay::register(plugin)?;
    ac3::pay::register(plugin)?;

    amr::depay::register(plugin)?;
    amr::pay::register(plugin)?;

    av1::depay::register(plugin)?;
    av1::pay::register(plugin)?;

    jpeg::depay::register(plugin)?;
    jpeg::pay::register(plugin)?;

    klv::depay::register(plugin)?;
    klv::pay::register(plugin)?;

    linear_audio::depay::register(plugin)?;
    linear_audio::pay::register(plugin)?;

    mp2t::depay::register(plugin)?;
    mp2t::pay::register(plugin)?;

    mp4a::depay::register(plugin)?;
    mp4a::pay::register(plugin)?;

    mp4g::depay::register(plugin)?;
    mp4g::pay::register(plugin)?;

    mparobust::depay::register(plugin)?;

    opus::depay::register(plugin)?;
    opus::pay::register(plugin)?;

    pcmau::depay::register(plugin)?;
    pcmau::pay::register(plugin)?;

    smpte291::depay::register(plugin)?;
    smpte291::pay::register(plugin)?;

    vp8::depay::register(plugin)?;
    vp8::pay::register(plugin)?;

    vp9::depay::register(plugin)?;
    vp9::pay::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    rsrtp,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);

#[cfg(test)]
pub(crate) fn test_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        plugin_register_static().expect("rtp plugin test");
    });
}
