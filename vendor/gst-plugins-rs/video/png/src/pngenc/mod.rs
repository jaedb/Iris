// Copyright (C) 2020 Natanael Mojica <neithanmo@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRsPngCompressionLevel")]
pub(crate) enum CompressionLevel {
    #[enum_value(name = "Default: Use the default compression level.", nick = "default")]
    Default,
    #[enum_value(name = "Fastest: Use the fastest compression level.", nick = "fastest")]
    Fastest,
    #[enum_value(name = "Fast: A fast compression algorithm.", nick = "fast")]
    Fast,
    #[enum_value(
        name = "Balanced: Uses the algorithm with balanced results.",
        nick = "balanced"
    )]
    Balanced,
    #[enum_value(name = "High: Use the highest compression level.", nick = "high")]
    High,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRsPngFilter")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Filter {
    #[enum_value(
        name = "NoFilter: No filtering applied to the output.",
        nick = "nofilter"
    )]
    NoFilter,
    #[enum_value(name = "Sub: filter applied to each pixel.", nick = "sub")]
    Sub,
    #[enum_value(name = "Up: Up filter similar to Sub.", nick = "up")]
    Up,
    #[enum_value(
        name = "Avg: The Average filter uses the average of the two neighboring pixels.",
        nick = "avg"
    )]
    Avg,
    #[enum_value(
        name = "Paeth: The Paeth filter computes a simple linear function of the three neighboring pixels.",
        nick = "paeth"
    )]
    Paeth,
    #[enum_value(
        name = "Adaptive: Uses heuristics to select the best filter for every row.",
        nick = "Adaptive"
    )]
    Adaptive,
}

impl From<CompressionLevel> for png::Compression {
    #[allow(deprecated)]
    fn from(value: CompressionLevel) -> Self {
        match value {
            CompressionLevel::Default => png::Compression::default(),
            CompressionLevel::Fastest => png::Compression::Fastest,
            CompressionLevel::Fast => png::Compression::Fast,
            CompressionLevel::Balanced => png::Compression::Balanced,
            CompressionLevel::High => png::Compression::High,
        }
    }
}

impl From<Filter> for png::Filter {
    fn from(value: Filter) -> Self {
        match value {
            Filter::NoFilter => png::Filter::NoFilter,
            Filter::Sub => png::Filter::Sub,
            Filter::Up => png::Filter::Up,
            Filter::Avg => png::Filter::Avg,
            Filter::Paeth => png::Filter::Paeth,
            Filter::Adaptive => png::Filter::Adaptive,
        }
    }
}

glib::wrapper! {
    pub struct PngEncoder(ObjectSubclass<imp::PngEncoder>) @extends gst_video::VideoEncoder, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    CompressionLevel::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    #[cfg(feature = "doc")]
    Filter::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());

    gst::Element::register(
        Some(plugin),
        "rspngenc",
        gst::Rank::PRIMARY,
        PngEncoder::static_type(),
    )
}
