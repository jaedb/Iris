//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use bitstream_io::BigEndian;

pub const CLOCK_RATE: u32 = 90000;
pub const ENDIANNESS: BigEndian = BigEndian;

mod aggr_header;
mod error;
mod integers;
mod obu;

pub use aggr_header::*;
pub(crate) use error::*;
pub use integers::*;
pub use obu::*;
