// SPDX-License-Identifier: MPL-2.0

pub mod depay;
mod header;
pub use header::{AuHeader, AuHeaderContext};
mod mode;
pub use mode::ModeConfig;
pub mod pay;

#[cfg(test)]
mod tests;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Mpeg4GenericError {
    #[error("Can't compare AU index 0x8000_0000 to 0")]
    AuIndexComparisonLimit,

    #[error("Can't compare RTP timestamps 0x8000_0000 to 0")]
    RTPTimestampComparisonLimit,
}

/// An Access Unit Index implemented as a comparable new type on a `[std::num::Wrapping]::<u32>`.
define_wrapping_comparable_u32_with_display!(
    AccessUnitIndex,
    Mpeg4GenericError,
    AuIndexComparisonLimit,
);

/// An RTP timestamp implemented as a comparable new type on a `[std::num::Wrapping]::<u32>`.
define_wrapping_comparable_u32_with_display!(
    RtpTimestamp,
    Mpeg4GenericError,
    RTPTimestampComparisonLimit,
);
