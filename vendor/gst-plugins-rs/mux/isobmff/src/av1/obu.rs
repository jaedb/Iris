//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
#![allow(non_camel_case_types)]

use bitstream_io::{BigEndian, BitRead, BitReader, Endianness};
use std::io::{self, Cursor, Read, Seek, SeekFrom};

pub fn parse_leb128<R, E>(reader: &mut BitReader<R, E>) -> io::Result<(u32, u32)>
where
    R: Read + Seek,
    E: Endianness,
{
    let mut value = 0;
    let mut num_bytes = 0;

    for i in 0..8 {
        let byte = reader.read::<8, u32>()?;
        value |= (byte & 0x7f) << (i * 7);
        num_bytes += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }

    reader.byte_align();
    Ok((value, num_bytes))
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizedObu {
    pub obu_type: ObuType,
    pub has_extension: bool,
    /// If the OBU header is followed by a leb128 size field.
    pub has_size_field: bool,
    pub temporal_id: u8,
    pub spatial_id: u8,
    /// size of the OBU payload in bytes.
    /// This may refer to different sizes in different contexts, not always
    /// to the entire OBU payload as it is in the AV1 bitstream.
    pub size: u32,
    /// the number of bytes the leb128 size field will take up
    /// when written with write_leb128().
    /// This does not imply `has_size_field`, and does not necessarily match with
    /// the length of the internal size field if present.
    pub leb_size: u32,
    pub header_len: u32,
    /// indicates that only part of this OBU has been processed so far
    pub is_fragment: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObuType {
    #[default]
    Reserved,
    SequenceHeader,
    TemporalDelimiter,
    FrameHeader,
    TileGroup,
    Metadata,
    Frame,
    RedundantFrameHeader,
    TileList,
    Padding,
}

impl SizedObu {
    /// Parse an OBU header and size field. If the OBU is not expected to contain
    /// a size field, but the size is known from external information,
    /// parse as an `UnsizedObu` and use `to_sized`.
    pub fn parse<R, E>(reader: &mut BitReader<R, E>) -> io::Result<Self>
    where
        R: Read + Seek,
        E: Endianness,
    {
        // check the forbidden bit
        if reader.read_bit()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "forbidden bit in OBU header is set",
            ));
        }

        let obu_type = reader.read::<4, u8>()?.into();
        let has_extension = reader.read_bit()?;

        // require a size field
        if !reader.read_bit()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected a size field",
            ));
        }

        // ignore the reserved bit
        let _ = reader.read_bit()?;

        let (temporal_id, spatial_id) = if has_extension {
            (reader.read::<3, u8>()?, reader.read::<2, u8>()?)
        } else {
            (0, 0)
        };

        reader.byte_align();

        let (size, leb_size) = parse_leb128(reader)?;

        Ok(Self {
            obu_type,
            has_extension,
            has_size_field: true,
            temporal_id,
            spatial_id,
            size,
            leb_size,
            header_len: has_extension as u32 + 1,
            is_fragment: false,
        })
    }

    /// The amount of bytes this OBU will take up, including the space needed for
    /// its leb128 size field.
    pub fn full_size(&self) -> u32 {
        self.size + self.leb_size + self.header_len
    }
}

pub fn read_seq_header_obu_bytes(data: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let mut cursor = Cursor::new(data);

    while cursor.position() < data.len() as u64 {
        let obu_start = cursor.position();

        let Ok(obu) = SizedObu::parse(&mut BitReader::endian(&mut cursor, BigEndian)) else {
            break;
        };

        // set reader to the beginning of the OBU
        cursor.seek(SeekFrom::Start(obu_start))?;

        if obu.obu_type != ObuType::SequenceHeader {
            // Skip the full OBU
            cursor.seek(SeekFrom::Current(obu.full_size() as i64))?;
            continue;
        };

        // read the full OBU
        let mut bytes = vec![0; obu.full_size() as usize];
        cursor.read_exact(&mut bytes)?;

        return Ok(Some(bytes));
    }

    Ok(None)
}

