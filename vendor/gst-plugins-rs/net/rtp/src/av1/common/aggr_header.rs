//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct AggregationHeader {
    pub leading_fragment: bool,
    pub trailing_fragment: bool,
    pub obu_count: Option<u8>,
    pub start_of_seq: bool,
}

impl From<u8> for AggregationHeader {
    fn from(byte: u8) -> Self {
        Self {
            leading_fragment: byte & (1 << 7) != 0,
            trailing_fragment: byte & (1 << 6) != 0,
            obu_count: match (byte >> 4) & 0b11 {
                0 => None,
                n => Some(n),
            },
            start_of_seq: byte & (1 << 3) != 0,
        }
    }
}

impl From<&[u8; 1]> for AggregationHeader {
    fn from(slice: &[u8; 1]) -> Self {
        AggregationHeader::from(slice[0])
    }
}

impl From<AggregationHeader> for u8 {
    fn from(aggr: AggregationHeader) -> Self {
        let mut byte = 0;

        byte |= (aggr.leading_fragment as u8) << 7;
        byte |= (aggr.trailing_fragment as u8) << 6;
        byte |= (aggr.start_of_seq as u8) << 3;

        if let Some(n) = aggr.obu_count {
            assert!(n < 0b100, "OBU count out of range");
            byte |= n << 4;
        }

        byte
    }
}

#[cfg(test)]
mod tests {
    use crate::av1::common::*;

    const HEADERS: [(u8, AggregationHeader); 3] = [
        (
            0b01011000,
            AggregationHeader {
                leading_fragment: false,
                trailing_fragment: true,
                obu_count: Some(1),
                start_of_seq: true,
            },
        ),
        (
            0b11110000,
            AggregationHeader {
                leading_fragment: true,
                trailing_fragment: true,
                obu_count: Some(3),
                start_of_seq: false,
            },
        ),
        (
            0b10000000,
            AggregationHeader {
                leading_fragment: true,
                trailing_fragment: false,
                obu_count: None,
                start_of_seq: false,
            },
        ),
    ];

    #[test]
    fn test_aggr_header() {
        for (idx, (byte, header)) in HEADERS.into_iter().enumerate() {
            println!("running test {idx}...");

            assert_eq!(byte, header.into());
            assert_eq!(header, byte.into());
            assert_eq!(header, (&[byte; 1]).into());
        }
    }
}
