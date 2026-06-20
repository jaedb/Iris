//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{self, Cursor};

use anyhow::{bail, Context as _};
use bitstream_io::{
    BigEndian, BitRead, ByteRead as _, ByteReader, FromBitStream, FromBitStreamWith,
    FromByteStream, LittleEndian,
};
use smallvec::SmallVec;

use super::bool_decoder::BoolDecoder;

#[derive(Debug)]
#[allow(unused)]
pub struct UncompressedFrameHeader {
    pub first_partition_size: u32,
    pub is_keyframe: bool,
    pub show_frame: bool,
    pub profile: u8,
    /// Horizontal and vertical scale only set for keyframes
    pub scale: Option<(u8, u8)>,
    /// Width and height only set for keyframes
    pub resolution: Option<(u16, u16)>,
    // More fields follow that we don't parse
}

impl FromByteStream for UncompressedFrameHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let b = r.read::<u8>().context("frame_header")?;

        let is_keyframe = (b & 0b0000_0001) == 0;
        let show_frame = (b & 0b0001_0000) != 0;
        let profile = (b >> 1) & 0b0011;
        let size0 = (b & 0b1110_0000) >> 5;

        let size1 = r.read::<u8>().context("size1")?;
        let size2 = r.read::<u8>().context("size2")?;

        let first_partition_size =
            ((size2 as u32) << (16 - 5)) | ((size1 as u32) << (8 - 5)) | (size0 as u32);

        let (scale, resolution) = if is_keyframe {
            let sync_code_1 = r.read::<u8>().context("sync_code_1")?;
            let sync_code_2 = r.read::<u8>().context("sync_code_2")?;
            let sync_code_3 = r.read::<u8>().context("sync_code_3")?;

            if [0x9d, 0x01, 0x2a] != [sync_code_1, sync_code_2, sync_code_3] {
                bail!("Invalid sync code");
            }

            let w = r.read_as::<LittleEndian, u16>().context("width")?;
            let h = r.read_as::<LittleEndian, u16>().context("height")?;

            (
                Some(((w >> 14) as u8, (h >> 14) as u8)),
                Some((w & 0b0011_1111_1111_1111, h & 0b0011_1111_1111_1111)),
            )
        } else {
            (None, None)
        };

        Ok(UncompressedFrameHeader {
            is_keyframe,
            show_frame,
            profile,
            scale,
            resolution,
            first_partition_size,
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct FrameHeader {
    pub color_space: Option<u8>,
    pub clamping_type: Option<u8>,
    pub update_segmentation: Option<UpdateSegmentation>,
    pub filter_type: u8,
    pub loop_filter_level: u8,
    pub sharpness_level: u8,
    pub mb_lf_adjustments: Option<MbLfAdjustments>,
    pub nbr_of_dct_partitions: u8,
    // More fields following
}

/// Helper trait to read an unsigned value and its sign bit.
trait BitReadExt: BitRead {
    /// Read an `i8` from 1-7 bits absolute value and a sign bit.
    // TODO: Could make this generic over `SignedNumeric` but that requires implementing a
    // `as_negative()` that works with absolute value + sign instead of two's complement.
    fn read_with_sign(&mut self, bits: u32) -> Result<i8, io::Error> {
        assert!(bits > 0 && bits <= 7);
        let value = self.read_var::<u8>(bits)?;
        let sign = self.read_bit()?;

        if sign {
            Ok(-(value as i8))
        } else {
            Ok(value as i8)
        }
    }
}

impl<T: BitRead + ?Sized> BitReadExt for T {}

impl FromBitStreamWith<'_> for FrameHeader {
    type Error = anyhow::Error;
    /// Keyframe?
    type Context = bool;

    fn from_reader<R: BitRead + ?Sized>(
        r: &mut R,
        keyframe: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        // Technically this uses arithmetic coding / range coding but the probability of every bit
        // that is read here is 50:50 so it's equivalent to no encoding at all.
        let (color_space, clamping_type) = if *keyframe {
            (
                Some(r.read::<1, u8>().context("color_space")?),
                Some(r.read::<1, u8>().context("clamping_type")?),
            )
        } else {
            (None, None)
        };

        let segmentation_enabled = r.read_bit().context("segmentation_enabled")?;
        let update_segmentation = if segmentation_enabled {
            Some(
                r.parse::<UpdateSegmentation>()
                    .context("update_segmentation")?,
            )
        } else {
            None
        };

        let filter_type = r.read::<1, u8>().context("filter_type")?;
        let loop_filter_level = r.read::<6, u8>().context("loop_filter_level")?;
        let sharpness_level = r.read::<3, u8>().context("sharpness_level")?;

        let loop_filter_adj_enable = r.read_bit().context("loop_filter_adj_enable")?;

        let mb_lf_adjustments = if loop_filter_adj_enable {
            Some(r.parse::<MbLfAdjustments>().context("mb_lf_adjustments")?)
        } else {
            None
        };

        let nbr_of_dct_partitions = 1 << r.read::<2, u8>().context("nbr_of_dct_partitions")?;

        Ok(FrameHeader {
            color_space,
            clamping_type,
            update_segmentation,
            filter_type,
            loop_filter_level,
            sharpness_level,
            mb_lf_adjustments,
            nbr_of_dct_partitions,
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct UpdateSegmentation {
    pub segment_feature_data: Option<SegmentFeatureData>,
    pub mb_segmentation_map: Option<MbSegmentationMap>,
}

impl FromBitStream for UpdateSegmentation {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let update_mb_segmentation_map = r.read_bit().context("update_mb_segmentation_map")?;
        let update_segment_feature_data = r.read_bit().context("update_segment_feature_data")?;

        let segment_feature_data = if update_segment_feature_data {
            Some(
                r.parse::<SegmentFeatureData>()
                    .context("segment_feature_data")?,
            )
        } else {
            None
        };

        let mb_segmentation_map = if update_mb_segmentation_map {
            Some(
                r.parse::<MbSegmentationMap>()
                    .context("mb_segmentation_map")?,
            )
        } else {
            None
        };

        Ok(UpdateSegmentation {
            segment_feature_data,
            mb_segmentation_map,
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct SegmentFeatureData {
    pub segment_feature_mode: u8,
    pub quantizer_update: SmallVec<[Option<i8>; 4]>,
    pub loop_filter_update: SmallVec<[Option<i8>; 4]>,
}

impl FromBitStream for SegmentFeatureData {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let segment_feature_mode = r.read::<1, u8>().context("segment_feature_mode")?;

        let mut quantizer_update = SmallVec::new();
        let mut loop_filter_update = SmallVec::new();
        for _ in 0..4 {
            let quantizer_update_flag = r.read_bit().context("quantizer_update_flag")?;

            if quantizer_update_flag {
                quantizer_update.push(Some(r.read_with_sign(7).context("quantizer_update")?));
            } else {
                quantizer_update.push(None);
            }
        }
        for _ in 0..4 {
            let loop_filter_update_flags = r.read_bit().context("loop_filter_update_flags")?;

            if loop_filter_update_flags {
                loop_filter_update.push(Some(r.read_with_sign(6).context("lf_update")?));
            } else {
                loop_filter_update.push(None);
            }
        }

        Ok(SegmentFeatureData {
            segment_feature_mode,
            quantizer_update,
            loop_filter_update,
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct MbSegmentationMap {
    pub segment_probs: SmallVec<[Option<u8>; 3]>,
}

impl FromBitStream for MbSegmentationMap {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let mut segment_probs = SmallVec::new();
        for _ in 0..3 {
            let segment_prob_update = r.read_bit().context("segment_prob_update")?;

            if segment_prob_update {
                let segment_prob = r.read::<8, u8>().context("segment_prob")?;

                segment_probs.push(Some(segment_prob));
            } else {
                segment_probs.push(None);
            }
        }

        Ok(MbSegmentationMap { segment_probs })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct MbLfAdjustments {
    pub ref_frame_delta_update: SmallVec<[Option<i8>; 4]>,
    pub mb_mode_delta_update: SmallVec<[Option<i8>; 4]>,
}

impl FromBitStream for MbLfAdjustments {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        // loop_filter_adj_enable already read by caller!

        let mode_ref_lf_delta_update = r.read_bit().context("mode_ref_lf_delta_update")?;

        let mut ref_frame_delta_update = SmallVec::new();
        let mut mb_mode_delta_update = SmallVec::new();
        if mode_ref_lf_delta_update {
            for _ in 0..4 {
                let ref_frame_delta_update_flag =
                    r.read_bit().context("ref_frame_delta_update_flag")?;

                if ref_frame_delta_update_flag {
                    ref_frame_delta_update
                        .push(Some(r.read_with_sign(6).context("delta_magnitude")?));
                } else {
                    ref_frame_delta_update.push(None);
                }
            }
            for _ in 0..4 {
                let mb_mode_delta_update_flag =
                    r.read_bit().context("mb_mode_delta_update_flag")?;

                if mb_mode_delta_update_flag {
                    mb_mode_delta_update
                        .push(Some(r.read_with_sign(6).context("delta_magnitude")?));
                } else {
                    mb_mode_delta_update.push(None);
                }
            }
        }

        Ok(MbLfAdjustments {
            ref_frame_delta_update,
            mb_mode_delta_update,
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct FrameInfo {
    pub uncompressed_frame_header: UncompressedFrameHeader,
    pub frame_header: FrameHeader,
    pub partition_offsets: SmallVec<[u32; 10]>,
}

impl FrameInfo {
    pub fn parse(data: impl AsRef<[u8]>) -> Result<FrameInfo, anyhow::Error> {
        let data = data.as_ref();
        let mut cursor = Cursor::new(data);

        let mut r = ByteReader::endian(&mut cursor, BigEndian);

        let uncompressed_frame_header = r
            .parse::<UncompressedFrameHeader>()
            .context("uncompressed_frame_header")?;

        let offset = cursor.position();
        if data.len() < offset as usize + uncompressed_frame_header.first_partition_size as usize {
            bail!("not enough data");
        }

        let mut r = BoolDecoder::new(&mut cursor).context("bool_decoder")?;
        let frame_header = r
            .parse_with::<FrameHeader>(&uncompressed_frame_header.is_keyframe)
            .context("frame_header")?;

        cursor.set_position(offset);
        let mut r = ByteReader::endian(&mut cursor, BigEndian);

        // Read partition sizes and calculate offsets from the start of the data
        let mut partition_offsets = SmallVec::<[u32; 10]>::new();

        // The partition sizes are stored right after the first partition as 24 bit little endian
        // integers. The last partition size is not given but until the end of the data.

        // We consider the uncompressed header and partition sizes as part of the first partition
        partition_offsets.push(0);

        // Skip to the partition sizes
        r.skip(uncompressed_frame_header.first_partition_size)
            .context("first_partition")?;

        // Offset of the second partition!
        let mut current_offset = uncompressed_frame_header.first_partition_size
            + offset as u32
            + 3 * (frame_header.nbr_of_dct_partitions as u32 - 1);

        for _ in 1..frame_header.nbr_of_dct_partitions {
            let size0 = r.read::<u8>().context("size0")?;
            let size1 = r.read::<u8>().context("size1")?;
            let size2 = r.read::<u8>().context("size2")?;

            let current_partition_size =
                ((size2 as u32) << 16) | ((size1 as u32) << 8) | (size0 as u32);

            partition_offsets.push(current_offset);

            current_offset += current_partition_size;
        }

        partition_offsets.push(current_offset);

        // Check if the partition offsets are actually valid. If they go outside the frame then
        // assume we just don't know anything about partitions and there's only a single one.
        if current_offset >= data.len() as u32 {
            partition_offsets.clear();
            partition_offsets.push(0);
        }
        partition_offsets.push(data.len() as u32);

        Ok(FrameInfo {
            uncompressed_frame_header,
            frame_header,
            partition_offsets,
        })
    }
}
