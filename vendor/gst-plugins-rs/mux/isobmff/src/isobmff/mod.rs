// Copyright (C) 2021-2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::sync::LazyLock;

use gst::glib;
use gst::prelude::*;
use gst::tags;

mod ac3;
mod aux_info;
mod boxes;
mod eac3;
mod flac;
mod fmp4mux;
mod mp4mux;
mod precision_timestamps;
mod transform_matrix;
mod uncompressed;

pub(crate) static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "isobmffmux",
        gst::DebugColorFlags::empty(),
        Some("ISO Base Media File Format Mux Element"),
    )
});

glib::wrapper! {
    pub(crate) struct FMP4MuxPad(ObjectSubclass<crate::isobmff::fmp4mux::imp::FMP4MuxPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

glib::wrapper! {
    pub(crate) struct MP4MuxPad(ObjectSubclass<crate::isobmff::mp4mux::imp::MP4MuxPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

glib::wrapper! {
    pub(crate) struct FMP4Mux(ObjectSubclass<crate::isobmff::fmp4mux::imp::FMP4Mux>) @extends gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct MP4Mux(ObjectSubclass<crate::isobmff::mp4mux::imp::MP4Mux>) @extends gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct CMAFMux(ObjectSubclass<crate::isobmff::fmp4mux::imp::CMAFMux>) @extends FMP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct DASHMP4Mux(ObjectSubclass<crate::isobmff::fmp4mux::imp::DASHMP4Mux>) @extends FMP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct ISOFMP4Mux(ObjectSubclass<crate::isobmff::fmp4mux::imp::ISOFMP4Mux>) @extends FMP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct ISOMP4Mux(ObjectSubclass<crate::isobmff::mp4mux::imp::ISOMP4Mux>) @extends MP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct ONVIFFMP4Mux(ObjectSubclass<crate::isobmff::fmp4mux::imp::ONVIFFMP4Mux>) @extends FMP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub(crate) struct ONVIFMP4Mux(ObjectSubclass<crate::isobmff::mp4mux::imp::ONVIFMP4Mux>) @extends MP4Mux, gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    if !gst::meta::CustomMeta::is_registered("FMP4KeyframeMeta") {
        gst::meta::CustomMeta::register("FMP4KeyframeMeta", &[]);
    }

    #[cfg(feature = "doc")]
    {
        FMP4Mux::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        FMP4MuxPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        HeaderUpdateMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        WriteEdtsMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        ChunkMode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        MP4Mux::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        MP4MuxPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "isofmp4mux",
        gst::Rank::PRIMARY,
        ISOFMP4Mux::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "cmafmux",
        gst::Rank::PRIMARY,
        CMAFMux::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "dashmp4mux",
        gst::Rank::PRIMARY,
        DASHMP4Mux::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "onviffmp4mux",
        gst::Rank::PRIMARY,
        ONVIFFMP4Mux::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "isomp4mux",
        gst::Rank::MARGINAL,
        ISOMP4Mux::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "onvifmp4mux",
        gst::Rank::MARGINAL,
        ONVIFMP4Mux::static_type(),
    )?;

    tags::register::<PrecisionClockTypeTag>();
    tags::register::<PrecisionClockTimeUncertaintyNanosecondsTag>();
    Ok(())
}

pub enum PrecisionClockTimeUncertaintyNanosecondsTag {}

pub enum PrecisionClockTypeTag {}

#[derive(Debug, Copy, Clone)]
pub(crate) enum DeltaFrames {
    /// Only single completely decodable frames
    IntraOnly,
    /// Frames may depend on past frames
    PredictiveOnly,
    /// Frames may depend on past or future frames
    Bidirectional,
}

impl DeltaFrames {
    /// Whether dts is required to order buffers differently from presentation order
    pub(crate) fn requires_dts(&self) -> bool {
        matches!(self, Self::Bidirectional)
    }
    /// Whether this coding structure does not allow delta flags on buffers
    pub(crate) fn intra_only(&self) -> bool {
        matches!(self, Self::IntraOnly)
    }
}

/**
 * GstFMP4MuxHeaderUpdateMode:
 *
 * How and when updating of the header (`moov`, initialization segment) is allowed.
 */
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[repr(i32)]
#[enum_type(name = "GstFMP4MuxHeaderUpdateMode")]
pub(crate) enum HeaderUpdateMode {
    /**
     * GstFMP4MuxHeaderUpdateMode:none:
     *
     * Don't allow and do any header updates at all.
     *
     * Caps changes are not allowed in this mode.
     */
    None,
    /**
     * GstFMP4MuxHeaderUpdateMode:rewrite:
     *
     * Try rewriting the initial header with the overall duration at the very end.
     *
     * Caps changes are not allowed in this mode.
     */
    Rewrite,
    /**
     * GstFMP4MuxHeaderUpdateMode:update:
     *
     * Send an updated version of the initial header with the overall duration at
     * the very end.
     *
     * Caps changes are not allowed in this mode.
     */
    Update,
    /**
     * GstFMP4MuxHeaderUpdateMode:caps:
     *
     * Send an updated header whenever caps or tag changes are pending that affect the initial
     * header. The updated header does not have the duration set and will always be followed by a
     * new fragment.
     *
     * Since: plugins-rs-0.14.0
     */
    Caps,
}

/// Synchronisation capability of clock
///
/// This is used in the TAIClockInfoBox, see ISO/IEC 23001-17:2024 Amd 1.
#[repr(u8)]
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TaicClockType {
    /// Clock type is unknown
    Unknown = 0u8,
    /// Clock does not synchronise to an atomic clock time source
    CannotSync = 1u8,
    // Clock can synchronise to an atomic clock time source
    CanSync = 2u8,
    // Reserved - DO NOT USE
    Reserved = 3u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum, Default)]
#[enum_type(name = "GstFMP4MuxWriteEdtsMode")]
pub(crate) enum WriteEdtsMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TaiClockInfo {
    // Set with the PrecisionClockTimeUncertaintyNanoseconds tag
    // Defaults to unknown
    time_uncertainty: u64,
    // Cannot currently be set, defaults to microsecond
    clock_resolution: u32,
    // Cannot currently be set, defaults to unknown
    clock_drift_rate: i32,
    // Set with the PrecisionClockType tag
    // Defaults to unknown
    clock_type: TaicClockType,
}

// Standard values for taic box (ISO/IEC 23001-17 Amd 1)
pub(crate) const TAIC_TIME_UNCERTAINTY_UNKNOWN: u64 = 0xFFFF_FFFF_FFFF_FFFF;
pub(crate) const TAIC_CLOCK_DRIFT_RATE_UNKNOWN: i32 = 0x7FFF_FFFF;

// Data for auxiliary information, as used for per-sample timestamps and for protection schemes
#[derive(Clone, Debug, Default)]
pub(crate) struct AuxiliaryInformationEntry {
    pub(crate) entry_offset: u64,
    pub(crate) entry_len: u8,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AuxiliaryInformation {
    pub(crate) aux_info_type: Option<[u8; 4]>,
    pub(crate) aux_info_type_parameter: u32,
    pub(crate) entries: Vec<AuxiliaryInformationEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct ElstInfo {
    pub(crate) start: Option<gst::Signed<gst::ClockTime>>,
    pub(crate) duration: Option<gst::ClockTime>,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Variant {
    CMAF,
    DASH,
    FragmentedISO,
    FragmentedONVIF,
    ISO,
    ONVIF,
}

impl Variant {
    pub(crate) fn is_single_stream(self) -> bool {
        match self {
            Variant::FragmentedISO | Variant::FragmentedONVIF => false,
            Variant::CMAF | Variant::DASH => true,
            Variant::ISO | Variant::ONVIF => todo!(),
        }
    }

    pub(crate) fn is_fragmented(self) -> bool {
        match self {
            Variant::FragmentedISO | Variant::FragmentedONVIF | Variant::CMAF | Variant::DASH => {
                true
            }
            Variant::ISO | Variant::ONVIF => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PresentationConfiguration {
    pub(crate) variant: Variant,
    pub(crate) update: bool,

    /// Pre-defined movie timescale if not 0.
    pub(crate) movie_timescale: u32,

    /// First caps must be the video/reference stream. Must be in the order the tracks are going to
    /// be used later for the fragments too, for fragmented encoding.
    pub(crate) tracks: Vec<TrackConfiguration>,

    pub(crate) write_mehd: bool,
    pub(crate) duration: Option<gst::ClockTime>,

    /// Whether to write edts box
    pub(crate) write_edts: bool,
}

impl PresentationConfiguration {
    pub(crate) fn to_timescale(&self) -> u32 {
        if self.movie_timescale > 0 {
            self.movie_timescale
        } else {
            // Use the reference track timescale
            self.tracks[0].to_timescale()
        }
    }
}

#[derive(Debug)]
pub(crate) struct Sample {
    /// Sync point
    pub(crate) sync_point: bool,

    /// Sample duration
    pub(crate) duration: gst::ClockTime,

    /// Composition time offset
    ///
    /// This is `None` for streams that have no concept of DTS.
    pub(crate) composition_time_offset: Option<i64>,

    /// Size
    pub(crate) size: u32,
}

#[derive(Debug)]
pub(crate) struct Chunk {
    /// Chunk start offset
    pub(crate) offset: u64,

    /// Samples of this stream that are part of this chunk
    pub(crate) samples: Vec<Sample>,
}

#[derive(Debug)]
pub(crate) struct TrackConfiguration {
    /// Caps of this track
    pub(crate) caps: gst::Caps,

    /// Set if this is an intra-only tracck
    pub(crate) delta_frames: DeltaFrames,

    /// Pre-defined trak timescale if not 0.
    pub(crate) trak_timescale: u32,

    // More data to be included in the track header
    pub(crate) extra_header_data: Option<Vec<u8>>,

    // Codec-specific boxes to be included in the sample entry
    pub(crate) codec_specific_boxes: Vec<u8>,

    // Tags meta for audio language and video orientation
    pub(crate) language_code: Option<[u8; 3]>,
    pub(crate) orientation: &'static transform_matrix::TransformMatrix,
    pub(crate) avg_bitrate: Option<u32>,
    pub(crate) max_bitrate: Option<u32>,

    /// Edit list clipping information
    pub(crate) elst_infos: Vec<ElstInfo>,

    /// Earliest PTS
    ///
    /// If this is >0 then an edit list entry is needed to shift
    /// Only used for non-fragmented track
    pub(crate) earliest_pts: gst::ClockTime,

    /// End PTS
    /// Only used for non-fragmented track
    pub(crate) end_pts: gst::ClockTime,

    /// All the chunks stored for this stream
    /// Only used for non-fragmented track
    pub(crate) chunks: Vec<Chunk>,

    /// Whether this stream should be encoded as an ISO/IEC 23008-12 image sequence
    pub(crate) image_sequence: bool,

    /// TAI Clock information (ISO/IEC 23001-17 Amd 1)
    pub(crate) tai_clock_info: Option<TaiClockInfo>,

    /// Sample auxiliary information (ISO/IEC 14496-12:2022 Section 8.7.8 and 8.7.9)
    pub(crate) auxiliary_info: Vec<AuxiliaryInformation>,
}

pub(crate) fn caps_to_timescale(caps: &gst::CapsRef) -> u32 {
    let s = caps.structure(0).unwrap();

    if let Ok(fps) = s.get::<gst::Fraction>("framerate") {
        if fps.numer() == 0 {
            return 10_000;
        }

        if fps.denom() != 1 && fps.denom() != 1001 {
            if let Some(fps) = (fps.denom() as u64)
                .nseconds()
                .mul_div_round(1_000_000_000, fps.numer() as u64)
                .and_then(gst_video::guess_framerate)
            {
                return (fps.numer() as u32)
                    .mul_div_round(100, fps.denom() as u32)
                    .unwrap_or(10_000);
            }
        }

        if fps.denom() == 1001 {
            fps.numer() as u32
        } else {
            (fps.numer() as u32)
                .mul_div_round(100, fps.denom() as u32)
                .unwrap_or(10_000)
        }
    } else if let Ok(rate) = s.get::<i32>("rate") {
        rate as u32
    } else {
        10_000
    }
}

impl TrackConfiguration {
    pub(crate) fn to_timescale(&self) -> u32 {
        if self.trak_timescale > 0 {
            self.trak_timescale
        } else {
            caps_to_timescale(&self.caps)
        }
    }
}

#[derive(Debug)]
pub(crate) struct Buffer {
    /// Track index
    pub(crate) idx: usize,

    /// Actual buffer
    pub(crate) buffer: gst::Buffer,

    /// Timestamp (PTS or DTS)
    pub(crate) timestamp: gst::Signed<gst::ClockTime>,

    /// Sample duration
    pub(crate) duration: gst::ClockTime,

    /// Composition time offset
    pub(crate) composition_time_offset: Option<i64>,
}

#[derive(Debug)]
pub(crate) struct FragmentHeaderConfiguration<'a> {
    pub(crate) variant: Variant,

    /// Sequence number for this fragment.
    pub(crate) sequence_number: u32,

    /// If this is a full fragment or only a chunk.
    pub(crate) chunk: bool,

    pub(crate) streams: &'a [FragmentHeaderStream],
    pub(crate) buffers: &'a [Buffer],

    /// If this is for the last fragment.
    pub(crate) last_fragment: bool,
}

#[derive(Debug)]
pub(crate) struct FragmentHeaderStream {
    /// Caps of this stream
    pub(crate) caps: gst::Caps,

    /// Set if this is an intra-only stream
    pub(crate) delta_frames: DeltaFrames,

    /// Pre-defined trak timescale if not 0.
    pub(crate) trak_timescale: u32,

    /// Start time of this fragment
    ///
    /// `None` if this stream has no buffers in this fragment.
    pub(crate) start_time: Option<gst::ClockTime>,

    /// Start NTP time of this fragment
    ///
    /// This is in nanoseconds since epoch and is used for writing the prft box if present.
    ///
    /// Only the first track is ever used.
    pub(crate) start_ntp_time: Option<gst::ClockTime>,
}

impl FragmentHeaderStream {
    pub(crate) fn to_timescale(&self) -> u32 {
        if self.trak_timescale > 0 {
            self.trak_timescale
        } else {
            caps_to_timescale(&self.caps)
        }
    }
}

#[derive(Debug)]
pub(crate) struct FragmentOffset {
    pub(crate) time: gst::ClockTime,
    pub(crate) offset: u64,
}

#[derive(Debug)]
pub(crate) struct SplitNowEvent {
    pub chunk: bool,
}

impl From<&SplitNowEvent> for gst::Event {
    fn from(value: &SplitNowEvent) -> Self {
        gst::event::CustomDownstream::builder(
            gst::Structure::builder("FMP4MuxSplitNow")
                .field("chunk", value.chunk)
                .build(),
        )
        .build()
    }
}

impl From<SplitNowEvent> for gst::Event {
    fn from(value: SplitNowEvent) -> Self {
        gst::Event::from(&value)
    }
}

impl SplitNowEvent {
    pub(crate) fn try_parse(
        event: &gst::event::CustomDownstream,
    ) -> Option<Result<Self, glib::BoolError>> {
        let s = event.structure()?;
        if s.name() != "FMP4MuxSplitNow" {
            return None;
        }

        let chunk = match s
            .get_optional::<bool>("chunk")
            .map_err(|e| glib::bool_error!("Invalid SplitNow event with wrong chunk field: {e}"))
        {
            Ok(chunk) => chunk.unwrap_or(false),
            Err(err) => return Some(Err(err)),
        };

        Some(Ok(SplitNowEvent { chunk }))
    }
}

/**
 * GstFMP4MuxChunkMode:
 *
 * Whether chunking is done based on duration or key frames.
 */
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[repr(i32)]
#[enum_type(name = "GstFMP4MuxChunkMode")]
pub(crate) enum ChunkMode {
    /**
     * GstFMP4MuxChunkMode:none:
     *
     * Chunk mode not set.
     */
    None,
    /**
     * GstFMP4MuxChunkMode:duration:
     *
     * Chunk based on duration.
     */
    Duration,
    /**
     * GstFMP4MuxChunkMode:keyframe:
     *
     * Chunk on key frame boundaries.
     */
    Keyframe,
}
