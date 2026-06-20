//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use anyhow::{bail, Context as _};
use bitstream_io::{
    BigEndian, ByteWrite as _, ByteWriter, FromByteStream, FromByteStreamWith, ToByteStream,
    ToByteStreamWith,
};

#[derive(Default)]
struct Counter(usize);

impl io::Write for Counter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MainHeader {
    pub type_specific: u8,
    pub fragment_offset: u32,
    pub type_: u8,
    pub q: u8,
    pub width: u16,
    pub height: u16,
}

impl MainHeader {
    pub fn size(&self) -> Result<usize, anyhow::Error> {
        let mut counter = Counter::default();
        let mut w = ByteWriter::endian(&mut counter, BigEndian);
        w.build::<MainHeader>(self)?;

        Ok(counter.0)
    }
}

impl FromByteStream for MainHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let type_specific = r.read::<u8>().context("type_specific")?;
        let fragment_offset = u32::from_be_bytes([
            0,
            r.read::<u8>().context("fragment_offset")?,
            r.read::<u8>().context("fragment_offset")?,
            r.read::<u8>().context("fragment_offset")?,
        ]);
        let type_ = r.read::<u8>().context("type")?;
        let q = r.read::<u8>().context("q")?;
        let width = r.read::<u8>().context("width")?;
        let height = r.read::<u8>().context("height")?;
        let width = u16::from(width) * 8;
        let height = u16::from(height) * 8;

        if ![0, 1, 64, 65].contains(&type_) {
            bail!("Unsupported RTP JPEG type {type_}");
        }

        if type_specific != 0 {
            bail!("Interlaced JPEG not supported");
        }

        Ok(MainHeader {
            type_specific,
            fragment_offset,
            type_,
            q,
            width,
            height,
        })
    }
}

impl ToByteStream for MainHeader {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        w.write::<u8>(self.type_specific).context("type_specific")?;

        if self.fragment_offset > 0x00ff_ffff {
            bail!("Too big frame");
        }

        let fragment_offset = self.fragment_offset.to_be_bytes();
        w.write_bytes(&fragment_offset[1..])
            .context("fragment_offset")?;

        w.write::<u8>(self.type_).context("type_")?;
        w.write::<u8>(self.q).context("q")?;
        if self.height > 2040 || self.width > 2040 {
            w.write::<u16>(0).context("width_height")?;
        } else {
            w.write::<u8>((self.width / 8) as u8).context("width")?;
            w.write::<u8>((self.height / 8) as u8).context("height")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RestartHeader {
    pub restart_interval: u16,
    pub first: bool,
    pub last: bool,
    pub restart_count: u16,
}

impl FromByteStream for RestartHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let restart_interval = r.read_as::<BigEndian, u16>().context("restart_interval")?;
        let restart_count = r.read_as::<BigEndian, u16>().context("restart_count")?;

        let first = (restart_count & 0b1000_0000_0000_0000) != 0;
        let last = (restart_count & 0b0100_0000_0000_0000) != 0;

        let restart_count = restart_count & 0b0011_1111_1111_1111;

        Ok(RestartHeader {
            restart_interval,
            first,
            last,
            restart_count,
        })
    }
}

