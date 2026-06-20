// Copyright 2022 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::io::Cursor;

use anyhow::anyhow;
use anyhow::Context;
use byteorder::ReadBytesExt;
use thiserror::Error;

/// A bit reader for h264 bitstreams. It properly handles emulation-prevention
/// bytes and stop bits.
pub(crate) struct NaluReader<'a> {
    /// A reference into the next unread byte in the stream.
    data: Cursor<&'a [u8]>,
    /// Contents of the current byte. First unread bit starting at position 8 -
    /// num_remaining_bits_in_curr_bytes.
    curr_byte: u8,
    /// Number of bits remaining in `curr_byte`
    num_remaining_bits_in_curr_byte: usize,
    /// Used in epb detection.
    prev_two_bytes: u16,
    /// Number of epbs (i.e. 0x000003) we found.
    num_epb: usize,
}

#[derive(Debug, Error)]
pub(crate) enum GetByteError {
    #[error("reader ran out of bits")]
    OutOfBits,
}

#[derive(Debug, Error)]
pub(crate) enum ReadBitsError {
    #[error("more than 31 ({0}) bits were requested")]
    TooManyBytesRequested(usize),
    #[error("failed to advance the current byte")]
    GetByte(#[from] GetByteError),
    #[error("failed to convert read input to target type")]
    ConversionFailed,
}

impl<'a> NaluReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data: Cursor::new(data),
            curr_byte: Default::default(),
            num_remaining_bits_in_curr_byte: Default::default(),
            prev_two_bytes: 0xffff,
            num_epb: Default::default(),
        }
    }

    /// Read a single bit from the stream.
    pub fn read_bit(&mut self) -> Result<bool, ReadBitsError> {
        let bit = self.read_bits::<u32>(1)?;
        match bit {
            1 => Ok(true),
            0 => Ok(false),
            _ => panic!("Unexpected value {bit}"),
        }
    }

    /// Read up to 31 bits from the stream.
    pub fn read_bits<U: TryFrom<u32>>(&mut self, num_bits: usize) -> Result<U, ReadBitsError> {
        if num_bits > 31 {
            return Err(ReadBitsError::TooManyBytesRequested(num_bits));
        }

        let mut bits_left = num_bits;
        let mut out = 0u32;

        while self.num_remaining_bits_in_curr_byte < bits_left {
            out |= (self.curr_byte as u32) << (bits_left - self.num_remaining_bits_in_curr_byte);
            bits_left -= self.num_remaining_bits_in_curr_byte;
            self.update_curr_byte()?;
        }

        out |= (self.curr_byte >> (self.num_remaining_bits_in_curr_byte - bits_left)) as u32;
        out &= (1 << num_bits) - 1;
        self.num_remaining_bits_in_curr_byte -= bits_left;

        U::try_from(out).map_err(|_| ReadBitsError::ConversionFailed)
    }

    /// Skip `num_bits` bits from the stream.
    pub fn skip_bits(&mut self, mut num_bits: usize) -> Result<(), ReadBitsError> {
        while num_bits > 0 {
            let n = std::cmp::min(num_bits, 31);
            self.read_bits::<u32>(n)?;
            num_bits -= n;
        }

        Ok(())
    }

    pub fn read_ue<U: TryFrom<u32>>(&mut self) -> anyhow::Result<U> {
        let mut num_bits = 0;

        while self.read_bits::<u32>(1)? == 0 {
            num_bits += 1;
        }

        if num_bits > 31 {
            return Err(anyhow!("invalid stream"));
        }

        let value = ((1u32 << num_bits) - 1)
            .checked_add(self.read_bits::<u32>(num_bits)?)
            .context("read number cannot fit in 32 bits")?;

        U::try_from(value).map_err(|_| anyhow!("conversion error"))
    }

    pub fn read_ue_bounded<U: TryFrom<u32>>(&mut self, min: u32, max: u32) -> anyhow::Result<U> {
        let ue = self.read_ue()?;
        if ue > max || ue < min {
            Err(anyhow!(
                "Value out of bounds: expected {} - {}, got {}",
                min,
                max,
                ue
            ))
        } else {
            Ok(U::try_from(ue).map_err(|_| anyhow!("Conversion error"))?)
        }
    }

    pub fn read_ue_max<U: TryFrom<u32>>(&mut self, max: u32) -> anyhow::Result<U> {
        self.read_ue_bounded(0, max)
    }

    pub fn read_se<U: TryFrom<i32>>(&mut self) -> anyhow::Result<U> {
        let ue = self.read_ue::<u32>()? as i32;

        if ue % 2 == 0 {
            Ok(U::try_from(-ue / 2).map_err(|_| anyhow!("Conversion error"))?)
        } else {
            Ok(U::try_from(ue / 2 + 1).map_err(|_| anyhow!("Conversion error"))?)
        }
    }

    pub fn read_se_bounded<U: TryFrom<i32>>(&mut self, min: i32, max: i32) -> anyhow::Result<U> {
        let se = self.read_se()?;
        if se < min || se > max {
            Err(anyhow!(
                "Value out of bounds, expected between {}-{}, got {}",
                min,
                max,
                se
            ))
        } else {
            Ok(U::try_from(se).map_err(|_| anyhow!("Conversion error"))?)
        }
    }

    fn get_byte(&mut self) -> Result<u8, GetByteError> {
        self.data.read_u8().map_err(|_| GetByteError::OutOfBits)
    }

    fn update_curr_byte(&mut self) -> Result<(), GetByteError> {
        let mut byte = self.get_byte()?;

        if self.prev_two_bytes == 0 && byte == 0x03 {
            // We found an epb
            self.num_epb += 1;
            // Read another byte
            byte = self.get_byte()?;
            // We need another 3 bytes before another epb can happen.
            self.prev_two_bytes = 0xffff;
        }

        self.num_remaining_bits_in_curr_byte = 8;
        self.prev_two_bytes = (self.prev_two_bytes << 8) | u16::from(byte);

        self.curr_byte = byte;
        Ok(())
    }
}
