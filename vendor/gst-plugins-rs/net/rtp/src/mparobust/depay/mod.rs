// GStreamer RTP MPEG Audio Robust elementary streams Depayloader
//
// Copyright (C) 2023 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

mod deint_buf;
pub(crate) use deint_buf::DeinterleavingAduBuffer;

mod frames;
pub(crate) use frames::{
    Adu, AduAccumulator, AduCompleteUnparsed, AduIdentifier, AduQueue, DataProvenance,
    InterleavingSeqnum, MaybeSingleAduOrList, Mp3Frame, SingleMp3FrameOrList,
};

pub mod imp;

use gst::{glib, prelude::*};

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmparobustdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG Audio Robust Depayloader"),
    )
});

glib::wrapper! {
    pub struct RtpMpegAudioRobustDepay(ObjectSubclass<imp::RtpMpegAudioRobustDepay>)
        @extends crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtpmparobustdepay2",
        gst::Rank::MARGINAL,
        RtpMpegAudioRobustDepay::static_type(),
    )
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum MpegAudioRobustDepayError {
    #[error("{provenance}: insufficient ADU len: {len}, expected at least {expected}")]
    InsufficientUnparsedAduSize {
        provenance: DataProvenance,
        len: usize,
        expected: usize,
    },

    #[error("ADU {id}: insufficient len: {len}, expected at least {expected}")]
    InsufficientAduSize {
        id: AduIdentifier,
        len: usize,
        expected: usize,
    },

    #[error("ADU {id}: error parsing MP3 header for ADU")]
    MP3HeaderParsingError { id: AduIdentifier },

    #[error("ADU {id}: unsupported MP3 frame with version {version} & channels {channels}")]
    UnsupportedMp3Frame {
        id: AduIdentifier,
        version: u8,
        channels: u8,
    },

    #[error("Initial ADU fragment {provenance}, but the accumulator is not empty")]
    NonEmptyAduAccumulator { provenance: DataProvenance },

    #[error("ADU continuation fragment {provenance}, but no initial fragment available")]
    NoInitialAduFragment { provenance: DataProvenance },

    #[error("ADU fragment {provenance}: len mismatch: {len}, expected {expected}")]
    AduFragmentSizeMismatch {
        provenance: DataProvenance,
        len: usize,
        expected: usize,
    },

    #[error("ADU fragment {provenance}: excessive len: {len}, remaining {remaining}")]
    ExcessiveAduFragmentLen {
        provenance: DataProvenance,
        len: usize,
        remaining: usize,
    },
}
