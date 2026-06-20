// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;
pub mod remote_types;
mod transcribe;
pub mod translate;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "awstranscribe",
        gst::DebugColorFlags::empty(),
        Some("AWS Transcribe element"),
    )
});

use aws_sdk_transcribestreaming::types::{PartialResultsStability, VocabularyFilterMethod};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsTranscriberResultStability")]
#[non_exhaustive]
pub enum AwsTranscriberResultStability {
    #[enum_value(name = "High: stabilize results as fast as possible", nick = "high")]
    High = 0,
    #[enum_value(
        name = "Medium: balance between stability and accuracy",
        nick = "medium"
    )]
    Medium = 1,
    #[enum_value(
        name = "Low: relatively less stable partial transcription results with higher accuracy",
        nick = "low"
    )]
    Low = 2,
}

impl From<AwsTranscriberResultStability> for PartialResultsStability {
    fn from(val: AwsTranscriberResultStability) -> Self {
        use AwsTranscriberResultStability::*;
        match val {
            High => PartialResultsStability::High,
            Medium => PartialResultsStability::Medium,
            Low => PartialResultsStability::Low,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsTranscriberVocabularyFilterMethod")]
#[non_exhaustive]
pub enum AwsTranscriberVocabularyFilterMethod {
    #[enum_value(name = "Mask: replace words with ***", nick = "mask")]
    Mask = 0,
    #[enum_value(name = "Remove: delete words", nick = "remove")]
    Remove = 1,
    #[enum_value(name = "Tag: flag words without changing them", nick = "tag")]
    Tag = 2,
}

impl From<AwsTranscriberVocabularyFilterMethod> for VocabularyFilterMethod {
    fn from(val: AwsTranscriberVocabularyFilterMethod) -> Self {
        use AwsTranscriberVocabularyFilterMethod::*;
        match val {
            Mask => VocabularyFilterMethod::Mask,
            Remove => VocabularyFilterMethod::Remove,
            Tag => VocabularyFilterMethod::Tag,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsTranscriberTranslationTokenizationMethod")]
#[non_exhaustive]
pub enum TranslationTokenizationMethod {
    #[default]
    #[enum_value(name = "None: don't tokenize translations", nick = "none")]
    None = 0,
    #[enum_value(
        name = "Span based: insert spans in the transript text and use the resulting spans in the translations to reproduce speech pacing.",
        nick = "span-based"
    )]
    SpanBased = 1,
}

glib::wrapper! {
    pub struct Transcriber(ObjectSubclass<imp::Transcriber>) @extends gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct TranslateSrcPad(ObjectSubclass<imp::TranslateSrcPad>) @extends gst::Pad, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        AwsTranscriberResultStability::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        AwsTranscriberVocabularyFilterMethod::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        TranslationTokenizationMethod::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        TranslateSrcPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "awstranscriber",
        gst::Rank::NONE,
        Transcriber::static_type(),
    )
}
