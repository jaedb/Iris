// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::{bail, Context as _, Error};
use bitstream_io::FromBitStream;

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) struct StreamHeader {
    pub mapping_major_version: u8,
    pub mapping_minor_version: u8,
    pub num_headers: u16,
    pub stream_info: StreamInfo,
}

impl FromBitStream for StreamHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let packet_type = r.read_to::<u8>().context("packet_type")?;
        if packet_type != 0x7f {
            bail!("Invalid packet type");
        }
        let signature = r.read_to::<[u8; 4]>().context("signature")?;
        if &signature != b"FLAC" {
            bail!("Invalid FLAC signature");
        }

        let mapping_major_version = r.read_to::<u8>().context("mapping_major_version")?;
        let mapping_minor_version = r.read_to::<u8>().context("mapping_minor_version")?;
        let num_headers = r.read_to::<u16>().context("num_headers")?;
        let signature = r.read_to::<[u8; 4]>().context("signature")?;
        if &signature != b"fLaC" {
            bail!("Invalid fLaC signature");
        }

        let stream_info = r.parse::<StreamInfo>().context("stream_info")?;

        Ok(StreamHeader {
            mapping_major_version,
            mapping_minor_version,
            num_headers,
            stream_info,
        })
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) struct StreamInfo {
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub sample_rate: u32,
    pub num_channels: u8,
    pub bits_per_sample: u8,
    pub num_samples: u64,
    pub md5: [u8; 16],
}

impl FromBitStream for StreamInfo {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let _is_last = r.read_bit().context("is_last")?;
        let metadata_block_type = r.read::<7, u8>().context("metadata_block_type")?;
        if metadata_block_type != 0 {
            bail!("Invalid metadata block type {metadata_block_type}");
        }
        let _metadata_block_size = r.read::<24, u32>().context("metadata_block_size")?;

        let min_block_size = r.read_to::<u16>().context("min_block_size")?;
        let max_block_size = r.read_to::<u16>().context("max_block_size")?;
        let min_frame_size = r.read::<24, u32>().context("min_frame_size")?;
        let max_frame_size = r.read::<24, u32>().context("max_frame_size")?;
        let sample_rate = r.read::<20, u32>().context("sample_rate")?;
        let num_channels = r.read::<3, u8>().context("num_channels")? + 1;
        let bits_per_sample = r.read::<5, u8>().context("bits_per_sample")? + 1;
        let num_samples = r.read::<36, u64>().context("num_samples")?;
        let md5 = r.read_to::<[u8; 16]>().context("md5")?;

        Ok(StreamInfo {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            num_channels,
            bits_per_sample,
            num_samples,
            md5,
        })
    }
}

pub(crate) fn parse_flac_stream_header(
    caps: &gst::CapsRef,
) -> Result<(StreamHeader, Vec<gst::Buffer>), Error> {
    use bitstream_io::BitRead as _;

    let s = caps.structure(0).unwrap();
    let Ok(streamheader) = s.get::<gst::ArrayRef>("streamheader") else {
        bail!("Need streamheader in caps for FLAC");
    };

    let Some((streaminfo, remainder)) = streamheader.as_ref().split_first() else {
        bail!("Empty FLAC streamheader");
    };
    let streaminfo = streaminfo.get::<&gst::Buffer>().unwrap();
    let map = streaminfo.map_readable().unwrap();

    let mut reader = bitstream_io::BitReader::endian(
        std::io::Cursor::new(map.as_slice()),
        bitstream_io::BigEndian,
    );

    let header = reader
        .parse::<StreamHeader>()
        .context("Parsing FLAC streamheader")?;

    Ok((
        header,
        std::iter::once(gst::Buffer::from_mut_slice(Vec::from(&map[13..])))
            .chain(remainder.iter().map(|v| v.get::<gst::Buffer>().unwrap()))
            .collect::<Vec<_>>(),
    ))
}