impl From<u8> for ObuType {
    fn from(n: u8) -> Self {
        assert!(n < 16);

        match n {
            1 => Self::SequenceHeader,
            2 => Self::TemporalDelimiter,
            3 => Self::FrameHeader,
            4 => Self::TileGroup,
            5 => Self::Metadata,
            6 => Self::Frame,
            7 => Self::RedundantFrameHeader,
            8 => Self::TileList,
            15 => Self::Padding,
            _ => Self::Reserved,
        }
    }
}

impl From<ObuType> for u8 {
    fn from(ty: ObuType) -> Self {
        match ty {
            ObuType::Reserved => 0,
            ObuType::SequenceHeader => 1,
            ObuType::TemporalDelimiter => 2,
            ObuType::FrameHeader => 3,
            ObuType::TileGroup => 4,
            ObuType::Metadata => 5,
            ObuType::Frame => 6,
            ObuType::RedundantFrameHeader => 7,
            ObuType::TileList => 8,
            ObuType::Padding => 15,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitstream_io::{BigEndian, BitReader};
    use std::io::Cursor;
    use std::sync::LazyLock;

    #[allow(clippy::type_complexity)]
    static OBUS: LazyLock<Vec<(SizedObu, Vec<u8>)>> = LazyLock::new(|| {
        vec![
            (
                SizedObu {
                    obu_type: ObuType::TemporalDelimiter,
                    has_extension: false,
                    has_size_field: true,
                    temporal_id: 0,
                    spatial_id: 0,
                    size: 0,
                    leb_size: 1,
                    header_len: 1,
                    is_fragment: false,
                },
                vec![0b0001_0010, 0b0000_0000],
            ),
            (
                SizedObu {
                    obu_type: ObuType::Padding,
                    has_extension: false,
                    has_size_field: true,
                    temporal_id: 0,
                    spatial_id: 0,
                    size: 10,
                    leb_size: 1,
                    header_len: 1,
                    is_fragment: false,
                },
                vec![0b0111_1010, 0b0000_1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                SizedObu {
                    obu_type: ObuType::SequenceHeader,
                    has_extension: true,
                    has_size_field: true,
                    temporal_id: 4,
                    spatial_id: 3,
                    size: 5,
                    leb_size: 1,
                    header_len: 2,
                    is_fragment: false,
                },
                vec![0b0000_1110, 0b1001_1000, 0b0000_0101, 1, 2, 3, 4, 5],
            ),
            (
                SizedObu {
                    obu_type: ObuType::Frame,
                    has_extension: true,
                    has_size_field: true,
                    temporal_id: 4,
                    spatial_id: 3,
                    size: 5,
                    leb_size: 1,
                    header_len: 2,
                    is_fragment: false,
                },
                vec![0b0011_0110, 0b1001_1000, 0b0000_0101, 1, 2, 3, 4, 5],
            ),
        ]
    });

    #[test]
    fn test_parse_rtp_obu() {
        for (idx, (sized_obu, raw_bytes)) in (*OBUS).iter().enumerate() {
            println!("running test {idx}...");

            let mut reader = BitReader::endian(Cursor::new(&raw_bytes), BigEndian);

            let obu_parsed = SizedObu::parse(&mut reader).unwrap();
            assert_eq!(&obu_parsed, sized_obu);

            if let Some(seq_header_obu_bytes) = read_seq_header_obu_bytes(raw_bytes).unwrap() {
                println!("validation of sequence header obu read/write...");
                assert_eq!(&seq_header_obu_bytes, raw_bytes);
            }
        }
    }

    #[test]
    fn test_read_seq_header_from_bitstream() {
        let mut bitstream = Vec::new();
        let mut seq_header_bytes_raw = None;
        for (obu, raw_bytes) in (*OBUS).iter() {
            bitstream.extend(raw_bytes);
            if obu.obu_type == ObuType::SequenceHeader {
                seq_header_bytes_raw = Some(raw_bytes.clone());
            }
        }

        let seq_header_obu_bytes = read_seq_header_obu_bytes(&bitstream).unwrap().unwrap();
        assert_eq!(seq_header_obu_bytes, seq_header_bytes_raw.unwrap());
    }
}
