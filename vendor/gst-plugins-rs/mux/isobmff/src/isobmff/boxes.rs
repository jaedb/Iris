// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::av1::util::{av1_seq_level_idx, av1_tier};
use crate::isobmff::flac::parse_flac_stream_header;
use crate::isobmff::fmp4mux::boxes::write_mvex;
use crate::isobmff::uncompressed::write_uncompressed_sample_entries;
use crate::isobmff::{ac3, eac3, Variant, CAT};
use crate::isobmff::{
    transform_matrix::IDENTITY_MATRIX, PresentationConfiguration, TrackConfiguration,
};
use anyhow::{anyhow, bail, Context, Error};
use gst::prelude::MulDiv;
use std::str::FromStr;

pub(crate) const FULL_BOX_VERSION_0: u8 = 0;
pub(crate) const FULL_BOX_VERSION_1: u8 = 1;

pub(crate) const FULL_BOX_FLAGS_NONE: u32 = 0;

pub(crate) fn write_box<T, F: FnOnce(&mut Vec<u8>) -> Result<T, Error>>(
    vec: &mut Vec<u8>,
    fourcc: impl std::borrow::Borrow<[u8; 4]>,
    content_func: F,
) -> Result<T, Error> {
    // Write zero size ...
    let size_pos = vec.len();
    vec.extend([0u8; 4]);
    vec.extend(fourcc.borrow());

    let res = content_func(vec)?;

    // ... and update it here later.
    let size: u32 = vec
        .len()
        .checked_sub(size_pos)
        .expect("vector shrunk")
        .try_into()
        .context("too big box content")?;
    vec[size_pos..][..4].copy_from_slice(&size.to_be_bytes());

    Ok(res)
}

pub(crate) fn write_full_box<T, F: FnOnce(&mut Vec<u8>) -> Result<T, Error>>(
    vec: &mut Vec<u8>,
    fourcc: impl std::borrow::Borrow<[u8; 4]>,
    version: u8,
    flags: u32,
    content_func: F,
) -> Result<T, Error> {
    write_box(vec, fourcc, move |vec| {
        assert_eq!(flags >> 24, 0);
        vec.extend(((u32::from(version) << 24) | flags).to_be_bytes());
        content_func(vec)
    })
}

/// Creates `ftyp` box
pub(crate) fn create_ftyp(
    major_brand: [u8; 4],
    minor_version: u32,
    compatible_brands: Vec<[u8; 4]>,
) -> Result<gst::Buffer, Error> {
    let mut v = vec![];
    write_ftyp(&mut v, major_brand, minor_version, compatible_brands)?;

    Ok(gst::Buffer::from_mut_slice(v))
}

pub(crate) fn write_ftyp(
    v: &mut Vec<u8>,
    major_brand: [u8; 4],
    minor_version: u32,
    compatible_brands: Vec<[u8; 4]>,
) -> Result<(), Error> {
    write_box(v, b"ftyp", |v| {
        // major brand
        v.extend(major_brand);
        v.extend(minor_version.to_be_bytes());
        // compatible brands
        v.extend(compatible_brands.into_iter().flatten());

        Ok(())
    })?;
    Ok(())
}

/// Creates `mdat` box *header*.
pub(crate) fn create_mdat_header_non_frag(size: Option<u64>) -> Result<gst::Buffer, Error> {
    let mut v = vec![];

    if let Some(size) = size {
        if let Ok(size) = u32::try_from(size + 8) {
            v.extend(8u32.to_be_bytes());
            v.extend(b"free");
            v.extend(size.to_be_bytes());
            v.extend(b"mdat");
        } else {
            v.extend(1u32.to_be_bytes());
            v.extend(b"mdat");
            v.extend((size + 16).to_be_bytes());
        }
    } else {
        v.extend(8u32.to_be_bytes());
        v.extend(b"free");
        v.extend(0u32.to_be_bytes());
        v.extend(b"mdat");
    }

    Ok(gst::Buffer::from_mut_slice(v))
}

/// Creates `ftype` (if fragmented) and `moov` box
pub(crate) fn create_moov(
    cfg: PresentationConfiguration,
    minor_version: u32,
    major_brand: Option<[u8; 4]>,
    compatible_brands: Option<Vec<[u8; 4]>>,
) -> Result<gst::Buffer, Error> {
    let mut v = vec![];

    if cfg.variant.is_fragmented() {
        if let (Some(brand), Some(compatible_brands)) = (major_brand, compatible_brands) {
            write_ftyp(&mut v, brand, minor_version, compatible_brands)?;
        }
    }

    write_box(&mut v, b"moov", |v| write_moov(v, &cfg))?;

    if cfg.variant == Variant::ONVIF {
        write_onvif_metabox(cfg, &mut v)?;
    }

    Ok(gst::Buffer::from_mut_slice(v))
}

fn write_moov(v: &mut Vec<u8>, cfg: &PresentationConfiguration) -> Result<(), Error> {
    use gst::glib;

    let base = glib::DateTime::from_utc(1904, 1, 1, 0, 0, 0.0)?;
    let now = glib::DateTime::now_utc()?;
    let creation_time =
        u64::try_from(now.difference(&base).as_seconds()).expect("time before 1904");

    write_full_box(v, b"mvhd", FULL_BOX_VERSION_1, FULL_BOX_FLAGS_NONE, |v| {
        write_mvhd(v, cfg, creation_time)
    })?;
    for (idx, stream) in cfg.tracks.iter().enumerate() {
        write_box(v, b"trak", |v| {
            let mut references = Vec::new();

            // Reference the video track for ONVIF metadata tracks
            if (cfg.variant == Variant::FragmentedONVIF || cfg.variant == Variant::ONVIF)
                && stream.caps.structure(0).unwrap().name() == "application/x-onvif-metadata"
            {
                // Find the first video track
                for (idx, other_stream) in cfg.tracks.iter().enumerate() {
                    let s = other_stream.caps.structure(0).unwrap();

                    if matches!(
                        s.name().as_str(),
                        "video/x-h264" | "video/x-h265" | "image/jpeg" | "video/x-raw"
                    ) {
                        references.push(TrackReference {
                            reference_type: *b"cdsc",
                            track_ids: vec![idx as u32 + 1],
                        });
                        break;
                    }
                }
            }

            write_trak(v, cfg, idx, stream, creation_time, &references)
        })?;
    }
    if cfg.variant.is_fragmented() {
        write_box(v, b"mvex", |v| write_mvex(v, cfg))?;
    }

    Ok(())
}

const TKHD_FLAGS_TRACK_ENABLED: u32 = 0x1;
const TKHD_FLAGS_TRACK_IN_MOVIE: u32 = 0x2;
const TKHD_FLAGS_TRACK_IN_PREVIEW: u32 = 0x4;

fn write_trak(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    idx: usize,
    stream: &TrackConfiguration,
    creation_time: u64,
    references: &[TrackReference],
) -> Result<(), Error> {
    write_full_box(
        v,
        b"tkhd",
        FULL_BOX_VERSION_1,
        TKHD_FLAGS_TRACK_ENABLED | TKHD_FLAGS_TRACK_IN_MOVIE | TKHD_FLAGS_TRACK_IN_PREVIEW,
        |v| write_tkhd(v, cfg, idx, stream, creation_time),
    )?;

    // TODO: write edts optionally for negative DTS instead of offsetting the DTS
    write_box(v, b"mdia", |v| write_mdia(v, cfg, stream, creation_time))?;
    // TODO: see if we can handle this better
    if cfg.variant.is_fragmented() {
        if !stream.elst_infos.is_empty() && cfg.write_edts {
            if let Err(e) = write_edts(v, cfg, stream) {
                gst::warning!(CAT, "Failed to write edts: {e}");
            }
        }
    } else {
        write_box(v, b"edts", |v| write_edts(v, cfg, stream))?;
    }

    if !references.is_empty() {
        write_box(v, b"tref", |v| write_tref(v, references))?;
    }

    Ok(())
}

