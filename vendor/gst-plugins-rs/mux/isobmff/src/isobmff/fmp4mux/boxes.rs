// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeSet;

use crate::isobmff::{
    boxes::{
        create_moov, write_box, write_full_box, FULL_BOX_FLAGS_NONE, FULL_BOX_VERSION_0,
        FULL_BOX_VERSION_1,
    },
    caps_to_timescale, Buffer, FragmentHeaderConfiguration, FragmentHeaderStream, FragmentOffset,
    PresentationConfiguration, Variant,
};
use anyhow::{anyhow, Context, Error};
use gst::prelude::*;

fn cmaf_brands_from_caps(caps: &gst::CapsRef, compatible_brands: &mut BTreeSet<[u8; 4]>) {
    let s = caps.structure(0).unwrap();
    match s.name().as_str() {
        "video/x-h264" => {
            let width = s.get::<i32>("width").ok();
            let height = s.get::<i32>("height").ok();
            let fps = s.get::<gst::Fraction>("framerate").ok();
            let profile = s.get::<&str>("profile").ok();
            let level = s
                .get::<&str>("level")
                .ok()
                .map(|l| l.split_once('.').unwrap_or((l, "0")));
            let colorimetry = s.get::<&str>("colorimetry").ok();

            if let (Some(width), Some(height), Some(profile), Some(level), Some(fps)) =
                (width, height, profile, level, fps)
            {
                if profile == "high"
                    || profile == "main"
                    || profile == "baseline"
                    || profile == "constrained-baseline"
                {
                    if width <= 864
                        && height <= 576
                        && level <= ("3", "1")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                                    | gst_video::VideoColorPrimaries::Bt470bg
                                    | gst_video::VideoColorPrimaries::Smpte170m
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                                    | gst_video::VideoTransferFunction::Bt601
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                                    | gst_video::VideoColorMatrix::Bt601
                            ) {
                                compatible_brands.insert(*b"cfsd");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"cfsd");
                        }
                    } else if width <= 1920
                        && height <= 1080
                        && level <= ("4", "0")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                            ) {
                                compatible_brands.insert(*b"cfhd");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"cfhd");
                        }
                    } else if width <= 1920
                        && height <= 1080
                        && level <= ("4", "2")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                            ) {
                                compatible_brands.insert(*b"chdf");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"chdf");
                        }
                    }
                }
            }
        }
        "audio/mpeg" => {
            compatible_brands.insert(*b"caac");
        }
        "audio/x-eac3" => {
            compatible_brands.insert(*b"ceac");
        }
        "video/x-av1" => {
            compatible_brands.insert(*b"cmf2");
        }
        "video/x-h265" => {
            let width = s.get::<i32>("width").ok();
            let height = s.get::<i32>("height").ok();
            let fps = s.get::<gst::Fraction>("framerate").ok();
            let profile = s.get::<&str>("profile").ok();
            let tier = s.get::<&str>("tier").ok();
            let level = s
                .get::<&str>("level")
                .ok()
                .map(|l| l.split_once('.').unwrap_or((l, "0")));
            let colorimetry = s.get::<&str>("colorimetry").ok();

            if let (Some(width), Some(height), Some(profile), Some(tier), Some(level), Some(fps)) =
                (width, height, profile, tier, level, fps)
            {
                if profile == "main" && tier == "main" {
                    if width <= 1920
                        && height <= 1080
                        && level <= ("4", "1")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                            ) {
                                compatible_brands.insert(*b"chhd");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"chhd");
                        }
                    } else if width <= 3840
                        && height <= 2160
                        && level <= ("5", "0")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                            ) {
                                compatible_brands.insert(*b"cud8");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"cud8");
                        }
                    }
                } else if profile == "main-10" && tier == "main-10" {
                    if width <= 1920
                        && height <= 1080
                        && level <= ("4", "1")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                            ) {
                                compatible_brands.insert(*b"chh1");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"chh1");
                        }
                    } else if width <= 3840
                        && height <= 2160
                        && level <= ("5", "1")
                        && fps <= gst::Fraction::new(60, 1)
                    {
                        if let Some(colorimetry) =
                            colorimetry.and_then(|c| c.parse::<gst_video::VideoColorimetry>().ok())
                        {
                            if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt709
                                    | gst_video::VideoColorPrimaries::Bt2020
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Bt709
                                    | gst_video::VideoTransferFunction::Bt202010
                                    | gst_video::VideoTransferFunction::Bt202012
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt709
                                    | gst_video::VideoColorMatrix::Bt2020
                            ) {
                                compatible_brands.insert(*b"cud1");
                            } else if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt2020
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::Smpte2084
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt2020
                            ) {
                                compatible_brands.insert(*b"chd1");
                            } else if matches!(
                                colorimetry.primaries(),
                                gst_video::VideoColorPrimaries::Bt2020
                            ) && matches!(
                                colorimetry.transfer(),
                                gst_video::VideoTransferFunction::AribStdB67
                            ) && matches!(
                                colorimetry.matrix(),
                                gst_video::VideoColorMatrix::Bt2020
                            ) {
                                compatible_brands.insert(*b"clg1");
                            }
                        } else {
                            // Assume it's OK
                            compatible_brands.insert(*b"cud1");
                        }
                    }
                }
            }
        }
        _ => (),
    }
}

