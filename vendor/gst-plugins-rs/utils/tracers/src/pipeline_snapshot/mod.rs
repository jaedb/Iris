// Copyright (C) 2022 OneStream Live <guillaume.desmottes@onestream.live>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

mod imp;

glib::wrapper! {
    pub struct PipelineSnapshot(ObjectSubclass<imp::PipelineSnapshot>) @extends gst::Tracer, gst::Object;
}

impl PipelineSnapshot {
    fn snapshot(&self) {
        self.imp().snapshot()
    }
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Tracer::register(
        Some(plugin),
        "pipeline-snapshot",
        PipelineSnapshot::static_type(),
    )
}
