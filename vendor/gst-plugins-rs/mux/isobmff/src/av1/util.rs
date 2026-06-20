// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

pub(crate) fn av1_seq_level_idx(level: Option<&str>) -> u8 {
    match level {
        Some("2.0") => 0,
        Some("2.1") => 1,
        Some("2.2") => 2,
        Some("2.3") => 3,
        Some("3.0") => 4,
        Some("3.1") => 5,
        Some("3.2") => 6,
        Some("3.3") => 7,
        Some("4.0") => 8,
        Some("4.1") => 9,
        Some("4.2") => 10,
        Some("4.3") => 11,
        Some("5.0") => 12,
        Some("5.1") => 13,
        Some("5.2") => 14,
        Some("5.3") => 15,
        Some("6.0") => 16,
        Some("6.1") => 17,
        Some("6.2") => 18,
        Some("6.3") => 19,
        Some("7.0") => 20,
        Some("7.1") => 21,
        Some("7.2") => 22,
        Some("7.3") => 23,
        _ => 1,
    }
}

pub(crate) fn av1_tier(tier: Option<&str>) -> u8 {
    match tier {
        Some("main") => 0,
        Some("high") => 1,
        _ => 0,
    }
}