impl ToByteStreamWith<'_> for RestartHeader {
    type Error = anyhow::Error;

    type Context = MainHeader;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(
        &self,
        w: &mut W,
        main_header: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        // Nothing to write here
        if main_header.type_ < 64 {
            return Ok(());
        }

        w.write::<u16>(self.restart_interval)
            .context("restart_interval")?;

        if self.restart_count > 0b0011_1111_1111_1111 {
            bail!("Too high restart count");
        }

        w.write::<u16>(
            self.restart_count
                | if self.first { 0b1000_0000_0000_0000 } else { 0 }
                | if self.last { 0b0100_0000_0000_0000 } else { 0 },
        )
        .context("restart_count")?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct QuantizationTableHeader {
    pub luma_quant: [u8; 128],
    pub luma_len: u8,
    pub chroma_quant: [u8; 128],
    pub chroma_len: u8,
}

impl QuantizationTableHeader {
    pub fn size(&self, main_header: &MainHeader) -> Result<usize, anyhow::Error> {
        let mut counter = Counter::default();
        let mut w = ByteWriter::endian(&mut counter, BigEndian);
        w.build_with::<QuantizationTableHeader>(self, main_header)?;

        Ok(counter.0)
    }
}

impl Default for QuantizationTableHeader {
    fn default() -> Self {
        Self {
            luma_quant: [0u8; 128],
            luma_len: 0,
            chroma_quant: [0u8; 128],
            chroma_len: 0,
        }
    }
}

impl FromByteStreamWith<'_> for QuantizationTableHeader {
    type Error = anyhow::Error;
    type Context = MainHeader;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(
        r: &mut R,
        main_header: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        assert!(main_header.q >= 128);
        assert!(main_header.fragment_offset == 0);
        assert!([0, 1, 64, 65].contains(&main_header.type_));

        let _mbz = r.read::<u8>().context("mbz")?;
        let precision = r.read::<u8>().context("precision")?;
        let length = r.read_as::<BigEndian, u16>().context("length")?;

        if length == 0 && main_header.q == 255 {
            bail!("Dynamic quantization tables can't be empty");
        }

        let mut luma_quant = [0u8; 128];
        let mut luma_len = 0;
        let mut chroma_quant = [0u8; 128];
        let mut chroma_len = 0;
        if length != 0 {
            // All currently supported types have two tables
            luma_len = if precision & 1 != 0 { 128 } else { 64 };
            chroma_len = if precision & 2 != 0 { 128 } else { 64 };

            if length != (luma_len + chroma_len) as u16 {
                bail!("Unsupported quantization table length {length}");
            }

            r.read_bytes(&mut luma_quant[..luma_len])
                .context("luma_quant")?;
            r.read_bytes(&mut chroma_quant[..chroma_len])
                .context("chroma_quant")?;
        }

        Ok(QuantizationTableHeader {
            luma_quant,
            luma_len: luma_len as u8,
            chroma_quant,
            chroma_len: chroma_len as u8,
        })
    }
}

impl ToByteStreamWith<'_> for QuantizationTableHeader {
    type Error = anyhow::Error;

    type Context = MainHeader;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(
        &self,
        w: &mut W,
        main_header: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        // Nothing to write here
        if main_header.q < 128 {
            return Ok(());
        }

        assert!(main_header.fragment_offset == 0);
        assert!([0, 1, 64, 65].contains(&main_header.type_));

        let (precision, length) = match (self.luma_len, self.chroma_len) {
            (64, 64) => (0, 64 + 64),
            (128, 64) => (1, 128 + 64),
            (64, 128) => (2, 128 + 64),
            (128, 128) => (3, 128 + 128),
            _ => {
                bail!("Unsupported quantization table lengths");
            }
        };

        w.write::<u8>(0).context("mbz")?;
        w.write::<u8>(precision).context("precision")?;

        // TODO: Could theoretically only write the tables every few frames
        // if the same table was written before
        w.write::<u16>(length).context("length")?;

        w.write_bytes(&self.luma_quant[..self.luma_len as usize])
            .context("luma_quant")?;
        w.write_bytes(&self.chroma_quant[..self.chroma_len as usize])
            .context("chroma_quant")?;

        Ok(())
    }
}

// From Appendix A

const ZIG_ZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

// Table K.1 from JPEG spec.
static JPEG_LUMA_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

// Table K.2 from JPEG spec.
static JPEG_CHROMA_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

pub fn detect_static_quant_table(
    luma_quant: &[u8],
    chroma_quant: &[u8],
    previous_q: Option<u8>,
) -> Option<u8> {
    if luma_quant.len() != 64 || chroma_quant.len() != 64 {
        return None;
    }

    // TODO: Could estimate a lower bound for q here based on luma_quant[0]
    // but probably doesn't really matter in the bigger picture.

    // Short-cut in case quantization tables don't change
    if let Some(previous_q) = previous_q {
        if Iterator::zip(
            Iterator::zip(luma_quant.iter().copied(), chroma_quant.iter().copied()),
            make_quant_tables_iter(previous_q),
        )
        .all(|(a, b)| a == b)
        {
            return Some(previous_q);
        }
    }

    (1..=99).find(|&q| {
        Iterator::zip(
            Iterator::zip(luma_quant.iter().copied(), chroma_quant.iter().copied()),
            make_quant_tables_iter(q),
        )
        .all(|(a, b)| a == b)
    })
}

