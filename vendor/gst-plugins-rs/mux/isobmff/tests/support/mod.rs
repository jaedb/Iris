// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//

pub struct ExpectedConfiguration {
    pub is_audio: bool,
    pub width: u32,
    pub height: u32,
    pub has_ctts: bool,
    pub has_stss: bool,
    pub has_taic: bool,
    pub taic_time_uncertainty: u64,
    pub taic_clock_type: u8,
    pub num_tai_timestamps: u32,
    pub is_fragmented: bool,
    pub audio_channel_count: u16,
    pub audio_sample_rate: mp4_atom::FixedPoint<u16>,
    pub audio_sample_size: u16,
}

impl Default for ExpectedConfiguration {
    fn default() -> Self {
        Self {
            is_audio: false,
            width: 320,
            height: 240,
            has_ctts: false,
            has_stss: false,
            has_taic: false,
            taic_time_uncertainty: 0,
            taic_clock_type: 0,
            num_tai_timestamps: 0,
            is_fragmented: false,
            audio_channel_count: 0,
            audio_sample_rate: 0.into(),
            audio_sample_size: 0,
        }
    }
}
pub fn check_ftyp_output(
    expected_major_brand: mp4_atom::FourCC,
    expected_minor_version: u32,
    expected_compatible_brands: &Vec<mp4_atom::FourCC>,
    ftyp: mp4_atom::Ftyp,
) {
    assert_eq!(ftyp.major_brand, expected_major_brand);
    assert_eq!(ftyp.minor_version, expected_minor_version);
    assert_eq!(
        ftyp.compatible_brands.len(),
        expected_compatible_brands.len(),
        "Got {:?}, expected {:?}",
        ftyp.compatible_brands,
        expected_compatible_brands
    );
    for fourcc in expected_compatible_brands {
        assert!(ftyp.compatible_brands.contains(fourcc));
    }
}

pub fn check_mvhd_sanity(mvhd: &mp4_atom::Mvhd, expected_config: &ExpectedConfiguration) {
    assert!(mvhd.creation_time > 0);
    assert!(mvhd.modification_time > 0);
    assert!(mvhd.next_track_id > 1);
    if expected_config.is_fragmented {
        assert_eq!(mvhd.duration, 0);
    } else {
        assert!(mvhd.duration > 0);
    }
    if expected_config.is_audio {
        assert!(mvhd.volume.integer() > 0);
    } else {
        assert_eq!(mvhd.volume.integer(), 1);
        assert_eq!(mvhd.volume.decimal(), 0);
    }
    // TODO: assess remaining values for potential invariant
}

pub fn check_trak_sanity(trak: &[mp4_atom::Trak], expected_config: &ExpectedConfiguration) {
    assert_eq!(trak.len(), 1);
    assert!(trak[0].meta.is_none());
    check_tkhd_sanity(&trak[0].tkhd, expected_config);
    if expected_config.is_fragmented {
        assert!(&trak[0].edts.is_none());
    } else {
        check_edts_sanity(&trak[0].edts);
    }
    check_mdia_sanity(&trak[0].mdia, expected_config);
}

fn check_edts_sanity(maybe_edts: &Option<mp4_atom::Edts>) {
    assert!(maybe_edts.as_ref().is_some_and(|edts| {
        assert!(edts.elst.is_some());
        let elst = edts.elst.as_ref().unwrap();
        assert!(!elst.entries.is_empty());
        true
    }));
}

fn check_tkhd_sanity(tkhd: &mp4_atom::Tkhd, expected_config: &ExpectedConfiguration) {
    assert!(tkhd.creation_time > 0);
    assert!(tkhd.modification_time > 0);
    assert!(tkhd.enabled);
    if expected_config.is_audio {
        assert_eq!(tkhd.width, 0.into());
        assert_eq!(tkhd.height, 0.into());
        assert!(tkhd.volume.integer() > 0);
    } else {
        assert_eq!(tkhd.width.integer(), expected_config.width as u16);
        assert_eq!(tkhd.height.integer(), expected_config.height as u16);
        assert_eq!(tkhd.volume.integer(), 0);
        assert_eq!(tkhd.volume.decimal(), 0);
    }
    // TODO: assess remaining values for potential invariant
}

