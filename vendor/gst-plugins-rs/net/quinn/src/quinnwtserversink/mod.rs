// Copyright (C) 2024, Fluendo, SA
//      Author: Ruben Gonzalez <rgonzalez@fluendo.com>
//      Author: Andoni Morales <amorales@fluendo.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * element-quinnwtserversink:
 * @short-description: [WebTransport](https://www.w3.org/TR/webtransport/) server that
 * sends data over the network.
 *
 * ## Example sender pipeline
 * ```bash
 * gst-launch-1.0 -v -e audiotestsrc num-buffers=512 ! \
 * audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! opusenc ! \
 * quinnwtserversink address="127.0.0.1" port=4443 \
 * certificate-file="certificates/fullchain.pem" \
 * private-key-file="certificates/privkey.pem"
 * ```
 */
use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct QuinnWebTransportSink(ObjectSubclass<imp::QuinnWebTransportSink>) @extends gst_base::BaseSink, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(not(feature = "doc"))]
    gst::Element::register(
        Some(plugin),
        "quinnwtserversink",
        gst::Rank::MARGINAL,
        QuinnWebTransportSink::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "quinnwtsink",
        gst::Rank::MARGINAL,
        QuinnWebTransportSink::static_type(),
    )
}