pub fn make_quant_tables_iter(q: u8) -> impl Iterator<Item = (u8, u8)> {
    let factor = u8::clamp(q, 1, 99);
    let q = if q < 50 {
        5000 / factor as u32
    } else {
        200 - factor as u32 * 2
    };

    ZIG_ZAG
        .iter()
        .copied()
        .map(|idx| {
            let idx = idx as usize;
            (JPEG_LUMA_QUANT[idx], JPEG_CHROMA_QUANT[idx])
        })
        .map(move |(lq, cq)| {
            let lq = (lq as u32 * q + 50) / 100;
            let cq = (cq as u32 * q + 50) / 100;

            // Limit the quantizers to 1 <= q <= 255
            (u32::clamp(lq, 1, 255) as u8, u32::clamp(cq, 1, 255) as u8)
        })
}

pub fn make_quant_tables(q: u8) -> ([u8; 128], [u8; 128]) {
    let mut luma_quant = [0u8; 128];
    let mut chroma_quant = [0u8; 128];

    for ((lq_out, cq_out), (lq, cq)) in Iterator::zip(
        Iterator::zip(luma_quant.iter_mut(), chroma_quant.iter_mut()),
        make_quant_tables_iter(q),
    ) {
        *lq_out = lq;
        *cq_out = cq;
    }

    (luma_quant, chroma_quant)
}

// Appendix B

static LUM_DC_CODELENS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];

static LUM_DC_SYMBOLS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

static LUM_AC_CODELENS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];

static LUM_AC_SYMBOLS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

static CHM_DC_CODELENS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];

static CHM_DC_SYMBOLS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

static CHM_AC_CODELENS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];

static CHM_AC_SYMBOLS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

#[derive(Debug, Clone)]
pub struct JpegHeader {
    pub type_: u8,
    pub width: u16,
    pub height: u16,
    pub quant: QuantizationTableHeader,
    pub dri: u16,
}

impl JpegHeader {
    pub fn new(
        main_header: &MainHeader,
        restart_header: Option<&RestartHeader>,
        quant: QuantizationTableHeader,
        width: u16,
        height: u16,
    ) -> Result<JpegHeader, anyhow::Error> {
        if ![0, 1, 64, 65].contains(&main_header.type_) {
            bail!("Unsupported type {}", main_header.type_);
        }

        let dri = if main_header.type_ >= 64 {
            if let Some(restart_header) = restart_header {
                restart_header.restart_interval
            } else {
                bail!("Require restart header");
            }
        } else {
            0
        };

        Ok(JpegHeader {
            type_: main_header.type_,
            width,
            height,
            quant,
            dri,
        })
    }
}

impl ToByteStream for JpegHeader {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        // Generate a frame and scan headers that can be prepended to the
        // RTP/JPEG data payload to produce a JPEG compressed image in
        // interchange format (except for possible trailing garbage and
        // absence of an EOI marker to terminate the scan).

        w.write_bytes(&[0xff, 0xd8]).context("SOI")?;

        for (i, quant) in [
            &self.quant.luma_quant[..self.quant.luma_len as usize],
            &self.quant.chroma_quant[..self.quant.chroma_len as usize],
        ]
        .into_iter()
        .enumerate()
        {
            assert!([64, 128].contains(&quant.len()));
            w.write_bytes(&[0xff, 0xdb]).context("DQT")?;
            w.write::<u16>(quant.len() as u16 + 3).context("size")?;
            w.write::<u8>(i as u8).context("table_no")?;
            w.write_bytes(quant).context("quant")?;
        }

        if self.dri != 0 {
            w.write_bytes(&[0xff, 0xdd]).context("DRI")?;
            w.write::<u16>(4).context("size")?;
            w.write::<u16>(self.dri).context("dri")?;
        }

        w.write_bytes(&[0xff, 0xc0]).context("SOF")?;
        w.write::<u16>(17).context("size")?;
        w.write::<u8>(8).context("precision")?;

        w.write::<u16>(self.height).context("height")?;
        w.write::<u16>(self.width).context("width")?;
        w.write::<u8>(3).context("comps")?;
        w.write::<u8>(0).context("comp 0")?;
        w.write::<u8>(if self.type_ & 0x3f == 0 { 0x21 } else { 0x22 })
            .context("samp")?;
        w.write::<u8>(0).context("quant table")?;

        w.write::<u8>(1).context("comp 1")?;
        w.write::<u8>(0x11).context("samp")?;
        w.write::<u8>(1).context("quant table")?;

        w.write::<u8>(2).context("comp 2")?;
        w.write::<u8>(0x11).context("samp")?;
        w.write::<u8>(1).context("quant table")?;

