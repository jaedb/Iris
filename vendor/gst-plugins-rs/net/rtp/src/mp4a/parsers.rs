// GStreamer MPEG-4 Audio bitstream parsers.
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::Context;
use bitstream_io::{BitRead, FromBitStream};
use gst::prelude::*;

const ACC_SAMPLING_FREQS: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// Errors that can be produced when parsing a `StreamMuxConfig` & `AudioSpecificConfig`.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum MPEG4AudioParserError {
    #[error("Unknown audioMuxVersion 1. Expected 0.")]
    UnknownVersion,

    #[error("Unsupported: num_progs {num_progs}, num_layers {num_layers}")]
    UnsupportedProgsLayer { num_progs: u8, num_layers: u8 },

    #[error("Invalid audio object type 0")]
    InvalidAudioObjectType0,

    #[error("Invalid sampling frequency idx {}", 0)]
    InvalidSamplingFreqIdx(u8),

    #[error("Invalid channels {}", .0)]
    InvalidChannels(u8),

    #[error("subframe {} with len 0", .0)]
    ZeroLengthSubframe(u8),

    #[error("Wrong frame size. Required {required}, available {available}")]
    WrongFrameSize { required: usize, available: usize },

    #[error("Unsupported Profile {profile}")]
    UnsupportedProfile { profile: String },

    #[error("Unsupported Level {level} for Profile {profile}")]
    UnsupportedLevel { level: String, profile: String },
}

impl MPEG4AudioParserError {
    pub fn is_zero_length_subframe(&self) -> bool {
        matches!(self, MPEG4AudioParserError::ZeroLengthSubframe(_))
    }
}

/// StreamMuxConfig (partial) - ISO/IEC 14496-3 sub 1 table 1.21
/// Support for:
///
/// * allStreamsSameTimeFraming == true
/// * 1 prog & 1 layer only => flatten
#[derive(Debug)]
pub struct StreamMuxConfig {
    pub num_sub_frames: u8,
    pub prog: AudioSpecificConfig,
}

impl FromBitStream for StreamMuxConfig {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> anyhow::Result<Self> {
        use MPEG4AudioParserError::*;

        // StreamMuxConfig - ISO/IEC 14496-3 sub 1 table 1.21
        // audioMuxVersion           == 0 (1 bit)
        // allStreamsSameTimeFraming == 1 (1 bit)
        // numSubFrames              == 0 means 1 subframe (6 bits)
        // numProgram                == 0 means 1 program (4 bits)
        // numLayer                  == 0 means 1 layer (3 bits)

        if r.read::<1, u8>().context("audioMuxVersion")? != 0 {
            Err(UnknownVersion)?;
        }

        let _ = r.read_bit().context("allStreamsSameTimeFraming")?;
        let num_sub_frames = r.read::<6, u8>().context("numSubFrames")? + 1;

        let num_progs = r.read::<4, u8>().context("numProgram")? + 1;
        let num_layers = r.read::<3, u8>().context("numLayer")? + 1;
        if !(num_progs == 1 && num_layers == 1) {
            // Same as for rtpmp4adepay
            Err(UnsupportedProgsLayer {
                num_progs,
                num_layers,
            })?;
        }

        // AudioSpecificConfig - ISO/IEC 14496-3 sub 1 table 1.8
        let prog = r.parse::<AudioSpecificConfig>().context("prog 1 layer 1")?;

        // Ignore remaining bits for now

        Ok(StreamMuxConfig {
            num_sub_frames,
            prog,
        })
    }
}

/// AudioSpecificConfig - ISO/IEC 14496-3 sub 1 table 1.8
///
/// Support for:
///
/// * allStreamsSameTimeFraming == true
/// * 1 prog & 1 layer only => flatten
#[derive(Debug)]
pub struct AudioSpecificConfig {
    pub audio_object_type: u8,
    pub sampling_freq: u32,
    pub channel_conf: u8,
    /// GASpecificConfig (partial) - ISO/IEC 14496-3 sub 4 table 4.1
    pub frame_len: usize,
}

impl FromBitStream for AudioSpecificConfig {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> anyhow::Result<Self> {
        use MPEG4AudioParserError::*;

        let audio_object_type = r.read::<5, _>().context("audioObjectType")?;
        if audio_object_type == 0 {
            Err(InvalidAudioObjectType0)?;
        }

        let sampling_freq_idx = r.read::<4, u8>().context("samplingFrequencyIndex")?;
        if sampling_freq_idx as usize >= ACC_SAMPLING_FREQS.len() && sampling_freq_idx != 0xf {
            Err(InvalidSamplingFreqIdx(sampling_freq_idx))?;
        }

        // RTP rate depends on sampling freq of the audio
        let sampling_freq = if sampling_freq_idx == 0xf {
            r.read::<24, _>().context("samplingFrequency")?
        } else {
            ACC_SAMPLING_FREQS[sampling_freq_idx as usize]
        };

        let channel_conf = r.read::<4, _>().context("channelConfiguration")?;
        if channel_conf > 7 {
            Err(InvalidChannels(channel_conf))?;
        }

