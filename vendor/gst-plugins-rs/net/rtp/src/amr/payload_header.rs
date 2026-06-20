// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::{bail, Context as _};
use bitstream_io::{FromBitStreamWith, FromByteStreamWith, ToBitStreamWith, ToByteStreamWith};
use smallvec::SmallVec;

#[derive(Debug)]
pub struct PayloadConfiguration {
    pub has_crc: bool,
    pub wide_band: bool,
}

#[derive(Debug)]
pub struct PayloadHeader {
    pub cmr: u8,
    // We don't handle interleaving yet so ILL/ILP are not parsed
    pub toc_entries: SmallVec<[TocEntry; 16]>,
    #[allow(unused)]
    pub crc: SmallVec<[u8; 16]>,
}

impl PayloadHeader {
    pub fn buffer_size(&self, wide_band: bool) -> usize {
        let frame_sizes = if wide_band {
            WB_FRAME_SIZES_BYTES.as_slice()
        } else {
            NB_FRAME_SIZES_BYTES.as_slice()
        };

        self.toc_entries
            .iter()
            .map(|entry| 1 + *frame_sizes.get(entry.frame_type as usize).unwrap_or(&0) as usize)
            .sum()
    }
}

impl FromByteStreamWith<'_> for PayloadHeader {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(
        r: &mut R,
        cfg: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let b = r.read::<u8>().context("cmr")?;
        let cmr = (b & 0b1111_0000) >> 4;

        let mut toc_entries = SmallVec::<[TocEntry; 16]>::new();
        loop {
            let toc_entry = r.parse_with::<TocEntry>(cfg).context("toc_entry")?;
            let last = toc_entry.last;
            toc_entries.push(toc_entry);

            if last {
                break;
            }
        }

        let mut crc = SmallVec::<[u8; 16]>::new();
        if cfg.has_crc {
            for toc_entry in &toc_entries {
                // Frame types without payload
                if toc_entry.frame_type > 9 {
                    continue;
                }
                let c = r.read::<u8>().context("crc")?;
                crc.push(c);
            }
        }

        Ok(PayloadHeader {
            cmr,
            toc_entries,
            crc,
        })
    }
}

impl FromBitStreamWith<'_> for PayloadHeader {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(
        r: &mut R,
        cfg: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        if cfg.has_crc {
            bail!("CRC not allowed in bandwidth-efficient mode");
        }

        let cmr = r.read::<4, u8>().context("cmr")?;

        let mut toc_entries = SmallVec::<[TocEntry; 16]>::new();
        loop {
            let toc_entry = r.parse_with::<TocEntry>(cfg).context("toc_entry")?;
            let last = toc_entry.last;
            toc_entries.push(toc_entry);

            if last {
                break;
            }
        }

        let crc = SmallVec::<[u8; 16]>::new();

        Ok(PayloadHeader {
            cmr,
            toc_entries,
            crc,
        })
    }
}

impl ToByteStreamWith<'_> for PayloadHeader {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(
        &self,
        w: &mut W,
        cfg: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if cfg.has_crc {
            bail!("Writing CRC not supported");
        }
        if self.cmr < 0b1111 {
            bail!("Invalid CMR value");
        }
        if self.toc_entries.is_empty() {
            bail!("No TOC entries");
        }

        w.write::<u8>(self.cmr << 4).context("cmr")?;

        for (i, entry) in self.toc_entries.iter().enumerate() {
            let mut entry = entry.clone();
            entry.last = i == self.toc_entries.len() - 1;

            w.build_with::<TocEntry>(&entry, cfg).context("toc_entry")?;
        }

        Ok(())
    }
}

impl ToBitStreamWith<'_> for PayloadHeader {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn to_writer<W: bitstream_io::BitWrite + ?Sized>(
        &self,
        w: &mut W,
        cfg: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if cfg.has_crc {
            bail!("Writing CRC not supported");
        }
        if self.cmr < 0b1111 {
            bail!("Invalid CMR value");
        }
        if self.toc_entries.is_empty() {
            bail!("No TOC entries");
        }

        w.write::<4, u8>(self.cmr).context("cmr")?;

        for (i, entry) in self.toc_entries.iter().enumerate() {
            let mut entry = entry.clone();
            entry.last = i == self.toc_entries.len() - 1;

            w.build_with::<TocEntry>(&entry, cfg).context("toc_entry")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub frame_type: u8,
    pub frame_quality_indicator: bool,
    pub last: bool,
}