fn write_mdia(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    stream: &TrackConfiguration,
    creation_time: u64,
) -> Result<(), Error> {
    write_full_box(v, b"mdhd", FULL_BOX_VERSION_1, FULL_BOX_FLAGS_NONE, |v| {
        write_mdhd(v, cfg, stream, creation_time)
    })?;

    write_full_box(v, b"hdlr", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_hdlr_for_stream(v, stream)
    })?;

    // TODO: write elng if needed

    write_box(v, b"minf", |v| write_minf(v, cfg, stream))?;

    Ok(())
}

fn write_minf(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    stream: &TrackConfiguration,
) -> Result<(), Error> {
    let s = stream.caps.structure(0).unwrap();

    match s.name().as_str() {
        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9" | "video/x-av1"
        | "image/jpeg" | "video/x-raw" => {
            // Flags are always 1 for unspecified reasons
            write_full_box(v, b"vmhd", FULL_BOX_VERSION_0, 1, write_vmhd)?
        }
        "audio/mpeg" | "audio/x-opus" | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw"
        | "audio/x-adpcm" | "audio/x-ac3" | "audio/x-eac3" => write_full_box(
            v,
            b"smhd",
            FULL_BOX_VERSION_0,
            FULL_BOX_FLAGS_NONE,
            write_smhd,
        )?,
        "application/x-onvif-metadata" => {
            write_full_box(v, b"nmhd", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |_v| {
                Ok(())
            })?
        }
        _ => unreachable!(),
    }

    write_box(v, b"dinf", write_dinf)?;

    write_box(v, b"stbl", |v| {
        write_stbl(v, stream, cfg.variant.is_fragmented())
    })?;

    Ok(())
}

pub(crate) fn write_stbl(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    is_fragmented: bool,
) -> Result<(), Error> {
    write_full_box(v, b"stsd", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_stsd(v, stream)
    })?;

    write_full_box(v, b"stts", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_stts(v, stream, is_fragmented)
    })?;

    // If there are any composition time offsets we need to write the ctts box. If any are negative
    // we need to write version 1 of the box, otherwise version 0 is sufficient.
    let mut need_ctts = None;
    if stream.delta_frames.requires_dts() {
        for composition_time_offset in stream.chunks.iter().flat_map(|c| {
            c.samples.iter().map(|b| {
                b.composition_time_offset
                    .expect("not all samples have a composition time offset")
            })
        }) {
            if composition_time_offset < 0 {
                need_ctts = Some(1);
                break;
            } else if composition_time_offset != 0 {
                need_ctts = Some(0);
            }
        }
    }
    if let Some(need_ctts) = need_ctts {
        let version = if need_ctts == 0 {
            FULL_BOX_VERSION_0
        } else {
            FULL_BOX_VERSION_1
        };

        write_full_box(v, b"ctts", version, FULL_BOX_FLAGS_NONE, |v| {
            write_ctts(v, stream, version)
        })?;

        write_full_box(v, b"cslg", FULL_BOX_VERSION_1, FULL_BOX_FLAGS_NONE, |v| {
            write_cslg(v, stream)
        })?;
    }

    // If any sample is not a sync point, write the stss box
    if !stream.delta_frames.intra_only() {
        let should_write_stss = is_fragmented
            || stream
                .chunks
                .iter()
                .flat_map(|c| c.samples.iter())
                .any(|b| !b.sync_point);

        if should_write_stss {
            write_full_box(v, b"stss", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
                write_stss(v, stream, is_fragmented)
            })?;
        }
    }

    write_full_box(v, b"stsz", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_stsz(v, stream, is_fragmented)
    })?;

    write_full_box(v, b"stsc", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_stsc(v, stream, is_fragmented)
    })?;

    if !is_fragmented && stream.chunks.last().unwrap().offset > u32::MAX as u64 {
        write_full_box(v, b"co64", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            write_stco(v, stream, true, is_fragmented)
        })?;
    } else {
        write_full_box(v, b"stco", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            write_stco(v, stream, false, is_fragmented)
        })?;
    }

    for auxiliary_information in &stream.auxiliary_info {
        if !auxiliary_information.entries.is_empty() {
            auxiliary_information.write_full_saiz(v)?;
            auxiliary_information.write_full_saio(v)?;
        }
    }

    Ok(())
}

fn write_stts(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    is_fragmented: bool,
) -> Result<(), Error> {
    if is_fragmented {
        // Entry count
        v.extend(0u32.to_be_bytes());
    } else {
        let timescale = stream.trak_timescale;

        let entry_count_position = v.len();
        // Entry count, rewritten in the end
        v.extend(0u32.to_be_bytes());

        let mut last_duration: Option<u32> = None;
        let mut sample_count = 0u32;
        let mut num_entries = 0u32;
        for duration in stream
            .chunks
            .iter()
            .flat_map(|c| c.samples.iter().map(|b| b.duration))
        {
            let duration = u32::try_from(
                duration
                    .nseconds()
                    .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
                    .context("too big sample duration")?,
            )
            .context("too big sample duration")?;

            if last_duration != Some(duration) {
                if let Some(last_duration) = last_duration {
                    v.extend(sample_count.to_be_bytes());
                    v.extend(last_duration.to_be_bytes());
                    num_entries += 1;
                }

                last_duration = Some(duration);
                sample_count = 1;
            } else {
                sample_count += 1;
            }
        }

        if let Some(last_duration) = last_duration {
            v.extend(sample_count.to_be_bytes());
            v.extend(last_duration.to_be_bytes());
            num_entries += 1;
        }

        // Rewrite entry count
        v[entry_count_position..][..4].copy_from_slice(&num_entries.to_be_bytes());
    }

    Ok(())
}

fn write_ctts(v: &mut Vec<u8>, stream: &TrackConfiguration, version: u8) -> Result<(), Error> {
    let timescale = stream.trak_timescale;

    let entry_count_position = v.len();
    // Entry count, rewritten in the end
    v.extend(0u32.to_be_bytes());

    let mut last_composition_time_offset = None;
    let mut sample_count = 0u32;
    let mut num_entries = 0u32;
    for composition_time_offset in stream
        .chunks
        .iter()
        .flat_map(|c| c.samples.iter().map(|b| b.composition_time_offset))
    {
        let composition_time_offset = composition_time_offset
            .expect("not all samples have a composition time offset")
            .mul_div_round(timescale as i64, gst::ClockTime::SECOND.nseconds() as i64)
            .context("too big sample composition time offset")?;

        if last_composition_time_offset != Some(composition_time_offset) {
            if let Some(last_composition_time_offset) = last_composition_time_offset {
                v.extend(sample_count.to_be_bytes());
                if version == FULL_BOX_VERSION_0 {
                    let last_composition_time_offset = u32::try_from(last_composition_time_offset)
                        .context("too big sample composition time offset")?;

                    v.extend(last_composition_time_offset.to_be_bytes());
                } else {
                    let last_composition_time_offset = i32::try_from(last_composition_time_offset)
                        .context("too big sample composition time offset")?;
                    v.extend(last_composition_time_offset.to_be_bytes());
                }
                num_entries += 1;
            }

            last_composition_time_offset = Some(composition_time_offset);
            sample_count = 1;
        } else {
            sample_count += 1;
        }
    }

    if let Some(last_composition_time_offset) = last_composition_time_offset {
        v.extend(sample_count.to_be_bytes());
        if version == FULL_BOX_VERSION_0 {
            let last_composition_time_offset = u32::try_from(last_composition_time_offset)
                .context("too big sample composition time offset")?;

            v.extend(last_composition_time_offset.to_be_bytes());
        } else {
            let last_composition_time_offset = i32::try_from(last_composition_time_offset)
                .context("too big sample composition time offset")?;
            v.extend(last_composition_time_offset.to_be_bytes());
        }
        num_entries += 1;
    }

    // Rewrite entry count
    v[entry_count_position..][..4].copy_from_slice(&num_entries.to_be_bytes());

    Ok(())
}

