// SPDX-License-Identifier: MPL-2.0

const DEFAULT_CLOCK_RATE: u32 = 90000;
const ENCODING_NAME: &str = "MP4A-LATM";

pub mod depay;
pub mod parsers;
pub mod pay;

#[cfg(test)]
mod tests;
