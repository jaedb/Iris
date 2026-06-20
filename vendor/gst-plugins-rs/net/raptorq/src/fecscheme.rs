// Copyright (C) 2022 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub const MAX_SOURCE_BLOCK_LEN: usize = 56403;
pub const MAX_ENCODING_SYMBOL_SIZE: usize = 65536;

// RFC6681, section 8.1.1.1
pub const FEC_SCHEME_ID: u32 = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataUnitHeader {
    pub flow_indication: u8,
    pub len_indication: u16,
}

// RFC6881, section 5
impl DataUnitHeader {
    pub fn encode(&self) -> [u8; 3] {
        let mut bytes: [u8; 3] = [0; 3];

        bytes[0] = self.flow_indication;
        bytes[1..3].copy_from_slice(&self.len_indication.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: [u8; 3]) -> Self {
        Self {
            flow_indication: bytes[0],
            len_indication: u16::from_be_bytes([bytes[1], bytes[2]]),
        }
    }
}

// RFC6881, section 8.1.3
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairPayloadId {
    pub initial_sequence_num: u16,
    pub source_block_len: u16,
    pub encoding_symbol_id: u32, // 24 bits
}

impl RepairPayloadId {
    pub fn encode(&self) -> [u8; 7] {
        let mut bytes: [u8; 7] = [0; 7];

        bytes[0..2].copy_from_slice(&self.initial_sequence_num.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.source_block_len.to_be_bytes());
        bytes[4..7].copy_from_slice(&self.encoding_symbol_id.to_be_bytes()[1..4]);
        bytes
    }

    pub fn decode(bytes: [u8; 7]) -> Self {
        Self {
            initial_sequence_num: u16::from_be_bytes([bytes[0], bytes[1]]),
            source_block_len: u16::from_be_bytes([bytes[2], bytes[3]]),
            encoding_symbol_id: u32::from_be_bytes([0, bytes[4], bytes[5], bytes[6]]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repair_payload_encode() {
        let payload_id = RepairPayloadId {
            initial_sequence_num: 42,
            source_block_len: 43,
            encoding_symbol_id: 44,
        };

        let encoded = payload_id.encode();
        assert_eq!(encoded.len(), 7);

        let decoded = RepairPayloadId::decode(encoded);
        assert_eq!(payload_id, decoded);
    }

    #[test]
    fn test_unit_data_header_encode() {
        let header = DataUnitHeader {
            flow_indication: 42,
            len_indication: 43,
        };

        let encoded = header.encode();
        assert_eq!(encoded.len(), 3);

        let decoded = DataUnitHeader::decode(encoded);
        assert_eq!(header, decoded);
    }
}