fn write_cslg(v: &mut Vec<u8>, stream: &TrackConfiguration) -> Result<(), Error> {
    let timescale = stream.trak_timescale;

    let (min_ctts, max_ctts) = stream
        .chunks
        .iter()
        .flat_map(|c| {
            c.samples.iter().map(|b| {
                b.composition_time_offset
                    .expect("not all samples have a composition time offset")
            })
        })
        .fold((None, None), |(min, max), ctts| {
            (
                if min.is_none_or(|min| ctts < min) {
                    Some(ctts)
                } else {
                    min
                },
                if max.is_none_or(|max| ctts > max) {
                    Some(ctts)
                } else {
                    max
                },
            )
        });
    let min_ctts = min_ctts
        .unwrap()
        .mul_div_round(timescale as i64, gst::ClockTime::SECOND.nseconds() as i64)
        .context("too big composition time offset")?;
    let max_ctts = max_ctts
        .unwrap()
        .mul_div_round(timescale as i64, gst::ClockTime::SECOND.nseconds() as i64)
        .context("too big composition time offset")?;

    // Composition to DTS shift
    v.extend((-min_ctts).to_be_bytes());

    // least decode to display delta
    v.extend(min_ctts.to_be_bytes());

    // greatest decode to display delta
    v.extend(max_ctts.to_be_bytes());

    // composition start time
    let composition_start_time = stream
        .earliest_pts
        .nseconds()
        .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .context("too earliest PTS")?;
    v.extend(composition_start_time.to_be_bytes());

    // composition end time
    let composition_end_time = stream
        .end_pts
        .nseconds()
        .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .context("too end PTS")?;
    v.extend(composition_end_time.to_be_bytes());

    Ok(())
}

fn write_stss(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    is_fragmented: bool,
) -> Result<(), Error> {
    if is_fragmented {
        // Entry count
        v.extend(0u32.to_be_bytes());
    } else {
        let entry_count_position = v.len();
        // Entry count, rewritten in the end
        v.extend(0u32.to_be_bytes());

        let mut num_entries = 0u32;
        for (idx, _sync_point) in stream
            .chunks
            .iter()
            .flat_map(|c| c.samples.iter().map(|b| b.sync_point))
            .enumerate()
            .filter(|(_idx, sync_point)| *sync_point)
        {
            v.extend((idx as u32 + 1).to_be_bytes());
            num_entries += 1;
        }

        // Rewrite entry count
        v[entry_count_position..][..4].copy_from_slice(&num_entries.to_be_bytes());
    }

    Ok(())
}

fn write_stsz(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    is_fragmented: bool,
) -> Result<(), Error> {
    if is_fragmented {
        // Sample size
        v.extend(0u32.to_be_bytes());

        // Sample count
        v.extend(0u32.to_be_bytes());
    } else {
        let first_sample_size = stream.chunks[0].samples[0].size;

        if stream
            .chunks
            .iter()
            .flat_map(|c| c.samples.iter().map(|b| b.size))
            .all(|size| size == first_sample_size)
        {
            // Sample size
            v.extend(first_sample_size.to_be_bytes());

            // Sample count
            let sample_count = stream
                .chunks
                .iter()
                .map(|c| c.samples.len() as u32)
                .sum::<u32>();
            v.extend(sample_count.to_be_bytes());
        } else {
            // Sample size
            v.extend(0u32.to_be_bytes());

            // Sample count, will be rewritten later
            let sample_count_position = v.len();
            let mut sample_count = 0u32;
            v.extend(0u32.to_be_bytes());

            for size in stream
                .chunks
                .iter()
                .flat_map(|c| c.samples.iter().map(|b| b.size))
            {
                v.extend(size.to_be_bytes());
                sample_count += 1;
            }

            v[sample_count_position..][..4].copy_from_slice(&sample_count.to_be_bytes());
        }
    }

    Ok(())
}

fn write_stsc(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    is_fragmented: bool,
) -> Result<(), Error> {
    if is_fragmented {
        // Entry count
        v.extend(0u32.to_be_bytes());
    } else {
        let entry_count_position = v.len();
        // Entry count, rewritten in the end
        v.extend(0u32.to_be_bytes());

        let mut num_entries = 0u32;
        let mut first_chunk = 1u32;
        let mut samples_per_chunk: Option<u32> = None;
        for (idx, chunk) in stream.chunks.iter().enumerate() {
            if samples_per_chunk != Some(chunk.samples.len() as u32) {
                if let Some(samples_per_chunk) = samples_per_chunk {
                    v.extend(first_chunk.to_be_bytes());
                    v.extend(samples_per_chunk.to_be_bytes());
                    // sample description index
                    v.extend(1u32.to_be_bytes());
                    num_entries += 1;
                }
                samples_per_chunk = Some(chunk.samples.len() as u32);
                first_chunk = idx as u32 + 1;
            }
        }

        if let Some(samples_per_chunk) = samples_per_chunk {
            v.extend(first_chunk.to_be_bytes());
            v.extend(samples_per_chunk.to_be_bytes());
            // sample description index
            v.extend(1u32.to_be_bytes());
            num_entries += 1;
        }

        // Rewrite entry count
        v[entry_count_position..][..4].copy_from_slice(&num_entries.to_be_bytes());
    }

    Ok(())
}

fn write_stco(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    co64: bool,
    is_fragmented: bool,
) -> Result<(), Error> {
    if is_fragmented {
        // Entry count
        v.extend(0u32.to_be_bytes());
    } else {
        // Entry count
        v.extend((stream.chunks.len() as u32).to_be_bytes());

        for chunk in &stream.chunks {
            if co64 {
                v.extend(chunk.offset.to_be_bytes());
            } else {
                v.extend(u32::try_from(chunk.offset).unwrap().to_be_bytes());
            }
        }
    }

    Ok(())
}

struct TrackReference {
    reference_type: [u8; 4],
    track_ids: Vec<u32>,
}

fn write_vmhd(v: &mut Vec<u8>) -> Result<(), Error> {
    // Graphics mode
    v.extend([0u8; 2]);

    // opcolor
    v.extend([0u8; 2 * 3]);

    Ok(())
}

fn write_smhd(v: &mut Vec<u8>) -> Result<(), Error> {
    // Balance
    v.extend([0u8; 2]);

    // Reserved
    v.extend([0u8; 2]);

    Ok(())
}

fn write_dinf(v: &mut Vec<u8>) -> Result<(), Error> {
    write_full_box(v, b"dref", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_dref(v)
    })?;

    Ok(())
}

const DREF_FLAGS_MEDIA_IN_SAME_FILE: u32 = 0x1;

fn write_dref(v: &mut Vec<u8>) -> Result<(), Error> {
    // Entry count
    v.extend(1u32.to_be_bytes());

    write_full_box(
        v,
        b"url ",
        FULL_BOX_VERSION_0,
        DREF_FLAGS_MEDIA_IN_SAME_FILE,
        |_v| Ok(()),
    )?;

    Ok(())
}

fn write_sample_entry_box<T, F: FnOnce(&mut Vec<u8>) -> Result<T, Error>>(
    v: &mut Vec<u8>,
    fourcc: impl std::borrow::Borrow<[u8; 4]>,
    content_func: F,
) -> Result<T, Error> {
    write_box(v, fourcc, move |v| {
        // Reserved
        v.extend([0u8; 6]);

        // Data reference index
        v.extend(1u16.to_be_bytes());

        content_func(v)
    })
}

