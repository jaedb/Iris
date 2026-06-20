// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use winnow::{error::StrContext, ModalParser, ModalResult, Parser};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeCode {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub frames: u32,
    pub drop_frame: bool,
}

/// Parser for parsing a run of ASCII, decimal digits and converting them into a `u32`
pub fn digits(s: &mut &[u8]) -> ModalResult<u32> {
    use winnow::stream::AsChar;
    use winnow::token::take_while;

    take_while(0.., AsChar::is_dec_digit)
        .try_map(std::str::from_utf8)
        .try_map(|s: &str| s.parse::<u32>())
        .parse_next(s)
}

/// Parser for a run of decimal digits, that converts them into a `u32` and checks if the result is
/// in the allowed range.
pub fn digits_range<'a, R: std::ops::RangeBounds<u32>>(
    range: R,
) -> impl ModalParser<&'a [u8], u32, winnow::error::ContextError> {
    move |s: &mut &'a [u8]| {
        digits
            .verify(|v| range.contains(v))
            .context(StrContext::Label("digits out of range"))
            .parse_next(s)
    }
}

/// Parser for a timecode in the form `hh:mm:ss:fs`
pub fn timecode(s: &mut &[u8]) -> ModalResult<TimeCode> {
    use winnow::token::one_of;

    (
        digits,
        one_of(':'),
        digits_range(0..60),
        one_of(':'),
        digits_range(0..60),
        one_of(|c| b":.;,".contains(&c)),
        digits,
    )
        .map(|(hours, _, minutes, _, seconds, sep, frames)| TimeCode {
            hours,
            minutes,
            seconds,
            frames,
            drop_frame: sep == b';' || sep == b',',
        })
        .context(StrContext::Label("invalid timecode"))
        .parse_next(s)
}

/// Parser that checks for EOF and optionally `\n` or `\r\n` before EOF
pub fn end_of_line(s: &mut &[u8]) -> ModalResult<()> {
    use winnow::combinator::alt;
    use winnow::combinator::{eof, opt};
    use winnow::token::literal;

    (opt(alt((literal("\r\n"), literal("\n")))), eof)
        .map(|_| ())
        .parse_next(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timecode() {
        let mut input = b"11:12:13;14".as_slice();
        assert_eq!(
            timecode(&mut input),
            Ok(TimeCode {
                hours: 11,
                minutes: 12,
                seconds: 13,
                frames: 14,
                drop_frame: true
            })
        );
        assert!(input.is_empty());

        let mut input = b"11:12:13:14".as_slice();
        assert_eq!(
            timecode(&mut input),
            Ok(TimeCode {
                hours: 11,
                minutes: 12,
                seconds: 13,
                frames: 14,
                drop_frame: false
            })
        );
        assert!(input.is_empty());

        let mut input = b"11:12:13:14abcd".as_slice();
        assert_eq!(
            timecode(&mut input),
            Ok(TimeCode {
                hours: 11,
                minutes: 12,
                seconds: 13,
                frames: 14,
                drop_frame: false
            })
        );
        assert_eq!(input, b"abcd");

        let mut input = b"abcd11:12:13:14".as_slice();
        assert!(timecode(&mut input).is_err());
    }
}
