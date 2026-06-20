// Copyright (C) 2022 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::HashAlgorithm;
#[cfg(feature = "dssim")]
use dssim_core::{Dssim, DssimImage};
use gst_video::{prelude::*, VideoFormat};
use image_hasher::{HashAlg, Hasher, HasherConfig, ImageHash};
#[cfg(feature = "dssim")]
use rgb::FromSlice;

pub enum HasherEngine {
    ImageHasher(Hasher),
    #[cfg(feature = "dssim")]
    DssimHasher(Dssim),
}

impl HasherEngine {
    pub fn hash_image(
        &self,
        frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
    ) -> Result<HashedImage, gst::FlowError> {
        use HasherEngine::*;

        let height = frame.height();
        let width = frame.width();
        let buf = tightly_packed_framebuffer(frame);

        let res = match self {
            ImageHasher(hasher) => match frame.format() {
                VideoFormat::Rgb => {
                    let frame_buf = image::RgbImage::from_raw(width, height, buf)
                        .ok_or(gst::FlowError::Error)?;
                    HashedImage::ImageHash(hasher.hash_image(&frame_buf))
                }
                VideoFormat::Rgba => {
                    let frame_buf = image::RgbaImage::from_raw(width, height, buf)
                        .ok_or(gst::FlowError::Error)?;
                    HashedImage::ImageHash(hasher.hash_image(&frame_buf))
                }
                _ => unreachable!(),
            },
            #[cfg(feature = "dssim")]
            DssimHasher(hasher) => {
                let hashed_img = match frame.format() {
                    VideoFormat::Rgb => hasher
                        .create_image_rgb(buf.as_rgb(), width as usize, height as usize)
                        .unwrap(),
                    VideoFormat::Rgba => hasher
                        .create_image_rgba(buf.as_rgba(), width as usize, height as usize)
                        .unwrap(),
                    _ => unreachable!(),
                };
                HashedImage::Dssim(hashed_img)
            }
        };

        Ok(res)
    }

    pub fn compare(&self, img1: &HashedImage, img2: &HashedImage) -> f64 {
        use HashedImage::*;

        match (self, img1, img2) {
            (_, ImageHash(left), ImageHash(right)) => left.dist(right) as f64,
            #[cfg(feature = "dssim")]
            (Self::DssimHasher(algo), Dssim(left), Dssim(right)) => {
                let (val, _) = algo.compare(left, right);
                val.into()
            }
            #[cfg(feature = "dssim")]
            _ => unreachable!(),
        }
    }
}

pub enum HashedImage {
    ImageHash(ImageHash),
    #[cfg(feature = "dssim")]
    Dssim(DssimImage<f32>),
}

impl From<super::HashAlgorithm> for HasherEngine {
    fn from(algo: HashAlgorithm) -> Self {
        use super::HashAlgorithm::*;

        let algo = match algo {
            Mean => HashAlg::Mean,
            Gradient => HashAlg::Gradient,
            VertGradient => HashAlg::VertGradient,
            DoubleGradient => HashAlg::DoubleGradient,
            Blockhash => HashAlg::Blockhash,
            #[cfg(feature = "dssim")]
            Dssim => {
                return HasherEngine::DssimHasher(dssim_core::Dssim::new());
            }
        };

        HasherEngine::ImageHasher(HasherConfig::new().hash_alg(algo).to_hasher())
    }
}

/// Helper method that takes a gstreamer video-frame and copies it into a
/// tightly packed rgb(a) buffer, ready for consumption.
fn tightly_packed_framebuffer(frame: &gst_video::VideoFrameRef<&gst::BufferRef>) -> Vec<u8> {
    assert_eq!(frame.n_planes(), 1); // RGB and RGBA are tightly packed
    let line_size = (frame.width() * frame.n_components()) as usize;
    let line_stride = frame.plane_stride()[0] as usize;

    if line_size == line_stride {
        return frame.plane_data(0).unwrap().to_vec();
    }

    let mut raw_frame = Vec::with_capacity(line_size * frame.info().height() as usize);

    // copy gstreamer frame to tightly packed rgb(a) frame.
    frame
        .plane_data(0)
        .unwrap()
        .chunks_exact(line_stride)
        .map(|padded_line| &padded_line[..line_size])
        .for_each(|line| raw_frame.extend_from_slice(line));

    raw_frame
}