fn brands_from_variant_and_caps<'a>(
    variant: Variant,
    caps: impl Iterator<Item = &'a gst::Caps> + Clone,
) -> (u32, [u8; 4], Vec<[u8; 4]>) {
    let mut major_brand = *b"iso6";
    let mut minor_version = 0u32;
    let mut compatible_brands = BTreeSet::new();

    match variant {
        Variant::FragmentedISO | Variant::FragmentedONVIF => {}
        Variant::DASH => {
            major_brand = *b"msdh";

            // FIXME: `dsms` / `dash` brands, `msix`
            compatible_brands.insert(*b"dums");
            compatible_brands.insert(*b"msdh");
            compatible_brands.insert(*b"iso6");
        }
        Variant::CMAF => {
            major_brand = *b"cmf2";
            compatible_brands.insert(*b"iso6");
            compatible_brands.insert(*b"cmfc");

            let mut caps = caps.clone();
            cmaf_brands_from_caps(caps.next().unwrap(), &mut compatible_brands);
            assert_eq!(caps.next(), None);
        }
        Variant::ISO => todo!(),
        Variant::ONVIF => todo!(),
    }

    for caps in caps {
        let caps_structure = caps.structure(0).unwrap();
        match caps_structure.name().as_str() {
            "video/x-av1" => {
                minor_version = 1;
                compatible_brands.insert(*b"av01");
            }
            "video/x-h264" => {
                compatible_brands.insert(*b"avc1");
            }
            "audio/x-ac3" | "audio/x-eac3" => {
                compatible_brands.insert(*b"dby1");
            }
            "audio/x-opus" => {
                compatible_brands.insert(*b"opus");
            }
            _ => {}
        }
    }
    compatible_brands.insert(major_brand);

    (
        minor_version,
        major_brand,
        compatible_brands.into_iter().collect(),
    )
}

pub(crate) fn create_fmp4_header(cfg: PresentationConfiguration) -> Result<gst::Buffer, Error> {
    let (minor_version, major_brand, compatible_brands) =
        brands_from_variant_and_caps(cfg.variant, cfg.tracks.iter().map(|s| &s.caps));

    create_moov(
        cfg,
        minor_version,
        Some(major_brand),
        Some(compatible_brands),
    )
}

pub(crate) fn write_mvex(v: &mut Vec<u8>, cfg: &PresentationConfiguration) -> Result<(), Error> {
    if cfg.write_mehd {
        if cfg.update && cfg.duration.is_some() {
            write_full_box(v, b"mehd", FULL_BOX_VERSION_1, FULL_BOX_FLAGS_NONE, |v| {
                write_mehd(v, cfg)
            })?;
        } else {
            write_box(v, b"free", |v| {
                // version/flags of full box
                v.extend(0u32.to_be_bytes());
                // mehd duration
                v.extend(0u64.to_be_bytes());

                Ok(())
            })?;
        }
    }

    for (idx, _stream) in cfg.tracks.iter().enumerate() {
        write_full_box(v, b"trex", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            write_trex(v, idx)
        })?;
    }

    Ok(())
}

