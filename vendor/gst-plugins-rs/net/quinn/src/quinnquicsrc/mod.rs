// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnquicsrc:
 * @short-description: Receive data over the network via QUIC
 *
 * ## Example receiver pipeline
 * ```bash
 * gst-launch-1.0 -v -e quinnquicsrc caps=audio/x-opus server-name="quic.net" \
 * certificate-file="certificates/fullchain.pem" private-key-file="certificates/privkey.pem" \
 * address="127.0.0.1" port=6000 ! opusparse ! opusdec ! \
 * audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! \
 * audioconvert ! autoaudiosink
 * ```
 */
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct QuinnQuicSrc(ObjectSubclass<imp::QuinnQuicSrc>) @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "quinnquicsrc",
        gst::Rank::MARGINAL,
        QuinnQuicSrc::static_type(),
    )
}