fn check_mdia_sanity(mdia: &mp4_atom::Mdia, expected_config: &ExpectedConfiguration) {
    check_hdlr_sanity(&mdia.hdlr);
    check_mdhd_sanity(&mdia.mdhd);
    check_minf_sanity(&mdia.minf, expected_config);
}

fn check_hdlr_sanity(hdlr: &mp4_atom::Hdlr) {
    assert!(
        (hdlr.handler == b"soun".into())
            || (hdlr.handler == b"vide".into())
            || (hdlr.handler == b"pict".into())
    );
    assert!(!hdlr.name.is_empty());
}

fn check_mdhd_sanity(mdhd: &mp4_atom::Mdhd) {
    assert!(mdhd.creation_time > 0);
    assert!(mdhd.modification_time > 0);
    assert!(mdhd.timescale > 0);
    assert!(mdhd.language.len() == 3);
}

fn check_minf_sanity(minf: &mp4_atom::Minf, expected_config: &ExpectedConfiguration) {
    check_dinf_sanity(&minf.dinf);
    assert!(
        (minf.smhd.is_some() && (minf.vmhd.is_none())
            || (minf.smhd.is_none() || minf.vmhd.is_some()))
    );
    check_stbl_sanity(&minf.stbl, expected_config);
}

fn check_dinf_sanity(dinf: &mp4_atom::Dinf) {
    let dref = &dinf.dref;
    assert_eq!(dref.urls.len(), 1);
    let url = &dref.urls[0];
    assert!(url.location.is_empty());
}

fn check_stbl_sanity(stbl: &mp4_atom::Stbl, expected_config: &ExpectedConfiguration) {
    assert!(stbl.co64.is_none());
    if expected_config.has_ctts {
        assert!(stbl.ctts.is_some());
        // TODO:
        // check_ctts_sanity(&stbl.ctts);
    } else {
        assert!(stbl.ctts.is_none());
    }
    check_saio_sanity(&stbl.saio, expected_config);
    check_saiz_sanity(&stbl.saiz, expected_config);
    check_stco_sanity(&stbl.stco, expected_config);
    check_stsc_sanity(&stbl.stsc, expected_config);
    check_stsd_sanity(&stbl.stsd, expected_config);
    if expected_config.has_stss {
        assert!(stbl.stss.is_some());
        // TODO:
        // check_stss_sanity(&stbl.ctts);
    } else {
        assert!(stbl.stss.is_none());
    }
    check_stsz_sanity(&stbl.stsz, expected_config);
    check_stts_sanity(&stbl.stts, expected_config);
    // TODO: check consistency between sample sizes and chunk / sample offsets
}

fn check_saio_sanity(saios: &Vec<mp4_atom::Saio>, expected_config: &ExpectedConfiguration) {
    if expected_config.num_tai_timestamps != 0 {
        assert_eq!(saios.len(), 1);
        for saio in saios {
            assert!(saio.aux_info.is_some());
            assert_eq!(
                saio.aux_info.as_ref().unwrap().aux_info_type,
                b"stai".into()
            );
            assert_eq!(saio.aux_info.as_ref().unwrap().aux_info_type_parameter, 0);
            assert_eq!(
                saio.offsets.len(),
                expected_config.num_tai_timestamps as usize
            );
            let mut previous_offset = 0u64;
            for offset in &saio.offsets {
                // We check that the byte offsets are increasing
                // This is different to checking that the timestamps are increasing
                assert!(*offset > previous_offset);
                previous_offset = *offset;
            }
        }
    } else {
        assert!(saios.is_empty());
    }
}

fn check_saiz_sanity(saizs: &Vec<mp4_atom::Saiz>, expected_config: &ExpectedConfiguration) {
    if expected_config.num_tai_timestamps != 0 {
        assert_eq!(saizs.len(), 1);
        for saiz in saizs {
            assert!(saiz.aux_info.is_some());
            assert_eq!(
                saiz.aux_info.as_ref().unwrap().aux_info_type,
                b"stai".into()
            );
            assert_eq!(saiz.aux_info.as_ref().unwrap().aux_info_type_parameter, 0);
            assert_eq!(saiz.default_sample_info_size, 9);
            assert_eq!(saiz.sample_count, expected_config.num_tai_timestamps);
        }
    } else {
        assert!(saizs.is_empty());
    }
}

