// Copyright (C) 2019 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) const CDG_PACKET_SIZE: i32 = 24;

// 75 sectors/sec * 4 packets/sector = 300 packets/sec
pub(crate) const CDG_PACKET_PERIOD: u64 = 300;

pub(crate) const CDG_WIDTH: u32 = 300;
pub(crate) const CDG_HEIGHT: u32 = 216;

pub(crate) const CDG_MASK: u8 = 0x3F;
pub(crate) const CDG_COMMAND: u8 = 0x09;
