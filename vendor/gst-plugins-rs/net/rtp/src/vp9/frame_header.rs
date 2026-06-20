//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::{bail, Context as _};
use bitstream_io::{FromBitStream, FromBitStreamWith};

#[derive(Debug)]
#[allow(unused)]
pub struct FrameHeader {
    pub profile: u8,
    pub show_existing_frame: bool,
    // All below only set for !show_existing_frame
    pub is_keyframe: Option<bool>,
    pub show_frame: Option<bool>,
    pub error_resilient_mode: Option<bool>,
    pub keyframe_info: Option<KeyframeInfo>,
    // More fields follow that we don't parse
    // TODO: intra-only frames can also have a frame size
}

impl FromBitStream for FrameHeader {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let marker = r.read::<2, u8>().context("frame_marker")?;
        if marker != 2 {
            bail!("Wrong frame marker");
        }

        let profile_low_bit = r.read::<1, u8>().context("profile_low_bit")?;
        let profile_high_bit = r.read::<1, u8>().context("profile_high_bit")?;

        let profile = (profile_high_bit << 1) | profile_low_bit;
        if profile == 3 {
            r.skip(1).context("reserved")?;
        }

        let show_existing_frame = r.read_bit().context("show_existing_frame")?;
        if show_existing_frame {
            return Ok(FrameHeader {
                profile,
                show_existing_frame,
                is_keyframe: None,
                show_frame: None,
                error_resilient_mode: None,
                keyframe_info: None,
            });
        }

        let is_keyframe = !r.read_bit().context("frame_type")?;
        let show_frame = r.read_bit().context("show_frame")?;
        let error_resilient_mode = r.read_bit().context("error_resilient_mode")?;

        if !is_keyframe {
            return Ok(FrameHeader {
                profile,
                show_existing_frame,
                is_keyframe: Some(is_keyframe),
                show_frame: Some(show_frame),
                error_resilient_mode: Some(error_resilient_mode),
                keyframe_info: None,
            });
        }

        let keyframe_info = r
            .parse_with::<KeyframeInfo>(&profile)
            .context("keyframe_info")?;

        Ok(FrameHeader {
            profile,
            show_existing_frame,
            is_keyframe: Some(is_keyframe),
            show_frame: Some(show_frame),
            error_resilient_mode: Some(error_resilient_mode),
            keyframe_info: Some(keyframe_info),
        })
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct KeyframeInfo {
    // sync code
    // color config
    pub color_config: ColorConfig,
    // frame size
    pub frame_size: (u32, u32),
    // render size
    pub render_size: Option<(u32, u32)>,
}

impl FromBitStreamWith<'_> for KeyframeInfo {
    type Error = anyhow::Error;

    /// Profile
    type Context = u8;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(
        r: &mut R,
        profile: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let sync_code_1 = r.read_to::<u8>().context("sync_code_1")?;
        let sync_code_2 = r.read_to::<u8>().context("sync_code_2")?;
        let sync_code_3 = r.read_to::<u8>().context("sync_code_3")?;

        if [0x49, 0x83, 0x42] != [sync_code_1, sync_code_2, sync_code_3] {
            bail!("Invalid sync code");
        }

        let color_config = r
            .parse_with::<ColorConfig>(profile)
            .context("color_config")?;

        let frame_width_minus_1 = r.read_to::<u16>().context("frame_width_minus_1")?;
        let frame_height_minus_1 = r.read_to::<u16>().context("frame_height_minus_1")?;

        let frame_size = (
            frame_width_minus_1 as u32 + 1,
            frame_height_minus_1 as u32 + 1,
        );

        let render_and_frame_size_different = r
            .read::<1, u8>()
            .context("render_and_frame_size_different")?
            == 1;

        let render_size = if render_and_frame_size_different {
            let render_width_minus_1 = r.read_to::<u16>().context("render_width_minus_1")?;
            let render_height_minus_1 = r.read_to::<u16>().context("render_height_minus_1")?;
            Some((
                render_width_minus_1 as u32 + 1,
                render_height_minus_1 as u32 + 1,
            ))
        } else {
            None
        };

        Ok(KeyframeInfo {
            color_config,
            frame_size,
            render_size,
        })
    }
}

impl KeyframeInfo {
    pub fn render_size(&self) -> (u32, u32) {
        if let Some((width, height)) = self.render_size {
            (width, height)
        } else {
            self.frame_size
        }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct ColorConfig {
    pub bit_depth: u8,
    pub color_space: u8,
    pub color_range: u8,
    pub sub_sampling_x: u8,
    pub sub_sampling_y: u8,
}

impl FromBitStreamWith<'_> for ColorConfig {
    type Error = anyhow::Error;

    /// Profile
    type Context = u8;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(
        r: &mut R,
        profile: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        const CS_RGB: u8 = 7;

        let bit_depth = if *profile >= 2 {
            let ten_or_twelve_bit = r.read_bit().context("ten_or_twelve_bit")?;
            if ten_or_twelve_bit {
                12
            } else {
                10
            }
        } else {
            8
        };

        let color_space = r.read::<3, u8>().context("color_space")?;
        let (color_range, sub_sampling_x, sub_sampling_y) = if color_space != CS_RGB {
            let color_range = r.read::<1, u8>().context("color_range")?;

            let (sub_sampling_x, sub_sampling_y) = if *profile == 1 || *profile == 3 {
                let sub_sampling_x = r.read::<1, u8>().context("sub_sampling_x")?;
                let sub_sampling_y = r.read::<1, u8>().context("sub_sampling_y")?;
                r.skip(1).context("reserved_zero")?;

                (sub_sampling_x, sub_sampling_y)
            } else {
                (1, 1)
            };

            (color_range, sub_sampling_x, sub_sampling_y)
        } else {
            if *profile == 1 || *profile == 3 {
                r.skip(1).context("reserved_zero")?;
            }

            (1, 0, 0)
        };

        Ok(ColorConfig {
            bit_depth,
            color_space,
            color_range,
            sub_sampling_x,
            sub_sampling_y,
        })
    }
}