fn check_stco_sanity(maybe_stco: &Option<mp4_atom::Stco>, expected_config: &ExpectedConfiguration) {
    assert!(maybe_stco.as_ref().is_some_and(|stco| {
        if expected_config.is_fragmented {
            stco.entries.is_empty()
        } else {
            !stco.entries.is_empty()
        }
    }));
    // TODO: see if there is anything generic about the stco entries we could check
}

fn check_stsc_sanity(stsc: &mp4_atom::Stsc, expected_config: &ExpectedConfiguration) {
    if expected_config.is_fragmented {
        assert!(stsc.entries.is_empty());
    } else {
        assert!(!stsc.entries.is_empty());
    }
    // TODO: see if there is anything generic about the stsc entries we could check
}

fn check_stsd_sanity(stsd: &mp4_atom::Stsd, expected_config: &ExpectedConfiguration) {
    assert_eq!(stsd.codecs.len(), 1);
    let codec = &stsd.codecs[0];
    match codec {
        mp4_atom::Codec::Avc1(avc1) => {
            assert_eq!(avc1.visual.width, expected_config.width as u16);
            assert_eq!(avc1.visual.height, expected_config.height as u16);
            assert_eq!(avc1.visual.depth, 24);
            if expected_config.has_taic {
                assert!(avc1.taic.as_ref().is_some_and(|taic| {
                    assert_eq!(taic.clock_type, expected_config.taic_clock_type.into());
                    assert_eq!(taic.time_uncertainty, expected_config.taic_time_uncertainty);
                    assert_eq!(taic.clock_drift_rate, 2147483647);
                    assert_eq!(taic.clock_resolution, 1000);
                    true
                }));
            } else {
                assert!(avc1.taic.is_none());
            }

            assert!(avc1
                .pasp
                .as_ref()
                .is_some_and(|pasp| { pasp.h_spacing == 1 && pasp.v_spacing == 1 }));
            assert!(avc1.colr.as_ref().is_some_and(|colr| {
                match colr {
                    mp4_atom::Colr::Nclx {
                        colour_primaries,
                        transfer_characteristics,
                        matrix_coefficients: _,
                        full_range_flag,
                    } => {
                        assert_eq!(*colour_primaries, 6);
                        assert_eq!(*transfer_characteristics, 6);
                        assert!(!(*full_range_flag));
                        true
                    }
                    mp4_atom::Colr::Ricc { profile: _ } => {
                        panic!("Incorrect colr type: ricc")
                    }
                    mp4_atom::Colr::Prof { profile: _ } => {
                        panic!("Incorrect colr type: prof")
                    }
                }
            }));
        }
        mp4_atom::Codec::Hev1(_hev1) => {
            // TODO: check HEVC codec (maybe shared?)
        }
        mp4_atom::Codec::Hvc1(_hvc1) => {
            // TODO: check HEVC codec (maybe shared?)
        }
        mp4_atom::Codec::Vp08(_vp08) => {
            // TODO: check VP8 codec
        }
        mp4_atom::Codec::Vp09(_vp09) => {
            // TODO: check VP9 codec
        }
        mp4_atom::Codec::Av01(_av01) => {
            // TODO: check AV1 codec
        }
        mp4_atom::Codec::Mp4a(_mp4a) => {
            // TODO: check MPEG Audio codec
        }
        mp4_atom::Codec::Tx3g(_tx3g) => {
            // TODO: check subtitles / text codec
        }
        mp4_atom::Codec::Opus(_opus) => {
            // TODO: check OPUS codec
        }
        mp4_atom::Codec::Flac(flac) => {
            check_flac_codec_sanity(flac, expected_config);
        }
        mp4_atom::Codec::Uncv(uncv) => {
            check_uncv_codec_sanity(uncv, expected_config);
        }
        mp4_atom::Codec::Ac3(ac3) => {
            check_ac3_codec_sanity(ac3, expected_config);
        }
        mp4_atom::Codec::Eac3(eac3) => {
            check_eac3_codec_sanity(eac3, expected_config);
        }
        mp4_atom::Codec::Unknown(four_cc) => {
            todo!("Unsupported codec type: {:?}", four_cc);
        }
    }
}