fn write_dops(v: &mut Vec<u8>, caps: &gst::Caps) -> Result<(), Error> {
    let rate;
    let channels;
    let channel_mapping_family;
    let stream_count;
    let coupled_count;
    let pre_skip;
    let output_gain;
    let mut channel_mapping = [0; 256];

    // TODO: Use audio clipping meta to calculate pre_skip

    if let Some(header) = caps
        .structure(0)
        .unwrap()
        .get::<gst::ArrayRef>("streamheader")
        .ok()
        .and_then(|a| a.first().and_then(|v| v.get::<gst::Buffer>().ok()))
    {
        (
            rate,
            channels,
            channel_mapping_family,
            stream_count,
            coupled_count,
            pre_skip,
            output_gain,
        ) = gst_pbutils::codec_utils_opus_parse_header(&header, Some(&mut channel_mapping))
            .unwrap();
    } else {
        (
            rate,
            channels,
            channel_mapping_family,
            stream_count,
            coupled_count,
        ) = gst_pbutils::codec_utils_opus_parse_caps(caps, Some(&mut channel_mapping)).unwrap();
        output_gain = 0;
        pre_skip = 0;
    }

    write_box(v, b"dOps", move |v| {
        // Version number
        v.push(0);
        v.push(channels);
        v.extend(pre_skip.to_be_bytes());
        v.extend(rate.to_be_bytes());
        v.extend(output_gain.to_be_bytes());
        v.push(channel_mapping_family);
        if channel_mapping_family > 0 {
            v.push(stream_count);
            v.push(coupled_count);
            v.extend(&channel_mapping[..channels as usize]);
        }

        Ok(())
    })
}

fn write_xml_meta_data_sample_entry(
    v: &mut Vec<u8>,
    stream: &super::TrackConfiguration,
) -> Result<(), Error> {
    let s = stream.caps.structure(0).unwrap();
    let namespace = match s.name().as_str() {
        "application/x-onvif-metadata" => b"http://www.onvif.org/ver10/schema",
        _ => unreachable!(),
    };

    write_sample_entry_box(v, b"metx", move |v| {
        // content_encoding, empty string
        v.push(0);

        // namespace
        v.extend_from_slice(namespace);
        v.push(0);

        // schema_location, empty string list
        v.push(0);

        Ok(())
    })?;

    Ok(())
}

fn write_mvhd(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    creation_time: u64,
) -> Result<(), Error> {
    let timescale = cfg.to_timescale();
    // Creation time
    v.extend(creation_time.to_be_bytes());
    // Modification time
    v.extend(creation_time.to_be_bytes());
    // Timescale
    v.extend(timescale.to_be_bytes());
    // Duration
    if cfg.variant.is_fragmented() {
        v.extend(0u64.to_be_bytes());
    } else {
        let min_earliest_pts = cfg.tracks.iter().map(|s| s.earliest_pts).min().unwrap();
        let max_end_pts = cfg
            .tracks
            .iter()
            .map(|stream| stream.end_pts)
            .max()
            .unwrap();
        let duration = (max_end_pts - min_earliest_pts)
            .nseconds()
            .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
            .context("too big track duration")?;
        v.extend(duration.to_be_bytes());
    }

    // Rate 1.0
    v.extend((1u32 << 16).to_be_bytes());
    // Volume 1.0
    v.extend((1u16 << 8).to_be_bytes());
    // Reserved
    v.extend([0u8; 2 + 2 * 4]);

    // Matrix
    v.extend(IDENTITY_MATRIX.iter().flatten());

    // Pre defined
    v.extend([0u8; 6 * 4]);

    // Next track id
    v.extend((cfg.tracks.len() as u32 + 1).to_be_bytes());

    Ok(())
}

pub(crate) fn write_hdlr_box(
    v: &mut Vec<u8>,
    handler_type: &[u8; 4],
    name: &[u8],
) -> Result<(), Error> {
    // Pre-defined
    v.extend([0u8; 4]);

    // Handler type
    v.extend(handler_type);

    // Reserved
    v.extend([0u8; 3 * 4]);

    // Name
    v.extend(name);

    Ok(())
}

fn write_hdlr_for_stream(v: &mut Vec<u8>, stream: &TrackConfiguration) -> Result<(), Error> {
    let s = stream.caps.structure(0).unwrap();
    let (handler_type, name) = match s.name().as_str() {
        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9" | "video/x-av1"
        | "image/jpeg" | "video/x-raw" => {
            if stream.image_sequence {
                // See ISO/IEC 23008-12:2022 Section 7.2.2
                (b"pict", b"PictureHandler\0".as_slice())
            } else {
                (b"vide", b"VideoHandler\0".as_slice())
            }
        }
        "audio/mpeg" | "audio/x-opus" | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw"
        | "audio/x-adpcm" | "audio/x-ac3" | "audio/x-eac3" => {
            (b"soun", b"SoundHandler\0".as_slice())
        }
        "application/x-onvif-metadata" => (b"meta", b"MetadataHandler\0".as_slice()),
        _ => unreachable!(),
    };
    write_hdlr_box(v, handler_type, name)
}

fn write_tkhd(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    idx: usize,
    stream: &TrackConfiguration,
    creation_time: u64,
) -> Result<(), Error> {
    // Creation time
    v.extend(creation_time.to_be_bytes());
    // Modification time
    v.extend(creation_time.to_be_bytes());
    // Track ID
    v.extend((idx as u32 + 1).to_be_bytes());
    // Reserved
    v.extend(0u32.to_be_bytes());
    // Duration
    if cfg.variant.is_fragmented() {
        v.extend(0u64.to_be_bytes());
    } else {
        // Track header duration is in movie header timescale
        let timescale = cfg.to_timescale();

        let min_earliest_pts = cfg.tracks.iter().map(|s| s.earliest_pts).min().unwrap();
        // Duration is the end PTS of this stream up to the beginning of the earliest stream
        let duration = stream.end_pts - min_earliest_pts;
        let duration = duration
            .nseconds()
            .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
            .context("too big track duration")?;
        v.extend(duration.to_be_bytes());
    }
    // Reserved
    v.extend([0u8; 2 * 4]);

    // Layer
    v.extend(0u16.to_be_bytes());
    // Alternate group
    v.extend(0u16.to_be_bytes());

    // Volume
    let s = stream.caps.structure(0).unwrap();
    match s.name().as_str() {
        "audio/mpeg" | "audio/x-opus" | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw"
        | "audio/x-adpcm" | "audio/x-ac3" | "audio/x-eac3" => v.extend((1u16 << 8).to_be_bytes()),
        _ => v.extend(0u16.to_be_bytes()),
    }

    // Reserved
    v.extend([0u8; 2]);

    // Per stream orientation matrix for video
    let matrix = match s.name().as_str() {
        x if x.starts_with("video/") || x.starts_with("image/") => stream.orientation,
        _ => &crate::isobmff::transform_matrix::IDENTITY_MATRIX,
    };
    v.extend(matrix.iter().flatten());

    // Width/height
    match s.name().as_str() {
        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9" | "video/x-av1"
        | "image/jpeg" | "video/x-raw" => {
            let width = s.get::<i32>("width").context("video caps without width")? as u32;
            let height = s
                .get::<i32>("height")
                .context("video caps without height")? as u32;
            let par = s
                .get::<gst::Fraction>("pixel-aspect-ratio")
                .unwrap_or_else(|_| gst::Fraction::new(1, 1));

            let width = std::cmp::min(
                width
                    .mul_div_round(par.numer() as u32, par.denom() as u32)
                    .unwrap_or(u16::MAX as u32),
                u16::MAX as u32,
            );
            let height = std::cmp::min(height, u16::MAX as u32);

            v.extend((width << 16).to_be_bytes());
            v.extend((height << 16).to_be_bytes());
        }
        _ => v.extend([0u8; 2 * 4]),
    }

    Ok(())
}

