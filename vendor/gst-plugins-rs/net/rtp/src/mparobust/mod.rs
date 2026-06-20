// SPDX-License-Identifier: MPL-2.0

pub mod depay;

pub(crate) mod mpeg_audio_utils;
pub(crate) use mpeg_audio_utils::{peek_frame_header, FrameHeader};

#[cfg(test)]
mod tests;