fn check_flac_codec_sanity(flac: &mp4_atom::Flac, expected_config: &ExpectedConfiguration) {
    check_audio_sample_entry_sanity(&flac.audio, expected_config);
    check_flac_sample_entry_sanity(&flac.dfla, expected_config);
}

fn check_ac3_codec_sanity(ac3: &mp4_atom::Ac3, expected_config: &ExpectedConfiguration) {
    check_audio_sample_entry_sanity(&ac3.audio, expected_config);
    check_ac3_sample_entry_sanity(&ac3.dac3, expected_config);
}

fn check_eac3_codec_sanity(eac3: &mp4_atom::Eac3, expected_config: &ExpectedConfiguration) {
    check_audio_sample_entry_sanity(&eac3.audio, expected_config);
    check_eac3_sample_entry_sanity(&eac3.dec3, expected_config);
}

fn check_audio_sample_entry_sanity(
    audio: &mp4_atom::Audio,
    expected_config: &ExpectedConfiguration,
) {
    assert_eq!(audio.channel_count, expected_config.audio_channel_count);
    assert_eq!(audio.data_reference_index, 1);
    assert_eq!(audio.sample_rate, expected_config.audio_sample_rate);
    assert_eq!(audio.sample_size, expected_config.audio_sample_size);
}

fn check_flac_sample_entry_sanity(dfla: &mp4_atom::Dfla, expected_config: &ExpectedConfiguration) {
    assert!(!dfla.blocks.is_empty());
    match dfla.blocks.first().unwrap() {
        mp4_atom::FlacMetadataBlock::StreamInfo {
            minimum_block_size,
            maximum_block_size: _,
            minimum_frame_size: _,
            maximum_frame_size: _,
            sample_rate: _,
            num_channels_minus_one,
            bits_per_sample_minus_one,
            number_of_interchannel_samples: _,
            md5_checksum,
        } => {
            // TODO: more checks
            assert!(*minimum_block_size >= 16);
            assert_eq!(
                (num_channels_minus_one + 1) as u16,
                expected_config.audio_channel_count
            );
            assert_eq!(
                (bits_per_sample_minus_one + 1) as u16,
                expected_config.audio_sample_size
            );
            assert_eq!(md5_checksum.len(), 16);
        }
        _ => {
            panic!("The first dfLa metadata block must be Streaminfo");
        }
    }
    // TODO: check Streaminfo
    for i in 1..dfla.blocks.len() {
        match dfla.blocks.get(i).unwrap() {
            mp4_atom::FlacMetadataBlock::StreamInfo {
                minimum_block_size: _,
                maximum_block_size: _,
                minimum_frame_size: _,
                maximum_frame_size: _,
                sample_rate: _,
                num_channels_minus_one: _,
                bits_per_sample_minus_one: _,
                number_of_interchannel_samples: _,
                md5_checksum: _,
            } => {
                panic!("Streaminfo must be the first metadata block, and cannot re-occur.");
            }
            mp4_atom::FlacMetadataBlock::Padding => todo!(),
            mp4_atom::FlacMetadataBlock::Application => todo!(),
            mp4_atom::FlacMetadataBlock::SeekTable => todo!(),
            mp4_atom::FlacMetadataBlock::VorbisComment {
                vendor_string,
                comments,
            } => {
                assert!(!vendor_string.is_empty());
                for comment in comments {
                    assert!(!comment.is_empty());
                }
            }
            mp4_atom::FlacMetadataBlock::CueSheet => todo!(),
            mp4_atom::FlacMetadataBlock::Picture => todo!(),
            mp4_atom::FlacMetadataBlock::Reserved => todo!(),
            mp4_atom::FlacMetadataBlock::Forbidden => todo!(),
        }
    }
}

fn check_ac3_sample_entry_sanity(
    dac3: &mp4_atom::Ac3SpecificBox,
    _expected_config: &ExpectedConfiguration,
) {
    assert_eq!(
        *dac3,
        mp4_atom::Ac3SpecificBox {
            fscod: 1,
            bsid: 8,
            bsmod: 0,
            acmod: 2,
            lfeon: false,
            bit_rate_code: 10
        }
    );
}