fn write_mdhd(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    stream: &TrackConfiguration,
    creation_time: u64,
) -> Result<(), Error> {
    let timescale = stream.to_timescale();

    // Creation time
    v.extend(creation_time.to_be_bytes());
    // Modification time
    v.extend(creation_time.to_be_bytes());
    // Timescale
    v.extend(timescale.to_be_bytes());
    // Duration
    if cfg.variant.is_fragmented() {
        v.extend(0u64.to_be_bytes());
    } else {
        let duration = stream
            .chunks
            .iter()
            .flat_map(|c| c.samples.iter().map(|b| b.duration.nseconds()))
            .sum::<u64>()
            .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
            .context("too big track duration")?;
        v.extend(duration.to_be_bytes());
    }

    // Language as ISO-639-2/T
    if let Some(lang) = stream.language_code {
        v.extend(language_code(lang).to_be_bytes());
    } else {
        v.extend(language_code(b"und").to_be_bytes());
    }

    // Pre-defined
    v.extend([0u8; 2]);

    Ok(())
}

fn write_tref(v: &mut Vec<u8>, references: &[TrackReference]) -> Result<(), Error> {
    for reference in references {
        write_box(v, reference.reference_type, |v| {
            for track_id in &reference.track_ids {
                v.extend(track_id.to_be_bytes());
            }

            Ok(())
        })?;
    }

    Ok(())
}

fn language_code(lang: impl std::borrow::Borrow<[u8; 3]>) -> u16 {
    let lang = lang.borrow();

    assert!(lang.iter().all(u8::is_ascii_lowercase));

    (((lang[0] as u16 - 0x60) & 0x1F) << 10)
        + (((lang[1] as u16 - 0x60) & 0x1F) << 5)
        + ((lang[2] as u16 - 0x60) & 0x1F)
}

pub(crate) fn write_esds_aac(
    v: &mut Vec<u8>,
    stream: &TrackConfiguration,
    codec_data: &[u8],
) -> Result<(), Error> {
    let calculate_len = |mut len| {
        if len > 260144641 {
            bail!("too big descriptor length");
        }

        if len == 0 {
            return Ok(([0; 4], 1));
        }

        let mut idx = 0;
        let mut lens = [0u8; 4];
        while len > 0 {
            lens[idx] = ((if len > 0x7f { 0x80 } else { 0x00 }) | (len & 0x7f)) as u8;
            idx += 1;
            len >>= 7;
        }

        Ok((lens, idx))
    };

    write_full_box(
        v,
        b"esds",
        FULL_BOX_VERSION_0,
        FULL_BOX_FLAGS_NONE,
        move |v| {
            // Calculate all lengths bottom up

            // Decoder specific info
            let decoder_specific_info_len = calculate_len(codec_data.len())?;

            // Decoder config
            let decoder_config_len =
                calculate_len(13 + 1 + decoder_specific_info_len.1 + codec_data.len())?;

            // SL config
            let sl_config_len = calculate_len(1)?;

            // ES descriptor
            let es_descriptor_len = calculate_len(
                3 + 1
                    + decoder_config_len.1
                    + 13
                    + 1
                    + decoder_specific_info_len.1
                    + codec_data.len()
                    + 1
                    + sl_config_len.1
                    + 1,
            )?;

            // ES descriptor tag
            v.push(0x03);

            // Length
            v.extend_from_slice(&es_descriptor_len.0[..(es_descriptor_len.1)]);

            // Track ID
            v.extend(1u16.to_be_bytes());
            // Flags
            v.push(0u8);

            // Decoder config descriptor
            v.push(0x04);

            // Length
            v.extend_from_slice(&decoder_config_len.0[..(decoder_config_len.1)]);

            // Object type ESDS_OBJECT_TYPE_MPEG4_P3
            v.push(0x40);
            // Stream type ESDS_STREAM_TYPE_AUDIO
            v.push((0x05 << 2) | 0x01);

            // Buffer size db?
            v.extend([0u8; 3]);

            // Max bitrate
            v.extend(stream.max_bitrate.unwrap_or(0u32).to_be_bytes());

            // Avg bitrate
            v.extend(stream.avg_bitrate.unwrap_or(0u32).to_be_bytes());

            // Decoder specific info
            v.push(0x05);

            // Length
            v.extend_from_slice(&decoder_specific_info_len.0[..(decoder_specific_info_len.1)]);
            v.extend_from_slice(codec_data);

            // SL config descriptor
            v.push(0x06);

            // Length: 1 (tag) + 1 (length) + 1 (predefined)
            v.extend_from_slice(&sl_config_len.0[..(sl_config_len.1)]);

            // Predefined
            v.push(0x02);
            Ok(())
        },
    )
}

fn write_edts(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    stream: &TrackConfiguration,
) -> Result<(), Error> {
    write_full_box(v, b"elst", FULL_BOX_VERSION_1, 0, |v| {
        write_elst(v, cfg, stream)
    })?;

    Ok(())
}

fn write_elst(
    v: &mut Vec<u8>,
    cfg: &PresentationConfiguration,
    stream: &TrackConfiguration,
) -> Result<(), Error> {
    let movie_timescale = cfg.to_timescale();
    let track_timescale = stream.to_timescale();

    // Entry count
    let mut num_entries = 0u32;
    let entry_count_position = v.len();
    // Entry count, rewritten in the end
    v.extend(0u32.to_be_bytes());

    for elst_info in &stream.elst_infos {
        // Edit duration (in movie timescale)
        let edit_duration = elst_info
            .duration
            .expect("Should have been set by `get_elst_infos`")
            .nseconds()
            .mul_div_round(movie_timescale as u64, gst::ClockTime::SECOND.nseconds())
            .unwrap();

        if edit_duration == 0 {
            continue;
        }
        v.extend(edit_duration.to_be_bytes());

        // Media time (in media timescale)
        let media_time = elst_info
            .start
            .map(|start| {
                i64::try_from(start)
                    .unwrap()
                    .mul_div_round(
                        track_timescale as i64,
                        gst::ClockTime::SECOND.nseconds() as i64,
                    )
                    .unwrap()
            })
            .unwrap_or(-1i64);
        v.extend(media_time.to_be_bytes());

        // Media rate
        v.extend(1u16.to_be_bytes());
        v.extend(0u16.to_be_bytes());
        num_entries += 1;
    }

    // Rewrite entry count
    v[entry_count_position..][..4].copy_from_slice(&num_entries.to_be_bytes());

    Ok(())
}

pub(crate) fn write_stsd(v: &mut Vec<u8>, stream: &TrackConfiguration) -> Result<(), Error> {
    // Entry count
    v.extend(1u32.to_be_bytes());

    let s = stream.caps.structure(0).unwrap();
    match s.name().as_str() {
        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9" | "video/x-av1"
        | "image/jpeg" | "video/x-raw" => write_visual_sample_entry(v, stream)?,
        "audio/mpeg" | "audio/x-opus" | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw"
        | "audio/x-adpcm" | "audio/x-ac3" | "audio/x-eac3" => write_audio_sample_entry(v, stream)?,
        "application/x-onvif-metadata" => write_xml_meta_data_sample_entry(v, stream)?,
        _ => unreachable!(),
    }

    Ok(())
}

