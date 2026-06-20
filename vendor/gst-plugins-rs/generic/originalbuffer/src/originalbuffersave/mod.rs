// Copyright (C) 2024 Collabora Ltd
//   @author: Olivier Crête <olivier.crete@collabora.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-originalbuffersave
 *
 * GStreamer elements to store the original buffer and restore it later
 *
 * In many analysis scenario (for example machine learning), it is desirable to
 * use a pre-processed buffer, for example by lowering the resolution, but we may
 * want to take the output of this analysis, and apply it to the original buffer.
 *
 * These elements do just this, the typical usage would be a pipeline like:
 *
 * `... ! originalbuffersave ! videoconvertscale ! video/x-raw, width=100, height=100 ! analysiselement ! originalbufferrestore ! ...`
 *
 * The originalbufferrestore element will "restore" the buffer that was entered to the "save" element, but will keep any metadata that was added later.
 */
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OriginalBufferSave(ObjectSubclass<imp::OriginalBufferSave>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "originalbuffersave",
        gst::Rank::NONE,
        OriginalBufferSave::static_type(),
    )
}
