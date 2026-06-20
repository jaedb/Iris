// Copyright (C) 2024 Collabora Ltd
//   @author: Olivier Crête <olivier.crete@collabora.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-originalbufferrestore
 *
 * See originalbuffersave for details
 */
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct OriginalBufferRestore(ObjectSubclass<imp::OriginalBufferRestore>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "originalbufferrestore",
        gst::Rank::NONE,
        OriginalBufferRestore::static_type(),
    )
}