fn write_visual_sample_entry(v: &mut Vec<u8>, stream: &TrackConfiguration) -> Result<(), Error> {
    let s = stream.caps.structure(0).unwrap();
    let fourcc = match s.name().as_str() {
        "video/x-h264" => {
            let stream_format = s.get::<&str>("stream-format").context("no stream-format")?;
            match stream_format {
                "avc" => b"avc1",
                "avc3" => b"avc3",
                _ => unreachable!(),
            }
        }
        "video/x-h265" => {
            let stream_format = s.get::<&str>("stream-format").context("no stream-format")?;
            match stream_format {
                "hvc1" => b"hvc1",
                "hev1" => b"hev1",
                _ => unreachable!(),
            }
        }
        "image/jpeg" => b"jpeg",
        "video/x-vp8" => b"vp08",
        "video/x-vp9" => b"vp09",
        "video/x-av1" => b"av01",
        "video/x-raw" => b"uncv",
        _ => unreachable!(),
    };

    write_sample_entry_box(v, fourcc, move |v| {
        // pre-defined
        v.extend([0u8; 2]);
        // Reserved
        v.extend([0u8; 2]);
        // pre-defined
        v.extend([0u8; 3 * 4]);

        // Width
        let width =
            u16::try_from(s.get::<i32>("width").context("no width")?).context("too big width")?;
        v.extend(width.to_be_bytes());

        // Height
        let height = u16::try_from(s.get::<i32>("height").context("no height")?)
            .context("too big height")?;
        v.extend(height.to_be_bytes());

        // Horizontal resolution
        v.extend(0x00480000u32.to_be_bytes());

        // Vertical resolution
        v.extend(0x00480000u32.to_be_bytes());

        // Reserved
        v.extend([0u8; 4]);

        // Frame count
        v.extend(1u16.to_be_bytes());

        // Compressor name
        v.extend([0u8; 32]);

        // Depth
        v.extend(0x0018u16.to_be_bytes());

        // Pre-defined
        v.extend((-1i16).to_be_bytes());

        // Codec specific boxes
        match s.name().as_str() {
            "video/x-h264" => {
                let codec_data = s
                    .get::<&gst::BufferRef>("codec_data")
                    .context("no codec_data")?;
                let map = codec_data
                    .map_readable()
                    .context("codec_data not mappable")?;
                write_box(v, b"avcC", move |v| {
                    v.extend_from_slice(&map);
                    Ok(())
                })?;
            }
            "video/x-h265" => {
                let codec_data = s
                    .get::<&gst::BufferRef>("codec_data")
                    .context("no codec_data")?;
                let map = codec_data
                    .map_readable()
                    .context("codec_data not mappable")?;
                write_box(v, b"hvcC", move |v| {
                    v.extend_from_slice(&map);
                    Ok(())
                })?;
            }
            "video/x-vp9" => {
                let profile: u8 = match s.get::<&str>("profile").expect("no vp9 profile") {
                    "0" => Some(0),
                    "1" => Some(1),
                    "2" => Some(2),
                    "3" => Some(3),
                    _ => None,
                }
                .context("unsupported vp9 profile")?;
                let colorimetry = gst_video::VideoColorimetry::from_str(
                    s.get::<&str>("colorimetry").expect("no colorimetry"),
                )
                .context("failed to parse colorimetry")?;
                let video_full_range =
                    colorimetry.range() == gst_video::VideoColorRange::Range0_255;
                let chroma_format: u8 =
                    match s.get::<&str>("chroma-format").expect("no chroma-format") {
                        "4:2:0" =>
                        // chroma-site is optional
                        {
                            match s
                                .get::<&str>("chroma-site")
                                .ok()
                                .and_then(|cs| gst_video::VideoChromaSite::from_str(cs).ok())
                            {
                                Some(gst_video::VideoChromaSite::V_COSITED) => Some(0),
                                // COSITED
                                _ => Some(1),
                            }
                        }
                        "4:2:2" => Some(2),
                        "4:4:4" => Some(3),
                        _ => None,
                    }
                    .context("unsupported chroma-format")?;
                let bit_depth: u8 = {
                    let bit_depth_luma = s.get::<u32>("bit-depth-luma").expect("no bit-depth-luma");
                    let bit_depth_chroma = s
                        .get::<u32>("bit-depth-chroma")
                        .expect("no bit-depth-chroma");
                    if bit_depth_luma != bit_depth_chroma {
                        return Err(anyhow!("bit-depth-luma and bit-depth-chroma have different values which is an unsupported configuration"));
                    }
                    bit_depth_luma as u8
                };
                write_full_box(v, b"vpcC", 1, 0, move |v| {
                    v.push(profile);
                    // XXX: hardcoded level 1
                    v.push(10);
                    let mut byte: u8 = 0;
                    byte |= (bit_depth & 0xF) << 4;
                    byte |= (chroma_format & 0x7) << 1;
                    byte |= video_full_range as u8;
                    v.push(byte);
                    v.push(colorimetry.primaries().to_iso() as u8);
                    v.push(colorimetry.transfer().to_iso() as u8);
                    v.push(colorimetry.matrix().to_iso() as u8);
                    // 16-bit length field for codec initialization, unused
                    v.push(0);
                    v.push(0);
                    Ok(())
                })?;
            }
            "video/x-av1" => {
                write_box(v, b"av1C", move |v| {
                    if let Ok(codec_data) = s.get::<&gst::BufferRef>("codec_data") {
                        let map = codec_data
                            .map_readable()
                            .context("codec_data not mappable")?;

                        v.extend_from_slice(&map);
                    } else {
                        let presentation_delay_minus_one =
                            if let Ok(presentation_delay) = s.get::<i32>("presentation-delay") {
                                Some(
                                    (1u8 << 5)
                                        | std::cmp::max(
                                            0xF,
                                            (presentation_delay.saturating_sub(1) & 0xF) as u8,
                                        ),
                                )
                            } else {
                                None
                            };

                        let profile = match s.get::<&str>("profile").unwrap() {
                            "main" => 0,
                            "high" => 1,
                            "professional" => 2,
                            _ => unreachable!(),
                        };
                        let level = av1_seq_level_idx(s.get::<&str>("level").ok());
                        let tier = av1_tier(s.get::<&str>("tier").ok());
                        let (high_bitdepth, twelve_bit) =
                            match s.get::<u32>("bit-depth-luma").unwrap() {
                                8 => (false, false),
                                10 => (true, false),
                                12 => (true, true),
                                _ => unreachable!(),
                            };
                        let (monochrome, chroma_sub_x, chroma_sub_y) =
                            match s.get::<&str>("chroma-format").unwrap() {
                                "4:0:0" => (true, true, true),
                                "4:2:0" => (false, true, true),
                                "4:2:2" => (false, true, false),
                                "4:4:4" => (false, false, false),
                                _ => unreachable!(),
                            };

                        let chrome_sample_position = match s.get::<&str>("chroma-site") {
                            Ok("v-cosited") => 1,
                            Ok("v-cosited+h-cosited") => 2,
                            _ => 0,
                        };

                        let codec_data = [
                            0x80 | 0x01,            // marker | version
                            (profile << 5) | level, // profile | level
                            (tier << 7)
                                | ((high_bitdepth as u8) << 6)
                                | ((twelve_bit as u8) << 5)
                                | ((monochrome as u8) << 4)
                                | ((chroma_sub_x as u8) << 3)
                                | ((chroma_sub_y as u8) << 2)
                                | chrome_sample_position, // tier | high bitdepth | twelve bit | monochrome | chroma sub x |
                            // chroma sub y | chroma sample position
                            if let Some(presentation_delay_minus_one) = presentation_delay_minus_one
                            {
                                0x10 | presentation_delay_minus_one // reserved | presentation delay present | presentation delay
                            } else {
                                0
                            },
                        ];

                        v.extend_from_slice(&codec_data);
                    }

                    if let Some(extra_data) = &stream.extra_header_data {
                        // unsigned int(8) configOBUs[];
                        v.extend_from_slice(extra_data.as_slice());
                    }
                    Ok(())
                })?;
            }
            "video/x-vp8" | "image/jpeg" => {
                // Nothing to do here
            }
            "video/x-raw" => {
                let video_info = gst_video::VideoInfo::from_caps(&stream.caps).unwrap();
                write_uncompressed_sample_entries(v, video_info)?
            }
            _ => unreachable!(),
        }

        if let Ok(par) = s.get::<gst::Fraction>("pixel-aspect-ratio") {
            write_box(v, b"pasp", move |v| {
                v.extend((par.numer() as u32).to_be_bytes());
                v.extend((par.denom() as u32).to_be_bytes());
                Ok(())
            })?;
        }

        if let Some(colorimetry) = s
            .get::<&str>("colorimetry")
            .ok()
            .and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
        {
            write_box(v, b"colr", move |v| {
                v.extend(b"nclx");
                let (primaries, transfer, matrix) = {
                    (
                        (colorimetry.primaries().to_iso() as u16),
                        (colorimetry.transfer().to_iso() as u16),
                        (colorimetry.matrix().to_iso() as u16),
                    )
                };

                let full_range = match colorimetry.range() {
                    gst_video::VideoColorRange::Range0_255 => 0x80u8,
                    gst_video::VideoColorRange::Range16_235 => 0x00u8,
                    _ => 0x00,
                };

                v.extend(primaries.to_be_bytes());
                v.extend(transfer.to_be_bytes());
                v.extend(matrix.to_be_bytes());
                v.push(full_range);

                Ok(())
            })?;
        }

        if let Ok(cll) = gst_video::VideoContentLightLevel::from_caps(&stream.caps) {
            write_box(v, b"clli", move |v| {
                v.extend((cll.max_content_light_level()).to_be_bytes());
                v.extend((cll.max_frame_average_light_level()).to_be_bytes());
                Ok(())
            })?;
        }

        if let Ok(mastering) = gst_video::VideoMasteringDisplayInfo::from_caps(&stream.caps) {
            write_box(v, b"mdcv", move |v| {
                for primary in mastering.display_primaries() {
                    v.extend(primary.x.to_be_bytes());
                    v.extend(primary.y.to_be_bytes());
                }
                v.extend(mastering.white_point().x.to_be_bytes());
                v.extend(mastering.white_point().y.to_be_bytes());
                v.extend(mastering.max_display_mastering_luminance().to_be_bytes());
                v.extend(mastering.max_display_mastering_luminance().to_be_bytes());
                Ok(())
            })?;
        }

        // Write fiel box for codecs that require it
        if ["image/jpeg"].contains(&s.name().as_str()) {
            let interlace_mode = s
                .get::<&str>("interlace-mode")
                .ok()
                .map(gst_video::VideoInterlaceMode::from_string)
                .unwrap_or(gst_video::VideoInterlaceMode::Progressive);
            let field_order = s
                .get::<&str>("field-order")
                .ok()
                .map(gst_video::VideoFieldOrder::from_string)
                .unwrap_or(gst_video::VideoFieldOrder::Unknown);

            write_box(v, b"fiel", move |v| {
                let (interlace, field_order) = match interlace_mode {
                    gst_video::VideoInterlaceMode::Progressive => (1, 0),
                    gst_video::VideoInterlaceMode::Interleaved
                        if field_order == gst_video::VideoFieldOrder::TopFieldFirst =>
                    {
                        (2, 9)
                    }
                    gst_video::VideoInterlaceMode::Interleaved => (2, 14),
                    _ => (0, 0),
                };

                v.push(interlace);
                v.push(field_order);
                Ok(())
            })?;
        }

        if stream.image_sequence {
            match s.name().as_str() {
                // intra formats
                "video/x-vp9" | "video/x-vp8" | "image/jpeg" => {
                    let all_ref_pics_intra = 1u32; // 0 = don't know, 1 = reference pictures are only intra
                    let intra_pred_used = 1u32; // 0 = no, 1 = yes, or maybe
                    let max_ref_per_pic = 0u32; // none number
                    let packed_bits = (all_ref_pics_intra << 31)
                        | (intra_pred_used << 30)
                        | (max_ref_per_pic << 26);
                    write_full_box(v, b"ccst", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
                        v.extend(packed_bits.to_be_bytes());
                        Ok(())
                    })?;
                }
                // uncompressed
                "video/x-raw" => {
                    let all_ref_pics_intra = 1u32; // 0 = don't know, 1 = reference pictures are only intra
                    let intra_pred_used = 0u32; // 0 = no, 1 = yes, or maybe
                    let max_ref_per_pic = 0u32; // none
                    let packed_bits = (all_ref_pics_intra << 31)
                        | (intra_pred_used << 30)
                        | (max_ref_per_pic << 26);
                    write_full_box(v, b"ccst", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
                        v.extend(packed_bits.to_be_bytes());
                        Ok(())
                    })?;
                }
                _ => {
                    let all_ref_pics_intra = 0u32; // 0 = don't know, 1 = reference pictures are only intra
                    let intra_pred_used = 1u32; // 0 = no, 1 = yes, or maybe
                    let max_ref_per_pic = 15u32; // any number
                    let packed_bits = (all_ref_pics_intra << 31)
                        | (intra_pred_used << 30)
                        | (max_ref_per_pic << 26);
                    write_full_box(v, b"ccst", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
                        v.extend(packed_bits.to_be_bytes());
                        Ok(())
                    })?;
                }
            }
        }

        if stream.avg_bitrate.is_some() || stream.max_bitrate.is_some() {
            write_box(v, b"btrt", |v| {
                // Buffer size DB
                // TODO
                v.extend(0u32.to_be_bytes());

                // Maximum bitrate
                let max_bitrate = stream.max_bitrate.or(stream.avg_bitrate).unwrap();
                v.extend(max_bitrate.to_be_bytes());

                // Average bitrate
                let avg_bitrate = stream.avg_bitrate.or(stream.max_bitrate).unwrap();
                v.extend(avg_bitrate.to_be_bytes());

                Ok(())
            })?;
        }

        if let Some(taic) = &stream.tai_clock_info {
            taic.write_taic_box(v)?;
        }

        Ok(())
    })?;

    Ok(())
}

