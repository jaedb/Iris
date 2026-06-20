//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(non_camel_case_types)]

use bitstream_io::{BitRead, BitReader, BitWrite, BitWriter, Endianness};
use std::io::{self, Read, Seek, Write};

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

pub fn write_leb128<W, E>(writer: &mut BitWriter<W, E>, mut value: u32) -> io::Result<()>
where
    W: Write + Seek,
    E: Endianness,
{
    loop {
        writer.write_bit(value > 0x7f)?;
        writer.write::<7, u32>(value & 0x7f)?;
        value >>= 7;
        if value == 0 {
            writer.byte_align()?;
            return Ok(());
        }
    }
}

pub fn leb128_size(mut value: u32) -> u8 {
    let mut bytes = 1;

    loop {
        value >>= 7;
        if value == 0 {
            return bytes;
        }
        bytes += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
    use std::io::Cursor;

    #[test]
    fn test_leb128() {
        const TEST_CASES: [(u32, &[u8]); 8] = [
            (0, &[0x00]),
            (1, &[0x01]),
            (2, &[0x02]),
            (3, &[0x03]),
            (123, &[0x7b]),
            (2468, &[0xa4, 0x13]),
            (987654, &[0x86, 0xa4, 0x3c]),
            (u32::MAX, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
        ];

        for (value, encoding) in TEST_CASES {
            println!("testing: value={value}");

            let mut reader = BitReader::endian(Cursor::new(&encoding), BigEndian);
            assert_eq!(
                (value, encoding.len() as u32),
                parse_leb128(&mut reader).unwrap()
            );
            assert_eq!(
                encoding.len() as u64 * 8,
                reader.position_in_bits().unwrap()
            );

            let mut writer = BitWriter::endian(Cursor::new(Vec::new()), BigEndian);
            write_leb128(&mut writer, value).unwrap();
            writer.byte_align().unwrap();

            let mut data = writer.into_writer();
            data.set_position(0);

            let mut reader = BitReader::endian(data, BigEndian);
            assert_eq!(
                (value, encoding.len() as u32),
                parse_leb128(&mut reader).unwrap()
            );
        }
    }
}
