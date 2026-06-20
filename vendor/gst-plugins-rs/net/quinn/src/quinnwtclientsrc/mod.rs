// Copyright (C) 2024, Fluendo S.A.
//      Author: Andoni Morales Alastruey <amorales@fluendo.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnwtclientsrc:
 * @short-description: [WebTransport](https://www.w3.org/TR/webtransport/) client that receives
 * data over the network connecting to a WebTransport server
 *
 * ## Example receiver pipeline
 * ```bash
 * gst-launch-1.0 -v -e quinnwtclientsrc url="http://localhost:4443/" \
 * certificate-file="certificates/fullchain.pem" caps=audio/x-opus ! \
 * ! opusparse ! opusdec ! audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! \
 * audioconvert ! autoaudiosink
 * ```
 */
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct QuinnWebTransportClientSrc(ObjectSubclass<imp::QuinnWebTransportClientSrc>) @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "quinnwtclientsrc",
        gst::Rank::MARGINAL,
        QuinnWebTransportClientSrc::static_type(),
    )
}