fn write_audio_sample_entry(v: &mut Vec<u8>, stream: &TrackConfiguration) -> Result<(), Error> {
    let s = stream.caps.structure(0).unwrap();
    let fourcc = match s.name().as_str() {
        "audio/mpeg" => b"mp4a",
        "audio/x-opus" => b"Opus",
        "audio/x-flac" => b"fLaC",
        "audio/x-alaw" => b"alaw",
        "audio/x-mulaw" => b"ulaw",
        "audio/x-adpcm" => {
            let layout = s.get::<&str>("layout").context("no ADPCM layout field")?;

            match layout {
                "g726" => b"ms\x00\x45",
                _ => unreachable!(),
            }
        }
        "audio/x-ac3" => b"ac-3",
        "audio/x-eac3" => b"ec-3",
        _ => unreachable!(),
    };

    let sample_size = match s.name().as_str() {
        "audio/x-adpcm" => {
            let bitrate = s.get::<i32>("bitrate").context("no ADPCM bitrate field")?;
            (bitrate / 8000) as u16
        }
        "audio/x-flac" => {
            let (streamheader, _headers) =
                parse_flac_stream_header(&stream.caps).context("FLAC streamheader")?;
            streamheader.stream_info.bits_per_sample as u16
        }
        _ => 16u16,
    };

    write_sample_entry_box(v, fourcc, move |v| {
        // Reserved
        v.extend([0u8; 2 * 4]);

        // Channel count
        let channels = u16::try_from(s.get::<i32>("channels").context("no channels")?)
            .context("too many channels")?;
        v.extend(channels.to_be_bytes());

        // Sample size
        v.extend(sample_size.to_be_bytes());

        // Pre-defined
        v.extend([0u8; 2]);

        // Reserved
        v.extend([0u8; 2]);

        // Sample rate
        let rate = u16::try_from(s.get::<i32>("rate").context("no rate")?).unwrap_or(0);
        v.extend((u32::from(rate) << 16).to_be_bytes());

        // Codec specific boxes
        match s.name().as_str() {
            "audio/mpeg" => {
                let codec_data = s
                    .get::<&gst::BufferRef>("codec_data")
                    .context("no codec_data")?;
                let map = codec_data
                    .map_readable()
                    .context("codec_data not mappable")?;
                if map.len() < 2 {
                    bail!("too small codec_data");
                }
                write_esds_aac(v, stream, &map)?;
            }
            "audio/x-opus" => {
                write_dops(v, &stream.caps)?;
            }
            "audio/x-flac" => {
                write_dfla(v, &stream.caps)?;
            }
            "audio/x-alaw" | "audio/x-mulaw" | "audio/x-adpcm" => {
                // Nothing to do here
            }
            "audio/x-ac3" => {
                assert!(!stream.codec_specific_boxes.is_empty());
                assert!(&stream.codec_specific_boxes[4..8] == b"dac3");
                v.extend_from_slice(&stream.codec_specific_boxes);
            }
            "audio/x-eac3" => {
                assert!(!stream.codec_specific_boxes.is_empty());
                assert!(&stream.codec_specific_boxes[4..8] == b"dec3");
                v.extend_from_slice(&stream.codec_specific_boxes);
            }
            _ => unreachable!(),
        }

        // If rate did not fit into 16 bits write a full `srat` box
        if rate == 0 {
            let rate = s.get::<i32>("rate").context("no rate")?;
            // FIXME: This is defined as full box?
            write_full_box(
                v,
                b"srat",
                FULL_BOX_VERSION_0,
                FULL_BOX_FLAGS_NONE,
                move |v| {
                    v.extend((rate as u32).to_be_bytes());
                    Ok(())
                },
            )?;
        }

        if stream.avg_bitrate.is_some() || stream.max_bitrate.is_some() {
            write_box(v, b"btrt", |v| {
                // Buffer size DB
                // TODO
                v.extend(0u32.to_be_bytes());

                // Maximum bitrate
                let max_bitrate = stream.max_bitrate.or(stream.avg_bitrate).unwrap();
                v.extend(max_bitrate.to_be_bytes());

                // Average bitrate
                let avg_bitrate = stream.avg_bitrate.or(stream.max_bitrate).unwrap();
                v.extend(avg_bitrate.to_be_bytes());

                Ok(())
            })?;
        }

        // TODO: chnl box for channel ordering? probably not needed for AAC

        Ok(())
    })?;

    Ok(())
}

