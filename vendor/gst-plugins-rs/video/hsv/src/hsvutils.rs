// Copyright (C) 2020 Julien Bardagi <julien.bardagi@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Reference used for implementation: https://en.wikipedia.org/wiki/HSL_and_HSV

// Since the standard 'clamp' feature is still considered unstable, we provide
// a subsititute implementation here so we can still build with the stable toolchain.
// Source: https://github.com/rust-lang/rust/issues/44095#issuecomment-624879262
pub trait Clamp: Sized {
    fn clamp<L, U>(self, lower: L, upper: U) -> Self
    where
        L: Into<Option<Self>>,
        U: Into<Option<Self>>;
}

impl Clamp for f32 {
    fn clamp<L, U>(self, lower: L, upper: U) -> Self
    where
        L: Into<Option<Self>>,
        U: Into<Option<Self>>,
    {
        let below = match lower.into() {
            None => self,
            Some(lower) => self.max(lower),
        };
        match upper.into() {
            None => below,
            Some(upper) => below.min(upper),
        }
    }
}

const EPSILON: f32 = 0.00001;

// Converts a RGB pixel to HSV
#[inline]
pub fn from_rgb(in_p: &[u8; 3]) -> [f32; 3] {
    let r = in_p[0] as f32 / 255.0;
    let g = in_p[1] as f32 / 255.0;
    let b = in_p[2] as f32 / 255.0;

    let value: f32 = *in_p
        .iter()
        .max()
        .expect("Cannot find max value from rgb input") as f32
        / 255.0;
    let chroma: f32 = value
        - (*in_p
            .iter()
            .min()
            .expect("Cannot find min value from rgb input") as f32
            / 255.0);

    let mut hue: f32 = if chroma == 0.0 {
        0.0
    } else if (value - r).abs() < EPSILON {
        60.0 * ((g - b) / chroma)
    } else if (value - g).abs() < EPSILON {
        60.0 * (2.0 + ((b - r) / chroma))
    } else if (value - b).abs() < EPSILON {
        60.0 * (4.0 + ((r - g) / chroma))
    } else {
        0.0
    };

    if hue < 0.0 {
        hue += 360.0;
    }

    let saturation: f32 = if value == 0.0 { 0.0 } else { chroma / value };

    [
        hue % 360.0,
        saturation.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    ]
}

// Converts a BGR pixel to HSV
#[inline]
pub fn from_bgr(in_p: &[u8; 3]) -> [f32; 3] {
    let b = in_p[0] as f32 / 255.0;
    let g = in_p[1] as f32 / 255.0;
    let r = in_p[2] as f32 / 255.0;

    let value: f32 = *in_p
        .iter()
        .max()
        .expect("Cannot find max value from rgb input") as f32
        / 255.0;
    let chroma: f32 = value
        - (*in_p
            .iter()
            .min()
            .expect("Cannot find min value from rgb input") as f32
            / 255.0);

    let mut hue: f32 = if chroma == 0.0 {
        0.0
    } else if (value - r).abs() < EPSILON {
        60.0 * ((g - b) / chroma)
    } else if (value - g).abs() < EPSILON {
        60.0 * (2.0 + ((b - r) / chroma))
    } else if (value - b).abs() < EPSILON {
        60.0 * (4.0 + ((r - g) / chroma))
    } else {
        0.0
    };

    if hue < 0.0 {
        hue += 360.0;
    }

    let saturation: f32 = if value == 0.0 { 0.0 } else { chroma / value };

    [
        hue % 360.0,
        saturation.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    ]
}

