// GStreamer RTP KLV Metadata Utility Functions
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// Limit max KLV unit size allowed for now
const MAX_KLV_UNIT_LEN_ALLOWED: u64 = 32 * 1024 * 1024;

/// Errors that can be produced when peeking at KLV units
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub(crate) enum KlvError {
    #[error("Unexpected KLV unit length length {}", 0)]
    UnexpectedLengthLength(usize),

    #[error("Unexpectedly large KLV unit ({}), max allowed {}", 0, 1)]
    UnitTooLarge(u64, u64),

    #[error("Invalid header: {reason}")]
    InvalidHeader { reason: &'static str },
}

// Or maybe return struct with named fields instead of tuple?
fn peek_klv_len(data: &[u8]) -> Result<(usize, usize), KlvError> {
    use KlvError::*;

    // Size already checked by caller peek_klv()
    let first_byte = data[0];

    if first_byte & 0x80 == 0 {
        return Ok((1, first_byte as usize));
    }

    let len_len = (first_byte & 0x7f) as usize;

    if len_len == 0 || len_len > 8 || data.len() < (1 + len_len) {
        Err(UnexpectedLengthLength(len_len))?;
    }

    let len = data[1..=len_len]
        .iter()
        .fold(0u64, |acc, &elem| (acc << 8) + elem as u64);

    assert!(MAX_KLV_UNIT_LEN_ALLOWED <= usize::MAX as u64);

    // Check length in u64 before casting to usize (which might only be 4 bytes on some arches)
    if len > MAX_KLV_UNIT_LEN_ALLOWED {
        Err(UnitTooLarge(len, MAX_KLV_UNIT_LEN_ALLOWED))?;
    }

    let len = len as usize;

    Ok((len_len + 1, len))
}

pub(crate) fn peek_klv(data: &[u8]) -> anyhow::Result<usize> {
    use anyhow::Context;
    use KlvError::*;

    if data.len() < 17 {
        Err(InvalidHeader {
            reason: "Not enough data",
        })?;
    }

    if !data.starts_with(&[0x06, 0x0e, 0x2b, 0x34]) {
        Err(InvalidHeader {
            reason: "No KLV Universal Label start code",
        })?;
    }

    // UL Designator byte values shall be limited to the range 0x01 to 0x7F
    if data[4..8].iter().any(|&b| b > 0x7f) {
        Err(InvalidHeader {
            reason: "Invalid KLV Universal Label designator",
        })?;
    }

    let (len_len, value_len) = peek_klv_len(&data[16..]).context("length")?;

    Ok(16 + len_len + value_len)
}