impl TocEntry {
    pub fn frame_header(&self) -> u8 {
        (self.frame_type << 3) | ((self.frame_quality_indicator as u8) << 2)
    }
}

impl FromByteStreamWith<'_> for TocEntry {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(
        r: &mut R,
        cfg: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let b = r.read::<u8>().context("toc_entry")?;

        let last = (b & 0b1000_0000) == 0;
        let frame_type = (b & 0b0111_1000) >> 3;
        let frame_quality_indicator = (b & 0b0000_0100) != 0;

        if !cfg.wide_band && (9..=14).contains(&frame_type) {
            bail!("Invalid AMR frame type {frame_type}");
        }
        if cfg.wide_band && (10..=13).contains(&frame_type) {
            bail!("Invalid AMR-WB frame type {frame_type}");
        }

        Ok(TocEntry {
            frame_type,
            frame_quality_indicator,
            last,
        })
    }
}

impl FromBitStreamWith<'_> for TocEntry {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(
        r: &mut R,
        cfg: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let last = !r.read_bit().context("last")?;
        let frame_type = r.read::<4, u8>().context("frame_type")?;
        let frame_quality_indicator = r.read_bit().context("q")?;

        if !cfg.wide_band && (9..=14).contains(&frame_type) {
            bail!("Invalid AMR frame type {frame_type}");
        }
        if cfg.wide_band && (10..=13).contains(&frame_type) {
            bail!("Invalid AMR-WB frame type {frame_type}");
        }

        Ok(TocEntry {
            frame_type,
            frame_quality_indicator,
            last,
        })
    }
}

impl ToByteStreamWith<'_> for TocEntry {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(
        &self,
        w: &mut W,
        cfg: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if !cfg.wide_band && (9..=14).contains(&self.frame_type) {
            bail!("Invalid AMR frame type {}", self.frame_type);
        }
        if cfg.wide_band && (10..=13).contains(&self.frame_type) {
            bail!("Invalid AMR-WB frame type {}", self.frame_type);
        }
        if self.frame_type > 15 {
            bail!("Invalid AMR frame type {}", self.frame_type);
        }

        let b = ((!self.last as u8) << 7)
            | (self.frame_type << 3)
            | ((self.frame_quality_indicator as u8) << 2);

        w.write::<u8>(b).context("toc_entry")?;

        Ok(())
    }
}

impl ToBitStreamWith<'_> for TocEntry {
    type Error = anyhow::Error;

    type Context = PayloadConfiguration;

    fn to_writer<W: bitstream_io::BitWrite + ?Sized>(
        &self,
        w: &mut W,
        cfg: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if !cfg.wide_band && (9..=14).contains(&self.frame_type) {
            bail!("Invalid AMR frame type {}", self.frame_type);
        }
        if cfg.wide_band && (10..=13).contains(&self.frame_type) {
            bail!("Invalid AMR-WB frame type {}", self.frame_type);
        }
        if self.frame_type > 15 {
            bail!("Invalid AMR frame type {}", self.frame_type);
        }

        w.write_bit(!self.last).context("last")?;
        w.write::<4, u8>(self.frame_type).context("frame_type")?;
        w.write_bit(self.frame_quality_indicator)
            .context("frame_quality_indicator")?;

        Ok(())
    }
}

// See RFC3267 Table 1
pub static NB_FRAME_SIZES: [u16; 9] = [95, 103, 118, 134, 148, 159, 204, 244, 39];
pub static NB_FRAME_SIZES_BYTES: [u8; 9] = [12, 13, 15, 17, 19, 20, 26, 31, 5];

// See ETSI TS 126 201 Table 2 and 3
pub static WB_FRAME_SIZES: [u16; 10] = [132, 177, 253, 285, 317, 365, 397, 461, 477, 40];
pub static WB_FRAME_SIZES_BYTES: [u8; 10] = [17, 23, 32, 36, 40, 46, 50, 58, 60, 5];
