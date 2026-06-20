// Copyright (C) 2022,  Asymptotic Inc.
//      Author: Taruntej Kanakamalla <taruntej@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-whipsink
 *
 * `whipsink` is an element that acts a WHIP Client to ingest RTP content to a media server
 *
 * ``` bash
 * gst-launch-1.0 videotestsrc ! videoconvert ! openh264enc ! rtph264pay ! \
 * 'application/x-rtp,media=video,encoding-name=H264,payload=97,clock-rate=90000' ! \
 * whip.sink_0 audiotestsrc ! audioconvert ! opusenc ! rtpopuspay ! \
 * 'application/x-rtp,media=audio,encoding-name=OPUS,payload=96,clock-rate=48000,encoding-params=(string)2' ! \
 * whip.sink_1 whipsink name=whip auth-token=$WHIP_TOKEN whip-endpoint=$WHIP_ENDPOINT
 * ```
 *
 * Note: whipsink will be deprecated in the future. It is replaced by whipclientsink,
 * which provides more features such as managing encoding and performing bandwidth adaptation
 *
 */
use gst::glib;
use gst::prelude::*;

pub mod imp;

glib::wrapper! {
    pub struct WhipSink(ObjectSubclass<imp::WhipSink>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "whipsink",
        gst::Rank::MARGINAL,
        WhipSink::static_type(),
    )
}