        for (codelens, symbols, table, class) in [
            (LUM_DC_CODELENS, LUM_DC_SYMBOLS.as_slice(), 0, 0),
            (LUM_AC_CODELENS, LUM_AC_SYMBOLS.as_slice(), 0, 1),
            (CHM_DC_CODELENS, CHM_DC_SYMBOLS.as_slice(), 1, 0),
            (CHM_AC_CODELENS, CHM_AC_SYMBOLS.as_slice(), 1, 1),
        ] {
            w.write_bytes(&[0xff, 0xc4]).context("DHT")?;
            w.write::<u16>(3 + codelens.len() as u16 + symbols.len() as u16)
                .context("size")?;
            w.write::<u8>((class << 4) | table).context("class_table")?;
            w.write_bytes(&codelens).context("codelens")?;
            w.write_bytes(symbols).context("symbols")?;
        }

        w.write_bytes(&[0xff, 0xda]).context("SOS")?;
        w.write::<u16>(12).context("size")?;
        w.write::<u8>(3).context("comps")?;
        w.write::<u8>(0).context("comp")?;
        w.write::<u8>(0).context("huffman table")?;
        w.write::<u8>(1).context("comp")?;
        w.write::<u8>(0x11).context("huffman table")?;
        w.write::<u8>(2).context("comp")?;
        w.write::<u8>(0x11).context("huffman table")?;
        w.write::<u8>(0).context("first DCT coeff")?;
        w.write::<u8>(63).context("last DCT coeff")?;
        w.write::<u8>(0).context("successive approx.")?;

        Ok(())
    }
}

