// GStreamer RTP audio channel positions
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use byte_slice_cast::*;

use gst_audio::AudioChannelPosition;

pub(crate) const MAX_REORDER_CHANNELS: usize = 8;

// https://www.rfc-editor.org/rfc/rfc3551.html#section-4.1
const DEFAULT_1CH: &[gst_audio::AudioChannelPosition] = [
    // Mono
    AudioChannelPosition::Mono,
]
.as_slice();

const DEFAULT_2CH: &[gst_audio::AudioChannelPosition] = [
    // Stereo
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
]
.as_slice();

const DEFAULT_3CH: &[gst_audio::AudioChannelPosition] = [
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
]
.as_slice();

const DEFAULT_4CH: &[gst_audio::AudioChannelPosition] = [
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::Lfe1,
]
.as_slice();

const DEFAULT_5CH: &[gst_audio::AudioChannelPosition] = [
    // NOTE: These are guessed / best effort approximations
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::RearLeft,  // Or SideLeft?
    AudioChannelPosition::RearRight, // Or SideRight?
]
.as_slice();

const DEFAULT_6CH: &[gst_audio::AudioChannelPosition] = [
    // NOTE: These are guessed / best effort approximations
    AudioChannelPosition::FrontLeft,         // Or SideLeft?
    AudioChannelPosition::FrontLeftOfCenter, // Or FrontLeft?
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::FrontRight,         // Or SideRight?
    AudioChannelPosition::FrontRightOfCenter, // Or FrontRight?
    AudioChannelPosition::Lfe1,
]
.as_slice();

// https://www.rfc-editor.org/rfc/rfc3555.html#section-4.1.15
const DV_4CH_1: &[gst_audio::AudioChannelPosition] = [
    // DV.LRLsRs
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
]
.as_slice();

const DV_4CH_2: &[gst_audio::AudioChannelPosition] = [
    // DV.LRCS
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::RearCenter, // S - Surround
]
.as_slice();

const DV_4CH_3: &[gst_audio::AudioChannelPosition] = [
    // DV.LRCWo
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::Lfe1, // Wo - Woofer
]
.as_slice();

const DV_5CH_1: &[gst_audio::AudioChannelPosition] = [
    // DV.LRLsRsC
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
    AudioChannelPosition::FrontCenter,
]
.as_slice();

const DV_6CH_1: &[gst_audio::AudioChannelPosition] = [
    // DV.LRLsRsCS
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::RearCenter, // S - Surround
]
.as_slice();

const DV_6CH_2: &[gst_audio::AudioChannelPosition] = [
    // DV.LmixRmixTWoQ1Q2
    //
    // See https://www.itu.int/dms_pubrec/itu-r/rec/bs/R-REC-BS.775-1-199407-S!!PDF-E.pdf
    // Annex 3 for the actual meaning of T, Q1 and Q2 which we don't really support
    AudioChannelPosition::FrontLeft,   // Lmix is compatible
    AudioChannelPosition::FrontRight,  // Rmix is compatible
    AudioChannelPosition::FrontCenter, // T see above
    AudioChannelPosition::Lfe1,        // Wo - Woofer
    AudioChannelPosition::SideLeft,    // Q1 see above
    AudioChannelPosition::SideRight,   // Q2 see above
]
.as_slice();

const DV_8CH_1: &[gst_audio::AudioChannelPosition] = [
    // DV.LRCWoLsRsLmixRmix
    //
    // Lmix / Rmix have a specific meaning
    //  Lmix = L + 0.7071C + 0.7071LS
    //  Rmix = R + 0.7071C + 0.7071RS
    // We don't have such a thing in our channel positioning.
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::Lfe1, // Wo - Woofer
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
    AudioChannelPosition::RearLeft,  // Lmix
    AudioChannelPosition::RearRight, // Rmix
]
.as_slice();

const DV_8CH_2: &[gst_audio::AudioChannelPosition] = [
    // DV.LRCWoLs1Rs1Ls2Rs2
    // NOTE: These are guessed / best effort approximations
    //
    // Not sure what the difference between Ls1 and Ls2 is, we're guessing
    // here that Ls1/Rs1 are more towards the front and Ls2/Rs2 more towards
    // the rear.
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::Lfe1, // Wo - Woofer
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
    AudioChannelPosition::SurroundLeft,
    AudioChannelPosition::SurroundRight,
]
.as_slice();

const DV_8CH_3: &[gst_audio::AudioChannelPosition] = [
    // DV.LRCWoLsRsLcRc
    // NOTE: These are guessed / best effort approximations
    // Lc/Rc is probably FrontLeftOfCenter / FrontRightOfCenter?
    AudioChannelPosition::FrontLeft,
    AudioChannelPosition::FrontRight,
    AudioChannelPosition::FrontCenter,
    AudioChannelPosition::Lfe1,
    AudioChannelPosition::SideLeft,
    AudioChannelPosition::SideRight,
    AudioChannelPosition::RearLeft,
    AudioChannelPosition::RearRight,
]
.as_slice();