fn write_trex(v: &mut Vec<u8>, idx: usize) -> Result<(), Error> {
    // Track ID
    v.extend((idx as u32 + 1).to_be_bytes());

    // Default sample description index
    v.extend(1u32.to_be_bytes());

    // Default sample duration
    v.extend(0u32.to_be_bytes());

    // Default sample size
    v.extend(0u32.to_be_bytes());

    // Default sample flags
    v.extend(0u32.to_be_bytes());

    // Default sample duration/size/etc will be provided in the traf/trun if one can be determined
    // for a whole fragment

    Ok(())
}

fn write_mehd(v: &mut Vec<u8>, cfg: &PresentationConfiguration) -> Result<(), Error> {
    // Use the reference track timescale
    let timescale = cfg.to_timescale();

    let duration = cfg
        .duration
        .expect("no duration")
        .mul_div_ceil(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .context("too long duration")?;

    // Media duration in mvhd.timescale units
    v.extend(duration.to_be_bytes());

    Ok(())
}

/// Creates `styp` and `moof` boxes and `mdat` header
pub(crate) fn create_fmp4_fragment_header(
    cfg: FragmentHeaderConfiguration,
) -> Result<(gst::Buffer, u64), Error> {
    let mut v = vec![];

    // Don't write a `styp` if this is only a chunk unless it's the last.
    if !cfg.chunk || cfg.last_fragment {
        let (minor_version, major_brand, mut compatible_brands) =
            brands_from_variant_and_caps(cfg.variant, cfg.streams.iter().map(|s| &s.caps));

        if cfg.last_fragment {
            compatible_brands.push(*b"lmsg");
        }

        write_box(&mut v, b"styp", |v| {
            // major brand
            v.extend(major_brand);
            // minor version
            v.extend(minor_version.to_be_bytes());
            // compatible brands
            v.extend(compatible_brands.into_iter().flatten());

            Ok(())
        })?;
    }

    // Write prft for the first stream if we can
    if let Some(stream) = cfg.streams.first() {
        if let Some((start_time, start_ntp_time)) =
            Option::zip(stream.start_time, stream.start_ntp_time)
        {
            write_full_box(&mut v, b"prft", FULL_BOX_VERSION_1, 8, |v| {
                write_prft(v, &cfg, 0, stream, start_time, start_ntp_time)
            })?;
        }
    }

    let moof_pos = v.len();

    let data_offset_offsets = write_box(&mut v, b"moof", |v| write_moof(v, &cfg))?;

    let size = cfg
        .buffers
        .iter()
        .map(|buffer| buffer.buffer.size() as u64)
        .sum::<u64>();
    if let Ok(size) = u32::try_from(size + 8) {
        v.extend(size.to_be_bytes());
        v.extend(b"mdat");
    } else {
        v.extend(1u32.to_be_bytes());
        v.extend(b"mdat");
        v.extend((size + 16).to_be_bytes());
    }

    let data_offset = v.len() - moof_pos;
    for data_offset_offset in data_offset_offsets {
        let val = u32::from_be_bytes(v[data_offset_offset..][..4].try_into()?)
            .checked_add(u32::try_from(data_offset)?)
            .ok_or_else(|| anyhow!("can't calculate track run data offset"))?;
        v[data_offset_offset..][..4].copy_from_slice(&val.to_be_bytes());
    }

    Ok((gst::Buffer::from_mut_slice(v), moof_pos as u64))
}

fn write_prft(
    v: &mut Vec<u8>,
    _cfg: &FragmentHeaderConfiguration,
    idx: usize,
    stream: &FragmentHeaderStream,
    start_time: gst::ClockTime,
    start_ntp_time: gst::ClockTime,
) -> Result<(), Error> {
    // Reference track ID
    v.extend((idx as u32 + 1).to_be_bytes());
    // NTP timestamp
    let start_ntp_time = start_ntp_time
        .nseconds()
        .mul_div_floor(1u64 << 32, gst::ClockTime::SECOND.nseconds())
        .unwrap();
    v.extend(start_ntp_time.to_be_bytes());
    // Media time
    let timescale = stream.to_timescale();
    let media_time = start_time
        .mul_div_floor(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .unwrap();
    v.extend(media_time.to_be_bytes());

    Ok(())
}

fn write_moof(v: &mut Vec<u8>, cfg: &FragmentHeaderConfiguration) -> Result<Vec<usize>, Error> {
    write_full_box(v, b"mfhd", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
        write_mfhd(v, cfg)
    })?;

    let mut data_offset_offsets = vec![];
    for (idx, stream) in cfg.streams.iter().enumerate() {
        // Skip tracks without any buffers for this fragment.
        if stream.start_time.is_none() {
            continue;
        }

        write_box(v, b"traf", |v| {
            write_traf(v, cfg, &mut data_offset_offsets, idx, stream)
        })?;
    }

    Ok(data_offset_offsets)
}

fn write_mfhd(v: &mut Vec<u8>, cfg: &FragmentHeaderConfiguration) -> Result<(), Error> {
    v.extend(cfg.sequence_number.to_be_bytes());

    Ok(())
}

const DEFAULT_SAMPLE_DURATION_PRESENT: u32 = 0x08;
const DEFAULT_SAMPLE_SIZE_PRESENT: u32 = 0x10;
const DEFAULT_SAMPLE_FLAGS_PRESENT: u32 = 0x20;
const DEFAULT_BASE_IS_MOOF: u32 = 0x2_00_00;

const DATA_OFFSET_PRESENT: u32 = 0x0_01;
const FIRST_SAMPLE_FLAGS_PRESENT: u32 = 0x0_04;
const SAMPLE_DURATION_PRESENT: u32 = 0x1_00;
const SAMPLE_SIZE_PRESENT: u32 = 0x2_00;
const SAMPLE_FLAGS_PRESENT: u32 = 0x4_00;
const SAMPLE_COMPOSITION_TIME_OFFSET_PRESENT: u32 = 0x8_00;

#[allow(clippy::type_complexity)]
fn analyze_buffers(
    cfg: &FragmentHeaderConfiguration,
    idx: usize,
    stream: &FragmentHeaderStream,
    timescale: u32,
) -> Result<
    (
        // tf_flags
        u32,
        // tr_flags
        u32,
        // default size
        Option<u32>,
        // default duration
        Option<u32>,
        // default flags
        Option<u32>,
        // negative composition time offsets
        bool,
    ),
    Error,
> {
    let mut tf_flags = DEFAULT_BASE_IS_MOOF;
    let mut tr_flags = DATA_OFFSET_PRESENT;

    let mut duration = None;
    let mut size = None;
    let mut first_buffer_flags = None;
    let mut flags = None;

    let mut negative_composition_time_offsets = false;

    for Buffer {
        idx: _idx,
        buffer,
        timestamp: _timestamp,
        duration: sample_duration,
        composition_time_offset,
    } in cfg.buffers.iter().filter(|b| b.idx == idx)
    {
        if size.is_none() {
            size = Some(buffer.size() as u32);
        }
        if Some(buffer.size() as u32) != size {
            tr_flags |= SAMPLE_SIZE_PRESENT;
        }

        let sample_duration = u32::try_from(
            sample_duration
                .nseconds()
                .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
                .context("too big sample duration")?,
        )
        .context("too big sample duration")?;

        if duration.is_none() {
            duration = Some(sample_duration);
        }
        if Some(sample_duration) != duration {
            tr_flags |= SAMPLE_DURATION_PRESENT;
        }

        let f = sample_flags_from_buffer(stream, buffer);
        if first_buffer_flags.is_none() {
            // First buffer, remember as first buffer flags
            first_buffer_flags = Some(f);
        } else if flags.is_none() {
            // Second buffer, remember as general flags and if they're
            // different from the first buffer's flags then also remember
            // that
            flags = Some(f);
            if Some(f) != first_buffer_flags {
                tr_flags |= FIRST_SAMPLE_FLAGS_PRESENT;
            }
        } else if Some(f) != flags {
            // Third or later buffer, and the flags are different than the second buffer's flags.
            // In that case each sample will have to store its own flags.
            tr_flags &= !FIRST_SAMPLE_FLAGS_PRESENT;
            tr_flags |= SAMPLE_FLAGS_PRESENT;
        }

        if let Some(composition_time_offset) = *composition_time_offset {
            assert!(stream.delta_frames.requires_dts());
            if composition_time_offset != 0 {
                tr_flags |= SAMPLE_COMPOSITION_TIME_OFFSET_PRESENT;
            }
            if composition_time_offset < 0 {
                negative_composition_time_offsets = true;
            }
        }
    }

    if (tr_flags & SAMPLE_SIZE_PRESENT) == 0 {
        tf_flags |= DEFAULT_SAMPLE_SIZE_PRESENT;
    } else {
        size = None;
    }

    if (tr_flags & SAMPLE_DURATION_PRESENT) == 0 {
        tf_flags |= DEFAULT_SAMPLE_DURATION_PRESENT;
    } else {
        duration = None;
    }

    // If there is only a single buffer use its flags as default sample flags
    // instead of first sample flags.
    if flags.is_none() && first_buffer_flags.is_some() {
        tr_flags &= !FIRST_SAMPLE_FLAGS_PRESENT;
        flags = first_buffer_flags.take();
    }

    // If all but possibly the first buffer had the same flags then only store them once instead of
    // with every single sample.
    if (tr_flags & SAMPLE_FLAGS_PRESENT) == 0 {
        tf_flags |= DEFAULT_SAMPLE_FLAGS_PRESENT;
    } else {
        flags = None;
    }

    Ok((
        tf_flags,
        tr_flags,
        size,
        duration,
        flags,
        negative_composition_time_offsets,
    ))
}

#[allow(clippy::ptr_arg)]
fn write_traf(
    v: &mut Vec<u8>,
    cfg: &FragmentHeaderConfiguration,
    data_offset_offsets: &mut Vec<usize>,
    idx: usize,
    stream: &FragmentHeaderStream,
) -> Result<(), Error> {
    let timescale = stream.to_timescale();

    // Analyze all buffers to know what values can be put into the tfhd for all samples and what
    // has to be stored for every single sample
    let (
        tf_flags,
        mut tr_flags,
        default_size,
        default_duration,
        default_flags,
        negative_composition_time_offsets,
    ) = analyze_buffers(cfg, idx, stream, timescale)?;

    assert!((tf_flags & DEFAULT_SAMPLE_SIZE_PRESENT == 0) ^ default_size.is_some());
    assert!((tf_flags & DEFAULT_SAMPLE_DURATION_PRESENT == 0) ^ default_duration.is_some());
    assert!((tf_flags & DEFAULT_SAMPLE_FLAGS_PRESENT == 0) ^ default_flags.is_some());

    write_full_box(v, b"tfhd", FULL_BOX_VERSION_0, tf_flags, |v| {
        write_tfhd(v, cfg, idx, default_size, default_duration, default_flags)
    })?;

    let large_tfdt = stream
        .start_time
        .unwrap()
        .mul_div_floor(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .context("base time overflow")?
        .nseconds()
        > u32::MAX as u64;
    write_full_box(
        v,
        b"tfdt",
        if large_tfdt {
            FULL_BOX_VERSION_1
        } else {
            FULL_BOX_VERSION_0
        },
        FULL_BOX_FLAGS_NONE,
        |v| write_tfdt(v, cfg, idx, stream, timescale),
    )?;

    let mut current_data_offset = 0;

    for run in cfg
        .buffers
        .chunk_by(|a: &Buffer, b: &Buffer| a.idx == b.idx)
    {
        if run[0].idx != idx {
            // FIXME: What to do with >4GB offsets?
            current_data_offset = (current_data_offset as u64
                + run.iter().map(|b| b.buffer.size() as u64).sum::<u64>())
            .try_into()?;
            continue;
        }

        let data_offset_offset = write_full_box(
            v,
            b"trun",
            if negative_composition_time_offsets {
                FULL_BOX_VERSION_1
            } else {
                FULL_BOX_VERSION_0
            },
            tr_flags,
            |v| {
                write_trun(
                    v,
                    cfg,
                    current_data_offset,
                    tr_flags,
                    timescale,
                    stream,
                    run,
                )
            },
        )?;
        data_offset_offsets.push(data_offset_offset);

        // FIXME: What to do with >4GB offsets?
        current_data_offset = (current_data_offset as u64
            + run.iter().map(|b| b.buffer.size() as u64).sum::<u64>())
        .try_into()?;

        // Don't include first sample flags in any trun boxes except for the first
        tr_flags &= !FIRST_SAMPLE_FLAGS_PRESENT;
    }

    // TODO: saio, saiz, sbgp, sgpd, subs?

    Ok(())
}

fn write_tfhd(
    v: &mut Vec<u8>,
    _cfg: &FragmentHeaderConfiguration,
    idx: usize,
    default_size: Option<u32>,
    default_duration: Option<u32>,
    default_flags: Option<u32>,
) -> Result<(), Error> {
    // Track ID
    v.extend((idx as u32 + 1).to_be_bytes());

    // No base data offset, no sample description index

    if let Some(default_duration) = default_duration {
        v.extend(default_duration.to_be_bytes());
    }

    if let Some(default_size) = default_size {
        v.extend(default_size.to_be_bytes());
    }

    if let Some(default_flags) = default_flags {
        v.extend(default_flags.to_be_bytes());
    }

    Ok(())
}

fn write_tfdt(
    v: &mut Vec<u8>,
    _cfg: &FragmentHeaderConfiguration,
    _idx: usize,
    stream: &FragmentHeaderStream,
    timescale: u32,
) -> Result<(), Error> {
    let base_time = stream
        .start_time
        .unwrap()
        .mul_div_floor(timescale as u64, gst::ClockTime::SECOND.nseconds())
        .context("base time overflow")?
        .nseconds();

    if base_time > u32::MAX as u64 {
        v.extend(base_time.to_be_bytes());
    } else {
        v.extend((base_time as u32).to_be_bytes());
    }

    Ok(())
}

#[allow(clippy::identity_op)]
#[allow(clippy::bool_to_int_with_if)]
fn sample_flags_from_buffer(stream: &FragmentHeaderStream, buffer: &gst::BufferRef) -> u32 {
    if stream.delta_frames.intra_only() {
        (0b00u32 << (16 + 10)) | // leading: unknown
        (0b10u32 << (16 + 8)) | // depends: no
        (0b10u32 << (16 + 6)) | // depended: no
        (0b00u32 << (16 + 4)) | // redundancy: unknown
        (0b000u32 << (16 + 1)) | // padding: no
        (0b0u32 << 16) | // non-sync-sample: no
        (0u32) // degradation priority
    } else {
        let depends = if buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            0b01u32
        } else {
            0b10u32
        };
        let depended = if buffer.flags().contains(gst::BufferFlags::DROPPABLE) {
            0b10u32
        } else {
            0b00u32
        };
        let non_sync_sample = if buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            0b1u32
        } else {
            0b0u32
        };

        (0b00u32 << (16 + 10)) | // leading: unknown
        (depends << (16 + 8)) | // depends
        (depended << (16 + 6)) | // depended
        (0b00u32 << (16 + 4)) | // redundancy: unknown
        (0b000u32 << (16 + 1)) | // padding: no
        (non_sync_sample << 16) | // non-sync-sample
        (0u32) // degradation priority
    }
}

