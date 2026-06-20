// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::Error;
use gst::glib;
use gst::prelude::*;

use super::PrecisionClockTimeUncertaintyNanosecondsTag;
use super::PrecisionClockTypeTag;
use super::TaiClockInfo;
use crate::isobmff::{
    boxes::{write_full_box, FULL_BOX_FLAGS_NONE, FULL_BOX_VERSION_0},
    TaicClockType, TAIC_CLOCK_DRIFT_RATE_UNKNOWN, TAIC_TIME_UNCERTAINTY_UNKNOWN,
};

impl<'a> Tag<'a> for PrecisionClockTypeTag {
    type TagType = &'a str;
    const TAG_NAME: &'static glib::GStr = glib::gstr!("precision-clock-type");
}

impl CustomTag<'_> for PrecisionClockTypeTag {
    const FLAG: gst::TagFlag = gst::TagFlag::Meta;
    const NICK: &'static glib::GStr = glib::gstr!("precision-clock-type");
    const DESCRIPTION: &'static glib::GStr =
        glib::gstr!("ISO/IEC 23001-17 TAI Clock type information");
}

impl Tag<'_> for PrecisionClockTimeUncertaintyNanosecondsTag {
    type TagType = i32;
    const TAG_NAME: &'static glib::GStr =
        glib::gstr!("precision-clock-time-uncertainty-nanoseconds");
}

impl CustomTag<'_> for PrecisionClockTimeUncertaintyNanosecondsTag {
    const FLAG: gst::TagFlag = gst::TagFlag::Meta;
    const NICK: &'static glib::GStr = glib::gstr!("precision-clock-time-uncertainty-nanoseconds");
    const DESCRIPTION: &'static glib::GStr =
        glib::gstr!("ISO/IEC 23001-17 TAI Clock time uncertainty (in nanoseconds) information");
}

const TAIC_CLOCK_RESOLUTION_MICROSECONDS: u32 = 1000;

impl Default for TaiClockInfo {
    fn default() -> Self {
        TaiClockInfo {
            time_uncertainty: TAIC_TIME_UNCERTAINTY_UNKNOWN,
            clock_resolution: TAIC_CLOCK_RESOLUTION_MICROSECONDS,
            clock_drift_rate: TAIC_CLOCK_DRIFT_RATE_UNKNOWN,
            clock_type: super::TaicClockType::Unknown,
        }
    }
}

impl TaiClockInfo {
    pub(crate) fn new(clock_type: TaicClockType, time_uncertainty: u64) -> TaiClockInfo {
        TaiClockInfo {
            time_uncertainty,
            clock_type,
            ..Default::default()
        }
    }
    pub(crate) fn write_taic_box(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        write_full_box(v, b"taic", FULL_BOX_VERSION_0, FULL_BOX_FLAGS_NONE, |v| {
            v.extend(self.time_uncertainty.to_be_bytes());
            v.extend(self.clock_resolution.to_be_bytes());
            v.extend(self.clock_drift_rate.to_be_bytes());
            v.extend(((self.clock_type as u8) << 6).to_be_bytes());
            Ok(())
        })
    }
}