pub(crate) fn get_channel_order(
    name: Option<&str>,
    n_channels: i32,
) -> Option<&'static [gst_audio::AudioChannelPosition]> {
    assert!(n_channels > 0);

    let name = name.unwrap_or("default");

    // https://www.rfc-editor.org/rfc/rfc3551.html#section-4.1
    // https://www.rfc-editor.org/rfc/rfc3555.html#section-4.1.15
    match (n_channels, name) {
        // mono
        (1, _) => Some(DEFAULT_1CH),
        // stereo
        (2, _) => Some(DEFAULT_2CH),
        // 3ch
        (3, _) => Some(DEFAULT_3CH),
        // 4ch
        (4, "DV.LRLsRs") => Some(DV_4CH_1),
        (4, "DV.LRCS") => Some(DV_4CH_2),
        (4, "DV.LRCWo") => Some(DV_4CH_3),
        (4, _) => Some(DEFAULT_4CH),
        // 5ch
        (5, "DV.LRLsRsC") => Some(DV_5CH_1),
        (5, _) => Some(DEFAULT_5CH),
        // 6ch
        (6, "DV.LRLsRsCS") => Some(DV_6CH_1),
        (6, "DV.LmixRmixTWoQ1Q2") => Some(DV_6CH_2),
        (6, _) => Some(DEFAULT_6CH),
        // 7ch
        (7, _) => None,
        // 8ch
        (8, "DV.LRCWoLsRsLmixRmix") => Some(DV_8CH_1),
        (8, "DV.LRCWoLs1Rs1Ls2Rs2") => Some(DV_8CH_2),
        (8, "DV.LRCWoLsRsLcRc") => Some(DV_8CH_3),
        (8, _) => None,
        // >8ch
        (9.., _) => None,
        (..=0, _) => unreachable!(),
    }
}

fn positions_are_compatible(
    pos1: &[gst_audio::AudioChannelPosition],
    pos2: &[gst_audio::AudioChannelPosition],
) -> bool {
    if pos1.len() != pos2.len() {
        return false;
    }

    let Ok(mask1) = AudioChannelPosition::positions_to_mask(pos1, false) else {
        return false;
    };

    let Ok(mask2) = AudioChannelPosition::positions_to_mask(pos2, false) else {
        return false;
    };

    mask1 == mask2
}

const CHANNEL_MAPPINGS: &[(&[gst_audio::AudioChannelPosition], &str)] = &[
    // 1ch
    (DEFAULT_1CH, "default"),
    // 2ch
    (DEFAULT_2CH, "default"),
    // 3ch
    (DEFAULT_3CH, "default"),
    // 4ch
    (DV_4CH_1, "DV.LRLsRs"),
    (DV_4CH_2, "DV.LRCS"),
    (DV_4CH_3, "DV.LRCWo"),
    (DEFAULT_4CH, "default"),
    // 5ch
    (DV_5CH_1, "DV.LRLsRsC"),
    (DEFAULT_5CH, "default"),
    // 6ch
    (DV_6CH_1, "DV.LRLsRsCS"),
    (DV_6CH_2, "DV.LmixRmixTWoQ1Q2"),
    (DEFAULT_6CH, "default"),
    // 8ch
    (DV_8CH_1, "DV.LRCWoLsRsLmixRmix"),
    (DV_8CH_2, "DV.LRCWoLs1Rs1Ls2Rs2"),
    (DV_8CH_3, "DV.LRCWoLsRsLcRc"),
];

// Returns either one of the "DV.*" names, or "default" or None
pub(crate) fn find_channel_order_from_positions(
    pos: &[gst_audio::AudioChannelPosition],
) -> Option<&'static str> {
    let n_channels = pos.len();

    for (map, name) in CHANNEL_MAPPINGS {
        if map.len() == n_channels && positions_are_compatible(map, pos) {
            return Some(name);
        }
    }

    None
}

#[allow(clippy::manual_range_contains)]
pub(crate) fn reorder_channels<T: Default + Clone + Copy + FromByteSlice>(
    buffer_ref: &mut gst::BufferRef,
    reorder_map: &[usize],
) -> Result<(), gst::FlowError> {
    let mut map = buffer_ref
        .map_writable()
        .map_err(|_| gst::FlowError::Error)?;

    let n_channels = reorder_map.len();

    assert!(n_channels >= 1 && n_channels <= MAX_REORDER_CHANNELS);

    let mut scratch: [T; MAX_REORDER_CHANNELS] = Default::default();
    let in_frame = &mut scratch[0..n_channels];

    let data = map.as_mut_slice_of::<T>().unwrap();
    for out_frame in data.chunks_exact_mut(n_channels) {
        in_frame.copy_from_slice(out_frame);

        // "The reorder_map can be used for reordering by assigning
        //  channel i of the input to channel reorder_map[i] of the output."
        for (i, &out_idx) in reorder_map.iter().enumerate() {
            out_frame[out_idx] = in_frame[i];
        }
    }
    Ok(())
}