// Converts a HSV pixel to RGB
#[inline]
pub fn to_rgb(in_p: &[f32; 3]) -> [u8; 3] {
    let c: f32 = in_p[2] * in_p[1];
    let hue_prime: f32 = in_p[0] / 60.0;

    let x: f32 = c * (1.0 - ((hue_prime % 2.0) - 1.0).abs());

    let rgb_prime = if hue_prime < 0.0 {
        [0.0, 0.0, 0.0]
    } else if hue_prime <= 1.0 {
        [c, x, 0.0]
    } else if hue_prime <= 2.0 {
        [x, c, 0.0]
    } else if hue_prime <= 3.0 {
        [0.0, c, x]
    } else if hue_prime <= 4.0 {
        [0.0, x, c]
    } else if hue_prime <= 5.0 {
        [x, 0.0, c]
    } else if hue_prime <= 6.0 {
        [c, 0.0, x]
    } else {
        [0.0, 0.0, 0.0]
    };

    let m = in_p[2] - c;

    [
        ((rgb_prime[0] + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((rgb_prime[1] + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((rgb_prime[2] + m) * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

// Converts a HSV pixel to RGB
#[inline]
pub fn to_bgr(in_p: &[f32; 3]) -> [u8; 3] {
    let c: f32 = in_p[2] * in_p[1];
    let hue_prime: f32 = in_p[0] / 60.0;

    let x: f32 = c * (1.0 - ((hue_prime % 2.0) - 1.0).abs());

    let rgb_prime = if hue_prime < 0.0 {
        [0.0, 0.0, 0.0]
    } else if hue_prime <= 1.0 {
        [c, x, 0.0]
    } else if hue_prime <= 2.0 {
        [x, c, 0.0]
    } else if hue_prime <= 3.0 {
        [0.0, c, x]
    } else if hue_prime <= 4.0 {
        [0.0, x, c]
    } else if hue_prime <= 5.0 {
        [x, 0.0, c]
    } else if hue_prime <= 6.0 {
        [c, 0.0, x]
    } else {
        [0.0, 0.0, 0.0]
    };

    let m = in_p[2] - c;

    [
        ((rgb_prime[2] + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((rgb_prime[1] + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((rgb_prime[0] + m) * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

#[cfg(test)]
mod tests {

    fn is_equivalent(hsv: &[f32; 3], expected: &[f32; 3], eps: f32) -> bool {
        // We handle hue being circular here
        let ref_hue_offset = 180.0 - expected[0];
        let mut shifted_hue = hsv[0] + ref_hue_offset;

        if shifted_hue < 0.0 {
            shifted_hue += 360.0;
        }

        shifted_hue %= 360.0;

        (shifted_hue - 180.0).abs() < eps
            && (hsv[1] - expected[1]).abs() < eps
            && (hsv[2] - expected[2]).abs() < eps
    }

    const RGB_WHITE: [u8; 3] = [255, 255, 255];
    const RGB_BLACK: [u8; 3] = [0, 0, 0];
    const RGB_RED: [u8; 3] = [255, 0, 0];
    const RGB_GREEN: [u8; 3] = [0, 255, 0];
    const RGB_BLUE: [u8; 3] = [0, 0, 255];

    const BGR_WHITE: [u8; 3] = [255, 255, 255];
    const BGR_BLACK: [u8; 3] = [0, 0, 0];
    const BGR_RED: [u8; 3] = [0, 0, 255];
    const BGR_GREEN: [u8; 3] = [0, 255, 0];
    const BGR_BLUE: [u8; 3] = [255, 0, 0];

    const HSV_WHITE: [f32; 3] = [0.0, 0.0, 1.0];
    const HSV_BLACK: [f32; 3] = [0.0, 0.0, 0.0];
    const HSV_RED: [f32; 3] = [0.0, 1.0, 1.0];
    const HSV_GREEN: [f32; 3] = [120.0, 1.0, 1.0];
    const HSV_BLUE: [f32; 3] = [240.0, 1.0, 1.0];

    #[test]
    fn test_from_rgb() {
        use super::*;

        assert!(is_equivalent(&from_rgb(&RGB_WHITE), &HSV_WHITE, EPSILON));
        assert!(is_equivalent(&from_rgb(&RGB_BLACK), &HSV_BLACK, EPSILON));
        assert!(is_equivalent(&from_rgb(&RGB_RED), &HSV_RED, EPSILON));
        assert!(is_equivalent(&from_rgb(&RGB_GREEN), &HSV_GREEN, EPSILON));
        assert!(is_equivalent(&from_rgb(&RGB_BLUE), &HSV_BLUE, EPSILON));
    }

    #[test]
    fn test_from_bgr() {
        use super::*;

        assert!(is_equivalent(&from_bgr(&BGR_WHITE), &HSV_WHITE, EPSILON));
        assert!(is_equivalent(&from_bgr(&BGR_BLACK), &HSV_BLACK, EPSILON));
        assert!(is_equivalent(&from_bgr(&BGR_RED), &HSV_RED, EPSILON));
        assert!(is_equivalent(&from_bgr(&BGR_GREEN), &HSV_GREEN, EPSILON));
        assert!(is_equivalent(&from_bgr(&BGR_BLUE), &HSV_BLUE, EPSILON));
    }

    #[test]
    fn test_to_rgb() {
        use super::*;

        assert!(to_rgb(&HSV_WHITE) == RGB_WHITE);
        assert!(to_rgb(&HSV_BLACK) == RGB_BLACK);
        assert!(to_rgb(&HSV_RED) == RGB_RED);
        assert!(to_rgb(&HSV_GREEN) == RGB_GREEN);
        assert!(to_rgb(&HSV_BLUE) == RGB_BLUE);
    }

    #[test]
    fn test_to_bgr() {
        use super::*;

        assert!(to_bgr(&HSV_WHITE) == BGR_WHITE);
        assert!(to_bgr(&HSV_BLACK) == BGR_BLACK);
        assert!(to_bgr(&HSV_RED) == BGR_RED);
        assert!(to_bgr(&HSV_GREEN) == BGR_GREEN);
        assert!(to_bgr(&HSV_BLUE) == BGR_BLUE);
    }
}
