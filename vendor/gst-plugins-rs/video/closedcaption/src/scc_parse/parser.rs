// Copyright (C) 2019,2020 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019 Jordan Petridis <jordan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::parser_utils::{end_of_line, timecode, TimeCode};
use winnow::{error::StrContext, ModalResult, Parser};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SccLine {
    Header,
    Empty,
    Caption(TimeCode, Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Header,
    Empty,
    CaptionOrEmpty,
}

#[derive(Debug)]
pub struct SccParser {
    state: State,
}

/// Parser for the SCC header
fn header(s: &mut &[u8]) -> ModalResult<SccLine> {
    use winnow::combinator::opt;
    use winnow::token::literal;

    (
        opt(literal(&[0xEFu8, 0xBBu8, 0xBFu8][..])),
        literal("Scenarist_SCC V1.0"),
        end_of_line,
    )
        .map(|_| SccLine::Header)
        .context(StrContext::Label("invalid header"))
        .parse_next(s)
}

/// Parser that accepts only an empty line
fn empty_line(s: &mut &[u8]) -> ModalResult<SccLine> {
    end_of_line
        .map(|_| SccLine::Empty)
        .context(StrContext::Label("invalid empty line"))
        .parse_next(s)
}

/// A single SCC payload item. This is ASCII hex encoded bytes.
/// It returns an tuple of `(u8, u8)` of the hex encoded bytes.
fn scc_payload_item(s: &mut &[u8]) -> ModalResult<(u8, u8)> {
    use winnow::stream::AsChar;
    use winnow::token::take_while;

    take_while(4..=4, AsChar::is_hex_digit)
        .map(|s: &[u8]| {
            let hex_to_u8 = |v: u8| match v {
                v if v.is_ascii_digit() => v - b'0',
                v if (b'A'..=b'F').contains(&v) => 10 + v - b'A',
                v if (b'a'..=b'f').contains(&v) => 10 + v - b'a',
                _ => unreachable!(),
            };

            let val1 = (hex_to_u8(s[0]) << 4) | hex_to_u8(s[1]);
            let val2 = (hex_to_u8(s[2]) << 4) | hex_to_u8(s[3]);

            (val1, val2)
        })
        .context(StrContext::Label("invalid SCC payload item"))
        .parse_next(s)
}

/// Parser for the whole SCC payload with conversion to the underlying byte values.
fn scc_payload(s: &mut &[u8]) -> ModalResult<Vec<u8>> {
    use winnow::combinator::{alt, repeat};
    use winnow::token::literal;

    let parse_item = (
        scc_payload_item,
        alt((literal(" ").map(|_| ()), end_of_line)),
    )
        .map(|(item, _)| item);

    repeat(1.., parse_item)
        .fold(Vec::new, |mut acc: Vec<_>, item| {
            acc.push(item.0);
            acc.push(item.1);
            acc
        })
        .context(StrContext::Label("invalid SCC payload"))
        .parse_next(s)
}

/// Parser for a SCC caption line in the form `timecode\tpayload`.
fn caption(s: &mut &[u8]) -> ModalResult<SccLine> {
    use winnow::ascii::multispace0;
    use winnow::token::literal;

    (
        timecode,
        literal("\t"),
        multispace0,
        scc_payload,
        end_of_line,
    )
        .map(|(tc, _, _, value, _)| SccLine::Caption(tc, value))
        .context(StrContext::Label("invalid SCC caption line"))
        .parse_next(s)
}

/// SCC parser the parses line-by-line and keeps track of the current state in the file.
impl SccParser {
    pub fn new() -> Self {
        Self {
            state: State::Header,
        }
    }

    pub fn new_scan_captions() -> Self {
        Self {
            state: State::CaptionOrEmpty,
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Header;
    }

    pub fn parse_line(&mut self, mut line: &[u8]) -> Result<SccLine, winnow::error::ContextError> {
        use winnow::combinator::alt;

        match self.state {
            State::Header => header(&mut line)
                .inspect(|_v| {
                    self.state = State::Empty;
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::Empty => empty_line(&mut line)
                .inspect(|_v| {
                    self.state = State::CaptionOrEmpty;
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::CaptionOrEmpty => {
                alt((caption, empty_line))
                    .parse_next(&mut line)
                    .map_err(|err| match err {
                        winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                        winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        let mut input = b"Scenarist_SCC V1.0".as_slice();
        assert_eq!(header(&mut input), Ok(SccLine::Header));
        assert!(input.is_empty());

        let mut input = b"Scenarist_SCC V1.0\n".as_slice();
        assert_eq!(header(&mut input), Ok(SccLine::Header));
        assert!(input.is_empty());

        let mut input = b"Scenarist_SCC V1.0\r\n".as_slice();
        assert_eq!(header(&mut input), Ok(SccLine::Header));
        assert!(input.is_empty());

        let mut input = b"Scenarist_SCC V1.1".as_slice();
        assert!(header(&mut input).is_err());
    }

    #[test]
    fn test_empty_line() {
        let mut input = b"".as_slice();
        assert_eq!(empty_line(&mut input), Ok(SccLine::Empty));
        assert!(input.is_empty());

        let mut input = b"\n".as_slice();
        assert_eq!(empty_line(&mut input), Ok(SccLine::Empty));
        assert!(input.is_empty());

        let mut input = b"\r\n".as_slice();
        assert_eq!(empty_line(&mut input), Ok(SccLine::Empty));
        assert!(input.is_empty());

        let mut input = b" \r\n".as_slice();
        assert!(empty_line(&mut input).is_err());
    }

    #[test]
    fn test_caption() {
        let mut input = b"01:02:53:14\t94ae 94ae 9420 9420 947a 947a 97a2 97a2 a820 68ef f26e 2068 ef6e 6be9 6e67 2029 942c 942c 8080 8080 942f 942f".as_slice();
        assert_eq!(
            caption(&mut input),
            Ok(SccLine::Caption(
                TimeCode {
                    hours: 1,
                    minutes: 2,
                    seconds: 53,
                    frames: 14,
                    drop_frame: false
                },
                vec![
                    0x94, 0xae, 0x94, 0xae, 0x94, 0x20, 0x94, 0x20, 0x94, 0x7a, 0x94, 0x7a, 0x97,
                    0xa2, 0x97, 0xa2, 0xa8, 0x20, 0x68, 0xef, 0xf2, 0x6e, 0x20, 0x68, 0xef, 0x6e,
                    0x6b, 0xe9, 0x6e, 0x67, 0x20, 0x29, 0x94, 0x2c, 0x94, 0x2c, 0x80, 0x80, 0x80,
                    0x80, 0x94, 0x2f, 0x94, 0x2f,
                ]
            ))
        );
        assert!(input.is_empty());

        let mut input = b"01:02:55;14	942c 942c".as_slice();
        assert_eq!(
            caption(&mut input),
            Ok(SccLine::Caption(
                TimeCode {
                    hours: 1,
                    minutes: 2,
                    seconds: 55,
                    frames: 14,
                    drop_frame: true
                },
                vec![0x94, 0x2c, 0x94, 0x2c]
            ))
        );
        assert!(input.is_empty());

        let mut input = b"01:03:27:29	94ae 94ae 9420 9420 94f2 94f2 c845 d92c 2054 c845 5245 ae80 942c 942c 8080 8080 942f 942f".as_slice();
        assert_eq!(
            caption(&mut input),
            Ok(SccLine::Caption(
                TimeCode {
                    hours: 1,
                    minutes: 3,
                    seconds: 27,
                    frames: 29,
                    drop_frame: false
                },
                vec![
                    0x94, 0xae, 0x94, 0xae, 0x94, 0x20, 0x94, 0x20, 0x94, 0xf2, 0x94, 0xf2, 0xc8,
                    0x45, 0xd9, 0x2c, 0x20, 0x54, 0xc8, 0x45, 0x52, 0x45, 0xae, 0x80, 0x94, 0x2c,
                    0x94, 0x2c, 0x80, 0x80, 0x80, 0x80, 0x94, 0x2f, 0x94, 0x2f,
                ]
            ))
        );
        assert!(input.is_empty());

        let mut input = b"00:00:00;00\t942c 942c".as_slice();
        assert_eq!(
            caption(&mut input),
            Ok(SccLine::Caption(
                TimeCode {
                    hours: 0,
                    minutes: 0,
                    seconds: 00,
                    frames: 00,
                    drop_frame: true,
                },
                vec![0x94, 0x2c, 0x94, 0x2c],
            ))
        );
        assert!(input.is_empty());

        let mut input = b"00:00:00;00\t942c 942c\r\n".as_slice();
        assert_eq!(
            caption(&mut input),
            Ok(SccLine::Caption(
                TimeCode {
                    hours: 0,
                    minutes: 0,
                    seconds: 00,
                    frames: 00,
                    drop_frame: true,
                },
                vec![0x94, 0x2c, 0x94, 0x2c],
            ))
        );
        assert!(input.is_empty());
    }

    #[test]
    fn test_parser() {
        let scc_file = include_bytes!("../../tests/dn2018-1217.scc");
        let mut reader = crate::line_reader::LineReader::new();
        let mut parser = SccParser::new();
        let mut line_cnt = 0;

        reader.push(Vec::from(scc_file.as_ref()));

        while let Some(line) = reader.line() {
            let res = match parser.parse_line(line) {
                Ok(res) => res,
                Err(err) => panic!("Couldn't parse line {line_cnt}: {err:?}"),
            };

            match line_cnt {
                0 => assert_eq!(res, SccLine::Header),
                x if x % 2 != 0 => assert_eq!(res, SccLine::Empty),
                _ => match res {
                    SccLine::Caption(_, _) => (),
                    res => panic!("Expected caption at line {line_cnt}, got {res:?}"),
                },
            }

            line_cnt += 1;
        }
    }
}
