// GStreamer RTP MPEG-4 generic elementary streams Depayloader
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

pub mod imp;
pub(crate) mod parsers;

mod deint_buf;
pub(crate) use deint_buf::DeinterleaveAuBuffer;

use gst::glib;
use gst::prelude::*;
use smallvec::SmallVec;

use std::fmt;

use crate::mp4g::{AccessUnitIndex, Mpeg4GenericError};

glib::wrapper! {
    pub struct RtpMpeg4GenericDepay(ObjectSubclass<imp::RtpMpeg4GenericDepay>)
        @extends crate::basedepay::RtpBaseDepay2, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtpmp4gdepay2",
        gst::Rank::MARGINAL,
        RtpMpeg4GenericDepay::static_type(),
    )
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Mpeg4GenericDepayError {
    #[error("{}", .0)]
    Mpeg4Generic(#[from] Mpeg4GenericError),

    #[error("AU header section too large: expected end {expected_end} / {total}")]
    AuHeaderSectionTooLarge { expected_end: usize, total: usize },

    #[error("AU auxiliary section too large: expected end {expected_end} / {total}")]
    AuAuxiliarySectionTooLarge { expected_end: usize, total: usize },

    #[error("Empty AU Data section")]
    EmptyAuData,

    #[error("Unknown AU size, but multiple AUs in the packet")]
    MultipleAusUnknownSize,

    #[error("Multiple AUs in packet but the AU size {au_size} is > AU data size {au_data_size}")]
    MultipleAusGreaterSizeThanAuData { au_size: usize, au_data_size: usize },

    #[error("No more AU data left for AU with index {index}")]
    NoMoreAuDataLeft { index: AccessUnitIndex },

    #[error("Got AU with index {index} which is earlier than the expected index {expected_index}")]
    TooEarlyAU {
        index: AccessUnitIndex,
        expected_index: AccessUnitIndex,
    },

    #[error("Unexpected non-zero first AU index {index} in packet {ext_seqnum} due to configured constant duration")]
    ConstantDurationAuNonZeroIndex {
        index: AccessUnitIndex,
        ext_seqnum: u64,
    },

    #[error("Constant duration not configured and no headers in packet {ext_seqnum}")]
    NonConstantDurationNoAuHeaders { ext_seqnum: u64 },

    #[error("Constant duration not configured and no CTS delta for AU index {index} in packet {ext_seqnum}")]
    NonConstantDurationAuNoCtsDelta {
        index: AccessUnitIndex,
        ext_seqnum: u64,
    },

    #[error(
        "Fragmented AU size mismatch: expected {expected}, found {found}. Packet {ext_seqnum}"
    )]
    FragmentedAuSizeMismatch {
        expected: u32,
        found: usize,
        ext_seqnum: u64,
    },

    #[error("Fragmented AU CTS mismatch: expected {expected}, found {found}. Packet {ext_seqnum}")]
    FragmentedAuRtpTsMismatch {
        expected: i32,
        found: i32,
        ext_seqnum: u64,
    },

    #[error("Fragmented AU DTS mismatch: expected {expected}, found {found}. Packet {ext_seqnum}")]
    FragmentedAuDtsMismatch {
        expected: i32,
        found: i32,
        ext_seqnum: u64,
    },
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SingleAuOrList {
    Single(AccessUnit),
    List(SmallVec<[AccessUnit; 5]>),
}

impl SingleAuOrList {
    pub fn new_list(capacity: usize) -> Self {
        SingleAuOrList::List(SmallVec::with_capacity(capacity))
    }

    #[inline]
    pub fn push(&mut self, au: AccessUnit) {
        use SingleAuOrList::*;
        match self {
            Single(_) => {
                let list = List(SmallVec::new());
                let prev = std::mem::replace(self, list);
                let Single(prev) = prev else { unreachable!() };
                let List(list) = self else { unreachable!() };
                list.push(prev);
                list.push(au);
            }
            List(list) => list.push(au),
        }
    }
}

#[derive(Debug, Default)]
pub struct MaybeSingleAuOrList(Option<SingleAuOrList>);

impl MaybeSingleAuOrList {
    pub fn new_list(capacity: usize) -> Self {
        MaybeSingleAuOrList(Some(SingleAuOrList::new_list(capacity)))
    }

    pub fn push(&mut self, au: AccessUnit) {
        match &mut self.0 {
            Some(inner) => inner.push(au),
            None => self.0 = Some(SingleAuOrList::Single(au)),
        }
    }

    #[inline]
    pub fn take(&mut self) -> Option<SingleAuOrList> {
        self.0.take()
    }
}

impl From<AccessUnit> for MaybeSingleAuOrList {
    fn from(au: AccessUnit) -> Self {
        MaybeSingleAuOrList(Some(SingleAuOrList::Single(au)))
    }
}

/// A parsed Access Unit.
///
/// All timestamps and duration in clock rate ticks.
/// All timestamps are based on the RTP timestamp of the packet.
#[derive(Debug, Default)]
pub struct AccessUnit {
    pub(crate) ext_seqnum: u64,
    pub(crate) is_fragment: bool,
    pub(crate) size: Option<u32>,
    pub(crate) index: AccessUnitIndex,
    pub(crate) cts_delta: Option<i32>,
    pub(crate) dts_delta: Option<i32>,
    pub(crate) duration: Option<u32>,
    pub(crate) maybe_random_access: Option<bool>,
    pub(crate) is_interleaved: bool,
    pub(crate) data: Vec<u8>,
}

impl fmt::Display for AccessUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "index {} from Packet {}", self.index, self.ext_seqnum)
    }
}
