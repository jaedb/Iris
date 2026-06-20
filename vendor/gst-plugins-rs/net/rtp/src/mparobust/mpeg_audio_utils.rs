// GStreamer RTP MPEG Audio Utility Functions
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#[derive(Debug, Clone)]
pub(crate) struct FrameHeader {
    pub sample_rate: u16,
    pub channels: u8,
    pub layer: u8,
    pub version: u8,
    pub frame_len: usize,
    pub samples_per_frame: u16,
}

impl PartialEq for FrameHeader {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate == other.sample_rate
            && self.channels == other.channels
            && self.layer == other.layer
            && self.version == other.version
    }
}

pub(crate) fn peek_frame_header(data: &[u8]) -> Result<FrameHeader, ()> {
    if data.len() < 4 {
        return Err(());
    }

    let sync_hdr = u16::from_be_bytes([data[0], data[1]]) >> 5;

    if sync_hdr != 0b11111111111 {
        return Err(());
    }

    let mpeg_version = {
        let mpeg_version_bits = (data[1] >> 3) & 0b11;

        match mpeg_version_bits {
            0b00 => 3, // MPEG 2.5
            0b10 => 2,
            0b11 => 1,
            _ => return Err(()),
        }
    };

    #[allow(clippy::unusual_byte_groupings)]
    let layer = {
        let layer_bits = (data[1] & 0b000_00_11_0) >> 1;

        match layer_bits {
            0b01 => 3,
            0b10 => 2,
            0b11 => 1,
            _ => return Err(()),
        }
    };

    let lsf = (mpeg_version > 1) as u32; // low sampling frequencies (MPEG-2 part 3 / MPEG 2.5)

    let bitrate = {
        let bitrate_idx = (data[2] >> 4) as usize;

        if bitrate_idx == 0b1111 {
            return Err(());
        }

        let bitrate_table = match (mpeg_version, layer) {
            (1, 1) => [
                0u32, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
            ],
            (1, 2) => [
                0u32, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
            ],
            (1, 3) => [
                0u32, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
            ],
            (2..=3, 1) => [
                0u32, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
            ],
            (2..=3, 2..=3) => [
                0u32, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160,
            ],
            _ => unreachable!(),
        };

        bitrate_table[bitrate_idx] * 1000
    };

    let sample_rate = {
        let freq_idx = ((data[2] >> 2) & 0b11) as usize;

        if freq_idx == 0b11 {
            return Err(());
        }

        match mpeg_version {
            1 => [44100u32, 48000, 32000][freq_idx],
            2 => [22050u32, 24000, 16000][freq_idx],
            3 => [11025u32, 12000, 8000][freq_idx], // MPEG 2.5
            _ => unreachable!(),
        }
    };

    let channels = {
        let channel_bits = ((data[3] & 0b1100_0000) >> 6) as u32;

        if channel_bits == 0b11 {
            1
        } else {
            2
        }
    };

    let frame_len = if bitrate != 0 {
        let padding = ((data[2] >> 1) & 1) as u32;

        match layer {
            1 => 4 * ((bitrate * 12) / sample_rate + padding),
            2 => (bitrate * 144) / sample_rate + padding,
            3 => (bitrate * 144) / (sample_rate << lsf) + padding,
            _ => unreachable!(),
        }
    } else {
        // Free format mp3: try to find another sync header, otherwise assume frame is all data left.
        // Ignore the padding flag when looking for a matching sync header.
        data[3..]
            .windows(4)
            .enumerate()
            .find(|(_, w)| {
                w[0] == 0xff
                    && w[1] == data[1]
                    && (w[2] & 0b11111101) == (data[2] & 0b11111101)
                    && w[3] == data[3]
            })
            .map(|(i, _)| 3 + i)
            .unwrap_or(data.len()) as u32
    };

    let samples_per_frame = match layer {
        1 => 384,
        2 => 1152,
        3 => match mpeg_version {
            1 => 1152,
            2 | 3 => 576,
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };

    let frame_hdr = FrameHeader {
        sample_rate: sample_rate as u16,
        channels: channels as u8,
        layer: layer as u8,
        version: mpeg_version as u8,
        frame_len: frame_len as usize,
        samples_per_frame,
    };

    Ok(frame_hdr)
}