        // GASpecificConfig - ISO/IEC 14496-3 sub 4 table 4.1

        // TODO this is based on ISO/IEC 14496-3:2001 as implemented in rtpmp4adepay
        // and should be updated with enhancements from ISO/IEC 14496-3:2009
        // for AAC SBR & HE support.

        let frame_len = if [1, 2, 3, 4, 6, 7].contains(&audio_object_type)
            && r.read_bit().context("frame_len_flag")?
        {
            960
        } else {
            1024
        };

        // Ignore remaining bits for now

        Ok(AudioSpecificConfig {
            audio_object_type,
            sampling_freq,
            channel_conf,
            frame_len,
        })
    }
}

/// audioProfileLevelIndication - ISO/IEC 14496-3 (2009) table 1.14
pub struct ProfileLevel {
    pub profile: String,
    pub level: String,
    pub id: u8,
}

impl ProfileLevel {
    pub fn from_caps(s: &gst::StructureRef) -> anyhow::Result<ProfileLevel> {
        // Note: could use an AudioSpecificConfig based approach
        //       similar to what is done in gst_codec_utils_aac_get_level
        //       from gst-plugins-base/gst-libs/gst/pbutils/codec-utils.c

        use MPEG4AudioParserError::*;

        let profile = s.get::<String>("profile").context("profile")?;
        let level = s.get::<String>("level").context("level")?;

        let id = match profile.to_lowercase().as_str() {
            "lc" => {
                // Assumed to be AAC Profile in table 1.14
                match level.as_str() {
                    "1" => 0x28,
                    "2" => 0x29,
                    "4" => 0x2a,
                    "5" => 0x2b,
                    _ => Err(UnsupportedLevel {
                        level: level.clone(),
                        profile: profile.clone(),
                    })?,
                }
            }
            "he-aac" | "he-aac-v1" => {
                // High Efficiency AAC Profile in table 1.14
                match level.as_str() {
                    "2" => 0x2c,
                    "3" => 0x2d,
                    "4" => 0x2e,
                    "5" => 0x2f,
                    _ => Err(UnsupportedLevel {
                        level: level.clone(),
                        profile: profile.clone(),
                    })?,
                }
            }
            "he-aac-v2" => {
                // High Efficiency AAC v2 Profile in table 1.14
                match level.as_str() {
                    "2" => 0x30,
                    "3" => 0x31,
                    "4" => 0x32,
                    "5" => 0x33,
                    _ => Err(UnsupportedLevel {
                        level: level.clone(),
                        profile: profile.clone(),
                    })?,
                }
            }
            _ => Err(UnsupportedProfile {
                profile: profile.clone(),
            })?,
        };

        Ok(ProfileLevel { profile, level, id })
    }
}

#[derive(Debug)]
pub struct Subframes<'a> {
    frame: gst::MappedBuffer<gst::buffer::Readable>,
    pos: usize,
    subframe_idx: u8,
    config: &'a StreamMuxConfig,
}

impl<'a> Subframes<'a> {
    pub fn new<F>(frame: F, config: &'a StreamMuxConfig) -> Self
    where
        F: AsRef<[u8]> + Send + 'static,
    {
        Subframes {
            frame: gst::Buffer::from_slice(frame)
                .into_mapped_buffer_readable()
                .unwrap(),
            pos: 0,
            subframe_idx: 0,
            config,
        }
    }
}

impl Iterator for Subframes<'_> {
    type Item = Result<gst::Buffer, MPEG4AudioParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        use MPEG4AudioParserError::*;

        if self.subframe_idx >= self.config.num_sub_frames {
            return None;
        }

        self.subframe_idx += 1;

        let mut data_len: usize;
        let buf = &self.frame[self.pos..];

        // PayloadLengthInfo - ISO/IEC 14496-3 sub 1 table 1.22
        // Assuming:
        // * allStreamsSameTimeFraming == true
        // * 1 prog & 1 layer
        // * frameLengthType == 0

        data_len = 0;
        for byte in buf.iter() {
            data_len += *byte as usize;
            self.pos += 1;
            if *byte != 0xff {
                break;
            }
        }

        if data_len == 0 {
            return Some(Err(ZeroLengthSubframe(self.subframe_idx)));
        }

        if data_len > buf.len() {
            return Some(Err(WrongFrameSize {
                required: self.pos + data_len,
                available: self.pos + buf.len(),
            }));
        }

        let mut subframe = self
            .frame
            .buffer()
            .copy_region(
                gst::BufferCopyFlags::MEMORY,
                self.pos..(self.pos + data_len),
            )
            .expect("Failed to create subbuffer");

        let duration = (self.config.prog.frame_len as u64)
            .mul_div_floor(
                *gst::ClockTime::SECOND,
                self.config.prog.sampling_freq as u64,
            )
            .map(gst::ClockTime::from_nseconds);

        if let Some(duration) = duration {
            subframe.get_mut().unwrap().set_duration(duration);
        }

        self.pos += data_len;

        Some(Ok(subframe))
    }
}
