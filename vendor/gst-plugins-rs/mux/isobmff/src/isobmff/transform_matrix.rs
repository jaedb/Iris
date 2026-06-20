// Copyright (C) 2021-2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::subclass::prelude::*;

use crate::isobmff::CAT;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TransformMatrix([[u8; 4]; 9]);

impl std::ops::Deref for TransformMatrix {
    type Target = [[u8; 4]; 9];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for &TransformMatrix {
    fn default() -> &'static TransformMatrix {
        &IDENTITY_MATRIX
    }
}

impl TransformMatrix {
    pub fn from_tag(obj: &impl ObjectSubclass, tag: &gst::event::Tag) -> &'static TransformMatrix {
        gst_video::VideoOrientationMethod::from_tag(tag.tag()).map_or(Default::default(), {
            |orientation| match orientation {
                gst_video::VideoOrientationMethod::Identity => &IDENTITY_MATRIX,
                gst_video::VideoOrientationMethod::_90r => &ROTATE_90R_MATRIX,
                gst_video::VideoOrientationMethod::_180 => &ROTATE_180_MATRIX,
                gst_video::VideoOrientationMethod::_90l => &ROTATE_90L_MATRIX,
                gst_video::VideoOrientationMethod::Horiz => &FLIP_HORZ_MATRIX,
                gst_video::VideoOrientationMethod::Vert => &FLIP_VERT_MATRIX,
                gst_video::VideoOrientationMethod::UrLl => &FLIP_ROTATE_90R_MATRIX,
                gst_video::VideoOrientationMethod::UlLr => &FLIP_ROTATE_90L_MATRIX,
                _ => {
                    gst::info!(
                        CAT,
                        imp = obj,
                        "Orientation {:?} not yet supported",
                        orientation
                    );
                    &IDENTITY_MATRIX
                }
            }
        })
    }
}

macro_rules! tm {
    ( $($v:expr),* ) => {
        TransformMatrix([
            $(
                (($v << 16) as i32).to_be_bytes(),
            )*
        ])
    }
}

// Point (p, q, 1) -> (p', q')
// Matrix (a, b, u,
//         c, d, v,
//         x, y, w)
// Where a, b, c, d, x, y are FP 16.16 and u, v, w are FP 2.30
// m = ap + cq + x
// n = bp + dq + y
// z = up + vq + w
// p' = m/z
// q' = n/z
#[rustfmt::skip]
pub const IDENTITY_MATRIX: TransformMatrix = tm!(1, 0, 0,
                                                 0, 1, 0,
                                                 0, 0, (1 << 14));
#[rustfmt::skip]
const FLIP_VERT_MATRIX: TransformMatrix = tm!(1,  0, 0,
                                              0, -1, 0,
                                              0,  0, (1 << 14));
#[rustfmt::skip]
const FLIP_HORZ_MATRIX: TransformMatrix = tm!(-1, 0, 0,
                                               0, 1, 0,
                                               0, 0, (1 << 14));
#[rustfmt::skip]
const ROTATE_90R_MATRIX: TransformMatrix = tm!( 0, 1, 0,
                                               -1, 0, 0,
                                                0, 0, (1 << 14));
#[rustfmt::skip]
const ROTATE_180_MATRIX: TransformMatrix = tm!(-1,  0, 0,
                                                0, -1, 0,
                                                0,  0, (1 << 14));
#[rustfmt::skip]
const ROTATE_90L_MATRIX: TransformMatrix = tm!(0, -1, 0,
                                               1,  0, 0,
                                               0,  0, (1 << 14));
#[rustfmt::skip]
const FLIP_ROTATE_90R_MATRIX: TransformMatrix = tm!( 0, -1, 0,
                                                    -1,  0, 0,
                                                     0,  0, (1 << 14));
#[rustfmt::skip]
const FLIP_ROTATE_90L_MATRIX: TransformMatrix = tm!(0, 1, 0,
                                                    1, 0, 0,
                                                    0, 0, (1 << 14));