#[allow(clippy::too_many_arguments)]
fn write_trun(
    v: &mut Vec<u8>,
    _cfg: &FragmentHeaderConfiguration,
    current_data_offset: u32,
    tr_flags: u32,
    timescale: u32,
    stream: &FragmentHeaderStream,
    buffers: &[Buffer],
) -> Result<usize, Error> {
    // Sample count
    v.extend((buffers.len() as u32).to_be_bytes());

    let data_offset_offset = v.len();
    // Data offset, will be rewritten later
    v.extend(current_data_offset.to_be_bytes());

    if (tr_flags & FIRST_SAMPLE_FLAGS_PRESENT) != 0 {
        v.extend(sample_flags_from_buffer(stream, &buffers[0].buffer).to_be_bytes());
    }

    for Buffer {
        idx: _idx,
        ref buffer,
        timestamp: _timestamp,
        duration,
        composition_time_offset,
    } in buffers.iter()
    {
        if (tr_flags & SAMPLE_DURATION_PRESENT) != 0 {
            // Sample duration
            let sample_duration = u32::try_from(
                duration
                    .nseconds()
                    .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
                    .context("too big sample duration")?,
            )
            .context("too big sample duration")?;
            v.extend(sample_duration.to_be_bytes());
        }

        if (tr_flags & SAMPLE_SIZE_PRESENT) != 0 {
            // Sample size
            v.extend((buffer.size() as u32).to_be_bytes());
        }

        if (tr_flags & SAMPLE_FLAGS_PRESENT) != 0 {
            assert!((tr_flags & FIRST_SAMPLE_FLAGS_PRESENT) == 0);

            // Sample flags
            v.extend(sample_flags_from_buffer(stream, buffer).to_be_bytes());
        }

        if (tr_flags & SAMPLE_COMPOSITION_TIME_OFFSET_PRESENT) != 0 {
            // Sample composition time offset
            let composition_time_offset = i32::try_from(
                composition_time_offset
                    .unwrap_or(0)
                    .mul_div_round(timescale as i64, gst::ClockTime::SECOND.nseconds() as i64)
                    .context("too big composition time offset")?,
            )
            .context("too big composition time offset")?;
            v.extend(composition_time_offset.to_be_bytes());
        }
    }

    Ok(data_offset_offset)
}