impl FromByteStream for JpegHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let mut width = None;
        let mut height = None;
        let mut dri = None;

        #[derive(Default, Clone, Copy)]
        struct Component {
            id: u8,
            samp: u8,
            quant_table: u8,
            huffman_table: Option<u8>,
        }
        let mut components = [Component::default(); 3];

        #[derive(Clone, Copy)]
        struct QuantTable {
            id: u8,
            len: u8,
            table: [u8; 128],
        }
        impl Default for QuantTable {
            fn default() -> Self {
                Self {
                    id: 0,
                    len: 0,
                    table: [0u8; 128],
                }
            }
        }
        let mut quant_table_idx = 0;
        let mut quant_tables = [QuantTable::default(); 2];

        // Parse the different markers in the JPEG headers here to extract the few information
        // we're actually interested in
        'marker_loop: loop {
            let marker = {
                let mut start_code = false;

                loop {
                    match r.read::<u8>()? {
                        v @ 0xc0..=0xfe if start_code => {
                            break 0xff00 | (v as u16);
                        }
                        0xff => {
                            start_code = true;
                        }
                        _ => {
                            start_code = false;
                        }
                    }
                }
            };

            match marker {
                // SOI
                0xff_d8 => (),
                // EOI
                0xff_d9 => {
                    bail!("EOI marker before SOS marker, empty image");
                }
                // SOF0
                0xff_c0 => {
                    let len = r.read::<u16>().context("len")?;
                    if len != 17 {
                        bail!("Invalid SOF length {len}");
                    }
                    let precision = r.read::<u8>().context("precision")?;
                    if precision != 8 {
                        bail!("Unsupported precision {precision}");
                    }
                    let h = r.read::<u16>().context("height")?;
                    let w = r.read::<u16>().context("width")?;
                    if width.is_some() || height.is_some() {
                        bail!("Multiple SOF");
                    }
                    width = Some(w);
                    height = Some(h);

                    let comps = r.read::<u8>().context("comps")?;
                    if comps != 3 {
                        bail!("Unsupported number of components {comps}");
                    }

                    for component in &mut components {
                        let comp = r.read::<u8>().context("comp")?;
                        let samp = r.read::<u8>().context("samp")?;
                        let quant_table = r.read::<u8>().context("quant table")?;

                        *component = Component {
                            id: comp,
                            samp,
                            quant_table,
                            huffman_table: None,
                        };
                    }

                    components.sort_by(|a, b| a.id.cmp(&b.id));
                }
                // DQT
                0xff_db => {
                    let len = r.read::<u16>().context("len")?;
                    if len != 3 + 64 && len != 3 + 128 {
                        bail!("Unsupported quantization table size {}", len - 3);
                    }
                    let table_no = r.read::<u8>().context("table_no")?;

                    if quant_table_idx > quant_tables.len() {
                        bail!("Too many quantization tables");
                    }

                    let len = len - 3;
                    quant_tables[quant_table_idx].id = table_no;
                    quant_tables[quant_table_idx].len = len as u8;
                    r.read_bytes(
                        &mut quant_tables[quant_table_idx].table[..len as usize]
                        ).context("quant")?;
                    quant_table_idx += 1;
                }
                // DRI
                0xff_dd => {
                    let len = r.read::<u16>().context("len")?;
                    if len != 4 {
                        bail!("Invalid DRI length {len}");
                    }
                    if dri.is_some() {
                        bail!("Multiple DRI");
                    }
                    dri = Some(r.read::<u16>().context("dri")?);
                }
                // SOF1, SOF2, SOF3, SOF9, SOF10, SOF11
                (0xff_c1..=0xff_c3) | (0xff_c9..=0xff_cb)
                    // DHT, DAC, COM, DNL
                    | 0xff_c4 | 0xff_cc | 0xff_fe  | 0xff_dc |
                    // APP0-APP15
                    (0xff_e0..=0xff_ef) => {
                    let len = r.read::<u16>().context("len")?;
                    if len < 2 {
                        bail!("Invalid length");
                    }
                    r.skip(len as u32 - 2).context("skip")?;
                }
                    // RST0-RST7
                (0xff_d0..=0xff_d7) => {
                    // two bytes fixed size, just the marker id itself
                }
                // SOS
                0xff_da => {
                    let len = r.read::<u16>().context("len")?;
                    if len != 12 {
                        bail!("Unsupported SOS length");
                    }

                    let comps = r.read::<u8>().context("comps")?;
                    if comps != 3 {
                        bail!("Unsupported number of components {comps}");
                    }

                    for _ in 0..3 {
                        let comp = r.read::<u8>().context("comp")?;
                        let Some(comp) = components.iter_mut().find(|c| c.id == comp) else {
                            bail!("Unhandled component {comp}");
                        };
                        let huffman_table = r.read::<u8>().context("huffman_table")?;
                        comp.huffman_table = Some(huffman_table);
                        // TODO: Could check this together with parsing DHT
                    }

                    let first_dct = r.read::<u8>().context("first DCT coeff")?;
                    if first_dct != 0 {
                        bail!("Unsupported first DCT {first_dct}");
                    }
                    let last_dct = r.read::<u8>().context("last DCT coeff")?;
                    if last_dct != 63 {
                        bail!("Unsupported last DCT {last_dct}");
                    }
                    let successive_approx = r.read::<u8>().context("successive approx.")?;
                    if successive_approx!= 0 {
                        bail!("Unsupported successive approx. {successive_approx}");
                    }

                    break 'marker_loop;
                }
                _ => (),
            }
        }

        let width = width.unwrap();
        let height = height.unwrap();
        let dri = dri.unwrap_or(0);

        // Check if the headers are compatible with the subset of JPEG covered by the RFC
        if components[0].samp != 0x21 && components[0].samp != 0x22 {
            bail!(
                "Unsupported component sampling {} for component 0",
                components[0].samp
            );
        }
        if components[1].samp != 0x11 {
            bail!(
                "Unsupported component sampling {} for component 1",
                components[1].samp
            );
        }
        if components[2].samp != 0x11 {
            bail!(
                "Unsupported component sampling {} for component 2",
                components[2].samp
            );
        }
        let type_ = match components[0].samp {
            0x21 => 0,
            0x22 => 1,
            _ => unreachable!(),
        };

        if components[1].quant_table != components[2].quant_table {
            bail!("Components 1/2 have different quantization tables");
        }
        if components[0].quant_table == components[1].quant_table {
            bail!("Component 0 has same quantization table as component 1");
        }

        if quant_table_idx != 2 {
            bail!("Wrong number of quantization tables");
        }

        let Some(luma_quant) = quant_tables
            .iter()
            .find(|t| t.id == components[0].quant_table)
        else {
            bail!("Can't find luma quantization table");
        };

        let Some(chroma_quant) = quant_tables
            .iter()
            .find(|t| t.id == components[1].quant_table)
        else {
            bail!("Can't find chroma quantization table");
        };

        Ok(JpegHeader {
            type_,
            width,
            height,
            quant: QuantizationTableHeader {
                luma_quant: luma_quant.table,
                luma_len: luma_quant.len,
                chroma_quant: chroma_quant.table,
                chroma_len: chroma_quant.len,
            },
            dri,
        })
    }
}
