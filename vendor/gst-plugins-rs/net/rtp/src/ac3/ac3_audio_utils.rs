// GStreamer RTP AC-3 Audio Utility Functions
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::MulDiv;

const AC3_SAMPLES_PER_FRAME: u16 = 1536;

#[derive(Debug, Clone)]
pub(crate) struct FrameHeader {
    pub channels: u16,
    pub sample_rate: u16,
    pub frame_len: usize,
}

impl PartialEq for FrameHeader {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate == other.sample_rate && self.channels == other.channels
    }
}

impl FrameHeader {
    pub(crate) fn duration(&self) -> u64 {
        let samples = AC3_SAMPLES_PER_FRAME as u64;
        let sample_rate = self.sample_rate as u64;

        samples
            .mul_div_ceil(*gst::ClockTime::SECOND, sample_rate)
            .unwrap()
    }
}

const FRAME_LENS_32000: [u16; 38] = [
    96u16, 96, 120, 120, 144, 144, 168, 168, 192, 192, 240, 240, 288, 288, 336, 336, 384, 384, 480,
    480, 576, 576, 672, 672, 768, 768, 960, 960, 1152, 1152, 1344, 1344, 1536, 1536, 1728, 1728,
    1920, 1920,
];

const FRAME_LENS_44100: [u16; 38] = [
    69u16, 70, 87, 88, 104, 105, 121, 122, 139, 140, 174, 175, 208, 209, 243, 244, 278, 279, 348,
    349, 417, 418, 487, 488, 557, 558, 696, 697, 835, 836, 975, 976, 1114, 1115, 1253, 1254, 1393,
    1394,
];

const FRAME_LENS_48000: [u16; 38] = [
    64u16, 64, 80, 80, 96, 96, 112, 112, 128, 128, 160, 160, 192, 192, 224, 224, 256, 256, 320,
    320, 384, 384, 448, 448, 512, 512, 640, 640, 768, 768, 896, 896, 1024, 1024, 1152, 1152, 1280,
    1280,
];

pub(crate) fn peek_frame_header(data: &[u8]) -> Result<FrameHeader, ()> {
    // Need sync info and start of bit stream info (bsi)
    if data.len() < 5 + 3 {
        return Err(());
    }

    let sync_hdr = u16::from_be_bytes([data[0], data[1]]);

    if sync_hdr != 0x0b77 {
        return Err(());
    }

    // skipping 2 bytes of CRC

    let (sample_rate, len_table) = {
        let fscod = (data[4] >> 6) & 0b11;

        match fscod {
            0b00 => (48000, &FRAME_LENS_48000),
            0b01 => (44100, &FRAME_LENS_44100),
            0b10 => (32000, &FRAME_LENS_32000),
            _ => return Err(()),
        }
    };

    let frame_len = {
        let frmsizcod = data[4] & 0b00111111;

        let len_words = len_table.get(frmsizcod as usize).ok_or(())?;

        len_words * 2
    };

    let bsi = &data[5..];

    let _bsid = bsi[0] >> 3;
    let _bsmod = bsi[0] & 0b00000111;

    let channels = {
        let bits = u16::from_be_bytes([bsi[1], bsi[2]]);

        let acmod = (bits >> 13) & 0b111;

        let (nfchans, skip_bits) = match acmod {
            0b000 => (2, 0), // 1+1, dual mono
            0b001 => (1, 0), // 1/0, center/mono
            0b010 => (2, 2), // 2/0, stereo
            0b011 => (3, 2), // 3/0, L C R
            0b100 => (3, 2), // 2/1, L R S
            0b101 => (4, 4), // 3/1, L C R S
            0b110 => (4, 2), // 2/2, L R Sl Sr
            0b111 => (5, 4), // 3/2, L C R Sl Sr
            _ => unreachable!(),
        };

        let lfe_on = ((bits << (3 + skip_bits)) & 0x8000) >> 15;

        nfchans + lfe_on
    };

    Ok(FrameHeader {
        channels,
        sample_rate: sample_rate as u16,
        frame_len: frame_len as usize,
    })
}