fn with_flac_metadata<R>(
    caps: &gst::Caps,
    cb: impl FnOnce(&[u8], &[gst::glib::SendValue]) -> R,
) -> Result<R, Error> {
    let caps = caps.structure(0).unwrap();
    let header = caps.get::<gst::ArrayRef>("streamheader").unwrap();
    let (streaminfo, remainder) = header.as_ref().split_first().unwrap();
    let streaminfo = streaminfo.get::<&gst::BufferRef>().unwrap();
    let streaminfo = streaminfo.map_readable().unwrap();
    // 13 bytes for the Ogg/FLAC prefix and 38 for the streaminfo itself.
    match <&[_; 13 + 38]>::try_from(streaminfo.as_slice()) {
        Ok(i) if i.starts_with(b"\x7FFLAC\x01\x00") => Ok(cb(&i[13..], remainder)),
        Ok(_) | Err(_) => bail!("Unknown streamheader format"),
    }
}

fn write_dfla(v: &mut Vec<u8>, caps: &gst::Caps) -> Result<(), Error> {
    write_full_box(v, b"dfLa", 0, 0, move |v| {
        with_flac_metadata(caps, |streaminfo, remainder| {
            v.extend(streaminfo);
            for metadata in remainder {
                let metadata = metadata.get::<&gst::BufferRef>().unwrap();
                let metadata = metadata.map_readable().unwrap();
                v.extend(&metadata[..]);
            }
        })
    })
}

/// Offset between UNIX epoch and Jan 1 1601 epoch in seconds.
/// 1601 = UNIX + UNIX_1601_OFFSET.
const UNIX_1601_OFFSET: u64 = 11_644_473_600;

pub(crate) fn write_cstb(cfg: PresentationConfiguration, v: &mut Vec<u8>) -> Result<(), Error> {
    write_box(v, b"cstb", |v| {
        // entry count
        v.extend(1u32.to_be_bytes());

        // track id
        v.extend(0u32.to_be_bytes());

        // start UTC time in 100ns units since Jan 1 1601
        // This is the UTC time of the earliest stream, which has to be converted to
        // the correct epoch and scale.
        let start_utc_time = cfg
            .tracks
            .iter()
            .map(|s| s.earliest_pts)
            .min()
            .unwrap()
            .nseconds()
            / 100;
        let start_utc_time = start_utc_time + UNIX_1601_OFFSET * 10_000_000;
        v.extend(start_utc_time.to_be_bytes());

        Ok(())
    })
}

pub(crate) fn write_onvif_metabox(
    cfg: PresentationConfiguration,
    v: &mut Vec<u8>,
) -> Result<(), Error> {
    write_full_box(v, b"meta", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_full_box(v, b"hdlr", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            write_hdlr_box(v, b"null", b"MetadataHandler")
        })?;

        write_cstb(cfg, v)
    })?;
    Ok(())
}

/// Create AC-3 `dac3` box.
pub(crate) fn create_dac3(buffer: &gst::BufferRef) -> Result<Vec<u8>, Error> {
    use bitstream_io::{BitRead as _, BitWrite as _};

    let map = buffer
        .map_readable()
        .context("Mapping AC-3 buffer readable")?;
    let mut reader = bitstream_io::BitReader::endian(
        std::io::Cursor::new(map.as_slice()),
        bitstream_io::BigEndian,
    );

    let header = reader
        .parse::<ac3::Header>()
        .context("Parsing AC-3 header")?;

    let mut dac3 = Vec::with_capacity(11);
    let mut writer = bitstream_io::BitWriter::endian(&mut dac3, bitstream_io::BigEndian);
    writer
        .build(&ac3::Dac3 { header })
        .context("Writing dac3 box")?;

    Ok(dac3)
}

/// Create EAC-3 `dec3` box.
pub(crate) fn create_dec3(buffer: &gst::BufferRef) -> Result<Vec<u8>, Error> {
    use bitstream_io::{BitRead as _, BitWrite as _};

    let map = buffer
        .map_readable()
        .context("Mapping EAC-3 buffer readable")?;

    let mut slice = map.as_slice();
    let mut headers = Vec::new();

    while !slice.is_empty() {
        let mut reader =
            bitstream_io::BitReader::endian(std::io::Cursor::new(slice), bitstream_io::BigEndian);
        let header = reader
            .parse::<eac3::Header>()
            .context("Parsing EAC-3 header")?;

        let framesize = (header.bsi.frmsiz as usize + 1) * 2;
        if slice.len() < framesize {
            bail!("Incomplete EAC-3 frame");
        }

        headers.push(header);

        slice = &slice[framesize..];
    }

    let mut dec3 = Vec::new();
    let mut writer = bitstream_io::BitWriter::endian(&mut dec3, bitstream_io::BigEndian);
    writer
        .build(&eac3::Dec3 { headers })
        .context("Writing dec3 box")?;

    Ok(dec3)
}
