// Copyright (C) 2021 Jan Schmidt <jan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::cmp::Ordering;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ForceKeyUnitRequest {
    pub running_time: gst::ClockTime,
    pub all_headers: bool,
    pub count: u32,
}

impl Ord for ForceKeyUnitRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.running_time.cmp(&other.running_time)
    }
}

impl PartialOrd for ForceKeyUnitRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ForceKeyUnitRequest {
    pub fn new_from_event(fku: &gst_video::UpstreamForceKeyUnitEvent) -> ForceKeyUnitRequest {
        ForceKeyUnitRequest {
            running_time: fku.running_time.unwrap(),
            all_headers: fku.all_headers,
            count: fku.count,
        }
    }
}
