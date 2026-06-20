// Copyright (C) 2019 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;

use crate::constants::{CDG_COMMAND, CDG_MASK, CDG_PACKET_PERIOD, CDG_PACKET_SIZE};
use gst::{Caps, TypeFind, TypeFindProbability};
use std::cmp;

const NB_WINDOWS: u64 = 8;
const TYPEFIND_SEARCH_WINDOW_SEC: i64 = 4;
const TYPEFIND_SEARCH_WINDOW: i64 =
    TYPEFIND_SEARCH_WINDOW_SEC * (CDG_PACKET_SIZE as i64 * CDG_PACKET_PERIOD as i64); /* in bytes */

/* Return the percentage of CDG packets in the first @len bytes of @typefind */
fn cdg_packets_ratio(typefind: &mut TypeFind, start: i64, len: i64) -> i64 {
    let mut count = 0;
    let total = len / CDG_PACKET_SIZE as i64;

    for offset in (start..len).step_by(CDG_PACKET_SIZE as usize) {
        match typefind.peek(offset, CDG_PACKET_SIZE as u32) {
            Some(data) => {
                if data[0] & CDG_MASK == CDG_COMMAND {
                    count += 1;
                }
            }
            None => break,
        }
    }
    if total == 0 {
        0
    } else {
        (count * 100) / total
    }
}

/* Some CDG files starts drawing right away and then pause for a while
 * (typically because of the song intro) while other wait for a few
 * seconds before starting to draw.
 * In order to support all variants, scan through all the file per block
 * of size TYPEFIND_SEARCH_WINDOW and keep the highest ratio of CDG packets
 * detected. */
fn compute_probability(typefind: &mut TypeFind) -> TypeFindProbability {
    let mut best = TypeFindProbability::None;
    // Try looking at the start of the file if its length isn't available
    let len = typefind
        .length()
        .unwrap_or(TYPEFIND_SEARCH_WINDOW as u64 * NB_WINDOWS);
    let step = len / NB_WINDOWS;
    let search_window = cmp::min(len as i64, TYPEFIND_SEARCH_WINDOW);

    // Too short file
    if step == 0 {
        return TypeFindProbability::None;
    }

    for offset in (0..len).step_by(step as usize) {
        let proba = match cdg_packets_ratio(typefind, offset as i64, search_window) {
            0..=5 => TypeFindProbability::None,
            6..=10 => TypeFindProbability::Possible,
            _ => TypeFindProbability::Likely,
        };

        if proba == TypeFindProbability::Likely {
            return proba;
        }

        best = cmp::max(best, proba);
    }

    best
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    TypeFind::register(
        Some(plugin),
        "cdg_typefind",
        gst::Rank::NONE,
        Some("cdg"),
        Some(&Caps::builder("video/x-cdg").build()),
        |typefind| {
            let proba = compute_probability(typefind);

            if proba != gst::TypeFindProbability::None {
                typefind.suggest(proba, &Caps::builder("video/x-cdg").build());
            }
        },
    )
}