/// Creates `mfra` box
pub(crate) fn create_mfra(
    caps: &gst::CapsRef,
    fragment_offsets: &[FragmentOffset],
) -> Result<gst::Buffer, Error> {
    let timescale = caps_to_timescale(caps);

    let mut v = vec![];

    let offset = write_box(&mut v, b"mfra", |v| {
        write_full_box(v, b"tfra", FULL_BOX_VERSION_1, FULL_BOX_FLAGS_NONE, |v| {
            // Track ID
            v.extend(1u32.to_be_bytes());

            // Reserved / length of traf/trun/sample
            v.extend(0u32.to_be_bytes());

            // Number of entries
            v.extend(
                u32::try_from(fragment_offsets.len())
                    .context("too many fragments")?
                    .to_be_bytes(),
            );

            for FragmentOffset { time, offset } in fragment_offsets {
                // Time
                let time = time
                    .nseconds()
                    .mul_div_round(timescale as u64, gst::ClockTime::SECOND.nseconds())
                    .context("time overflow")?;
                v.extend(time.to_be_bytes());

                // moof offset
                v.extend(offset.to_be_bytes());

                // traf/trun/sample number
                v.extend_from_slice(&[1u8; 3][..]);
            }

            Ok(())
        })?;

        let offset = write_full_box(v, b"mfro", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            let offset = v.len();
            // Parent size
            v.extend(0u32.to_be_bytes());
            Ok(offset)
        })?;

        Ok(offset)
    })?;

    let len = u32::try_from(v.len() as u64).context("too big mfra")?;
    v[offset..][..4].copy_from_slice(&len.to_be_bytes());

    Ok(gst::Buffer::from_mut_slice(v))
}