fn check_eac3_sample_entry_sanity(
    dec3: &mp4_atom::Ec3SpecificBox,
    _expected_config: &ExpectedConfiguration,
) {
    assert_eq!(dec3.data_rate, 191);
    assert_eq!(dec3.substreams.len(), 1);
    assert_eq!(
        *(dec3.substreams.first().unwrap()),
        mp4_atom::Ec3IndependentSubstream {
            fscod: 1,
            bsid: 16,
            asvc: false,
            bsmod: 0,
            acmod: 2,
            lfeon: false,
            num_dep_sub: 0,
            chan_loc: None
        }
    );
}

fn check_uncv_codec_sanity(uncv: &mp4_atom::Uncv, expected_config: &ExpectedConfiguration) {
    check_visual_sample_entry_sanity(&uncv.visual, expected_config);
    // See ISO/IEC 23001-17 Table 5 for the profiles
    let valid_v0_profiles: Vec<mp4_atom::FourCC> = vec![
        b"2vuy".into(),
        b"yuv2".into(),
        b"yvyu".into(),
        b"vyuy".into(),
        b"yuv1".into(),
        b"v308".into(),
        b"v408".into(),
        b"y210".into(),
        b"v410".into(),
        b"v210".into(),
        b"rgb3".into(),
        b"i420".into(),
        b"nv12".into(),
        b"nv21".into(),
        b"rgba".into(),
        b"abgr".into(),
        b"yu22".into(),
        b"yv22".into(),
        b"yv20".into(),
        b"\0\0\0\0".into(),
    ];
    let valid_v1_profiles: Vec<mp4_atom::FourCC> =
        vec![b"rgb3".into(), b"rgba".into(), b"abgr".into()];
    let uncc = &uncv.uncc;
    match uncc {
        mp4_atom::UncC::V1 { profile } => {
            assert!(uncv.cmpd.is_none());
            assert!(valid_v1_profiles.contains(profile));
        }
        mp4_atom::UncC::V0 {
            profile,
            components,
            sampling_type,
            interleave_type,
            block_size: _,
            components_little_endian: _,
            block_pad_lsb: _,
            block_little_endian: _,
            block_reversed: _,
            pad_unknown: _,
            pixel_size: _,
            row_align_size: _,
            tile_align_size: _,
            num_tile_cols_minus_one: _,
            num_tile_rows_minus_one: _,
        } => {
            assert!(uncv.cmpd.is_some());
            let cmpd = uncv.cmpd.as_ref().unwrap();
            println!("profile: {profile:?}");
            assert!(valid_v0_profiles.contains(profile));
            assert!(!components.is_empty());
            for component in components {
                // Not clear if component.component_align_size could be tested
                // Not clear if component.component_bit_depth_minus_one coudl be tested
                assert!(component.component_index < (cmpd.components.len() as u16));
                assert!(component.component_format < 4); // 3 for signed int is in Amd 2.
            }
            assert!(*sampling_type < 4);
            assert!(*interleave_type < 6);
            // TODO: there are some special cases we could cross check here.
        }
    }
}

fn check_visual_sample_entry_sanity(
    visual: &mp4_atom::Visual,
    expected_config: &ExpectedConfiguration,
) {
    assert!(visual.depth > 0);
    assert_eq!(visual.height, expected_config.height as u16);
    assert_eq!(visual.width, expected_config.width as u16);
    // TODO: assess remaining values for potential invariant
}

fn check_stsz_sanity(stsz: &mp4_atom::Stsz, expected_config: &ExpectedConfiguration) {
    let samples = &stsz.samples;
    match samples {
        mp4_atom::StszSamples::Identical { count, size } => {
            assert!(*count > 0);
            assert!(*size > 0);
        }
        mp4_atom::StszSamples::Different { sizes } => {
            if expected_config.is_fragmented {
                assert!(sizes.is_empty())
            } else {
                assert!(!sizes.is_empty())
            }
        }
    }
}

fn check_stts_sanity(stts: &mp4_atom::Stts, expected_config: &ExpectedConfiguration) {
    if expected_config.is_fragmented {
        assert!(stts.entries.is_empty());
    } else {
        assert!(!stts.entries.is_empty());
    }
    // TODO: see if there is anything generic about the stts entries we could check
}
