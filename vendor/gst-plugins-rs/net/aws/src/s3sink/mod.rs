// Copyright (C) 2019 Amazon.com, Inc. or its affiliates <mkolny@amazon.com>
// Copyright (C) 2023 Asymptotic Inc
//      Author: Arun Raghavan <arun@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod multipartsink;
mod putobjectsink;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstS3PutObjectSinkNextFile")]
pub(crate) enum NextFile {
    #[enum_value(name = "New file for each buffer", nick = "next-buffer")]
    Buffer,
    #[enum_value(name = "New file after each discontinuity", nick = "next-discont")]
    Discont,
    #[enum_value(name = "New file at each key frame", nick = "next-key-frame")]
    KeyFrame,
    #[enum_value(
        name = "New file after a force key unit event",
        nick = "next-key-unit-event"
    )]
    KeyUnitEvent,
    #[enum_value(
        name = "New file when the configured maximum file size would be exceeded with the next buffer or buffer list",
        nick = "next-max-size"
    )]
    MaxSize,
    #[enum_value(
        name = "New file when the configured maximum duration would be exceeded with the next buffer or buffer list",
        nick = "next-max-duration"
    )]
    MaxDuration,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstS3SinkOnError")]
pub(crate) enum OnError {
    #[enum_value(name = "Abort: Abort multipart upload on error.", nick = "abort")]
    Abort,
    #[enum_value(
        name = "Complete: Complete multipart upload on error.",
        nick = "complete"
    )]
    Complete,
    #[enum_value(name = "DoNothing: Do nothing on error.", nick = "nothing")]
    DoNothing,
}

glib::wrapper! {
    pub struct S3Sink(ObjectSubclass<multipartsink::S3Sink>) @extends gst_base::BaseSink, gst::Element, gst::Object, @implements gst::URIHandler;
}

glib::wrapper! {
    pub struct S3PutObjectSink(ObjectSubclass<putobjectsink::S3PutObjectSink>) @extends gst_base::BaseSink, gst::Element, gst::Object, @implements gst::URIHandler;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "awss3sink",
        gst::Rank::PRIMARY,
        S3Sink::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "awss3putobjectsink",
        // This element should not be autoplugged as it is only useful for specific use cases
        gst::Rank::NONE,
        S3PutObjectSink::static_type(),
    )
}
