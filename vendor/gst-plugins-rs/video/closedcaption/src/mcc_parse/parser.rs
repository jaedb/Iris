// Copyright (C) 2018,2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use either::Either;

use crate::parser_utils::{digits, digits_range, end_of_line, timecode, TimeCode};
use winnow::{error::StrContext, ModalParser, ModalResult, Parser};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MccLine<'a> {
    Header,
    Comment,
    Empty,
    Uuid(&'a [u8]),
    Metadata(&'a [u8], &'a [u8]),
    TimeCodeRate(u8, bool),
    Caption(TimeCode, Option<Vec<u8>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Header,
    EmptyAfterHeader,
    Comments,
    Metadata,
    Captions,
}

#[derive(Debug)]
pub struct MccParser {
    state: State,
}

/// Parser for the MCC header
fn header<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    use winnow::combinator::{alt, opt};
    use winnow::token::literal;

    (
        opt(literal(&[0xEFu8, 0xBBu8, 0xBFu8][..])),
        literal("File Format=MacCaption_MCC V"),
        alt((literal("1.0"), literal("2.0"))),
        end_of_line,
    )
        .map(|_| MccLine::Header)
        .context(StrContext::Label("invalid header"))
        .parse_next(s)
}

/// Parser for an MCC comment, i.e. a line starting with `//`. We don't return the actual comment
/// text as it's irrelevant for us.
fn comment<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    use winnow::token::{literal, rest};

    (literal("//"), rest)
        .map(|_| MccLine::Comment)
        .context(StrContext::Label("invalid comment"))
        .parse_next(s)
}

/// Parser for the MCC UUID line.
fn uuid<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    use winnow::token::{literal, take_while};

    (
        literal("UUID="),
        take_while(1.., |b| b != b'\n' && b != b'\r'),
        end_of_line,
    )
        .map(|(_, uuid, _)| MccLine::Uuid(uuid))
        .context(StrContext::Label("invalid uuid"))
        .parse_next(s)
}

/// Parser for the MCC Time Code Rate line.
fn time_code_rate<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    use winnow::combinator::opt;
    use winnow::token::literal;

    (
        literal("Time Code Rate="),
        digits_range(1..256),
        opt(literal("DF")),
        end_of_line,
    )
        .map(|(_, v, df, _)| MccLine::TimeCodeRate(v as u8, df.is_some()))
        .context(StrContext::Label("invalid timecode rate"))
        .parse_next(s)
}

/// Parser for generic MCC metadata lines in the form `key=value`.
fn metadata<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    use winnow::token::{one_of, take_while};

    (
        take_while(1.., |b| b != b'='),
        one_of('='),
        take_while(1.., |b| b != b'\n' && b != b'\r'),
        end_of_line,
    )
        .map(|(name, _, value, _)| MccLine::Metadata(name, value))
        .context(StrContext::Label("invalid metadata"))
        .parse_next(s)
}

/// Parser that accepts only an empty line
fn empty_line<'a>(s: &mut &'a [u8]) -> ModalResult<MccLine<'a>> {
    end_of_line
        .map(|_| MccLine::Empty)
        .context(StrContext::Label("invalid empty line"))
        .parse_next(s)
}

/// A single MCC payload item. This is ASCII hex encoded bytes plus some single-character
/// short-cuts for common byte sequences.
///
/// It returns an `Either` of the single hex encoded byte or the short-cut byte sequence as a
/// static byte slice.
fn mcc_payload_item(s: &mut &[u8]) -> ModalResult<Either<u8, &'static [u8]>> {
    use winnow::combinator::alt;
    use winnow::stream::AsChar;
    use winnow::token::{literal, take_while};

    alt((
        literal("G").map(|_| Either::Right([0xfa, 0x00, 0x00].as_ref())),
        literal("H").map(|_| Either::Right([0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00].as_ref())),
        literal("I").map(|_| {
            Either::Right([0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00].as_ref())
        }),
        literal("J").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                ]
                .as_ref(),
            )
        }),
        literal("K").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00,
                ]
                .as_ref(),
            )
        }),
        literal("L").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00,
                ]
                .as_ref(),
            )
        }),
        literal("M").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                ]
                .as_ref(),
            )
        }),
        literal("N").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                ]
                .as_ref(),
            )
        }),
        literal("O").map(|_| {
            Either::Right(
                [
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                    0x00,
                ]
                .as_ref(),
            )
        }),
        literal("P").map(|_| Either::Right([0xfb, 0x80, 0x80].as_ref())),
        literal("Q").map(|_| Either::Right([0xfc, 0x80, 0x80].as_ref())),
        literal("R").map(|_| Either::Right([0xfd, 0x80, 0x80].as_ref())),
        literal("S").map(|_| Either::Right([0x96, 0x69].as_ref())),
        literal("T").map(|_| Either::Right([0x61, 0x01].as_ref())),
        literal("U").map(|_| Either::Right([0xe1, 0x00, 0x00, 0x00].as_ref())),
        literal("Z").map(|_| Either::Right([0x00].as_ref())),
        take_while(2..=2, AsChar::is_hex_digit).map(|s: &[u8]| {
            let hex_to_u8 = |v: u8| match v {
                v if v.is_ascii_digit() => v - b'0',
                v if (b'A'..=b'F').contains(&v) => 10 + v - b'A',
                v if (b'a'..=b'f').contains(&v) => 10 + v - b'a',
                _ => unreachable!(),
            };
            let val = (hex_to_u8(s[0]) << 4) | hex_to_u8(s[1]);
            Either::Left(val)
        }),
    ))
    .context(StrContext::Label("invalid payload item"))
    .parse_next(s)
}

/// Parser for the whole MCC payload with conversion to the underlying byte values.
fn mcc_payload(s: &mut &[u8]) -> ModalResult<Vec<u8>> {
    use winnow::combinator::repeat;

    repeat(1.., mcc_payload_item)
        .fold(Vec::new, |mut acc: Vec<_>, item| {
            match item {
                Either::Left(val) => acc.push(val),
                Either::Right(vals) => acc.extend_from_slice(vals),
            }
            acc
        })
        .context(StrContext::Label("invalid MCC payload"))
        .parse_next(s)
}

/// Parser for a MCC caption line in the form `timecode\tpayload`.
fn caption<'a>(
    parse_payload: bool,
) -> impl ModalParser<&'a [u8], MccLine<'a>, winnow::error::ContextError> {
    use winnow::combinator::opt;
    use winnow::token::{one_of, take_while};

    fn parse<'a>(
        parse_payload: bool,
    ) -> impl ModalParser<&'a [u8], Option<Vec<u8>>, winnow::error::ContextError> {
        move |s: &mut &'a [u8]| {
            if parse_payload {
                mcc_payload.map(Some).parse_next(s)
            } else {
                take_while(0.., |b| b != b'\n' && b != b'\r')
                    .map(|_| None)
                    .parse_next(s)
            }
        }
    }

    move |s: &mut &'a [u8]| {
        (
            timecode,
            opt((
                one_of('.'),
                one_of(|c| b"01".contains(&c)),
                opt((one_of(','), digits)),
            )),
            one_of('\t'),
            parse(parse_payload),
            end_of_line,
        )
            .map(|(tc, _, _, value, _)| MccLine::Caption(tc, value))
            .context(StrContext::Label("invalid MCC caption"))
            .parse_next(s)
    }
}

/// MCC parser the parses line-by-line and keeps track of the current state in the file.
impl MccParser {
    pub fn new() -> Self {
        Self {
            state: State::Header,
        }
    }

    pub fn new_scan_captions() -> Self {
        Self {
            state: State::Captions,
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Header;
    }

    pub fn parse_line<'a>(
        &mut self,
        mut line: &'a [u8],
        parse_payload: bool,
    ) -> Result<MccLine<'a>, winnow::error::ContextError> {
        use winnow::combinator::alt;

        match self.state {
            State::Header => header(&mut line)
                .inspect(|_v| {
                    self.state = State::EmptyAfterHeader;
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::EmptyAfterHeader => empty_line(&mut line)
                .inspect(|_v| {
                    self.state = State::Comments;
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::Comments => alt((empty_line, comment))
                .parse_next(&mut line)
                .inspect(|v| {
                    if *v == MccLine::Empty {
                        self.state = State::Metadata;
                    }
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::Metadata => alt((empty_line, uuid, time_code_rate, metadata))
                .parse_next(&mut line)
                .inspect(|v| {
                    if *v == MccLine::Empty {
                        self.state = State::Captions;
                    }
                })
                .map_err(|err| match err {
                    winnow::error::ErrMode::Incomplete(_) => unreachable!(),
                    winnow::error::ErrMode::Backtrack(e) | winnow::error::ErrMode::Cut(e) => e,
                }),
            State::Captions => {
                caption(parse_payload)
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
        let mut input = b"File Format=MacCaption_MCC V1.0".as_slice();
        assert_eq!(header(&mut input), Ok(MccLine::Header));
        assert!(input.is_empty());

        let mut input = b"File Format=MacCaption_MCC V1.0\n".as_slice();
        assert_eq!(header(&mut input), Ok(MccLine::Header));
        assert!(input.is_empty());

        let mut input = b"File Format=MacCaption_MCC V1.0\r\n".as_slice();
        assert_eq!(header(&mut input), Ok(MccLine::Header));
        assert!(input.is_empty());

        let mut input = b"File Format=MacCaption_MCC V2.0\r\n".as_slice();
        assert_eq!(header(&mut input), Ok(MccLine::Header));
        assert!(input.is_empty());

        let mut input = b"File Format=MacCaption_MCC V1.1".as_slice();
        assert!(header(&mut input).is_err());
    }

    #[test]
    fn test_empty_line() {
        let mut input = b"".as_slice();
        assert_eq!(empty_line(&mut input), Ok(MccLine::Empty));
        assert!(input.is_empty());

        let mut input = b"\n".as_slice();
        assert_eq!(empty_line(&mut input), Ok(MccLine::Empty));
        assert!(input.is_empty());

        let mut input = b"\r\n".as_slice();
        assert_eq!(empty_line(&mut input), Ok(MccLine::Empty));
        assert!(input.is_empty());

        let mut input = b" \r\n".as_slice();
        assert!(empty_line(&mut input).is_err());
    }

    #[test]
    fn test_comment() {
        let mut input = b"// blabla".as_slice();
        assert_eq!(comment(&mut input), Ok(MccLine::Comment));
        assert!(input.is_empty());

        let mut input = b"//\n".as_slice();
        assert_eq!(comment(&mut input), Ok(MccLine::Comment));
        assert!(input.is_empty());

        let mut input = b"//".as_slice();
        assert_eq!(comment(&mut input), Ok(MccLine::Comment));
        assert!(input.is_empty());

        let mut input = b" //".as_slice();
        assert!(comment(&mut input).is_err());
    }

    #[test]
    fn test_uuid() {
        let mut input = b"UUID=1234".as_slice();
        assert_eq!(uuid(&mut input), Ok(MccLine::Uuid(b"1234".as_ref())));
        assert!(input.is_empty());

        let mut input = b"UUID=1234\n".as_slice();
        assert_eq!(uuid(&mut input), Ok(MccLine::Uuid(b"1234".as_ref())));
        assert!(input.is_empty());

        let mut input = b"UUID=1234\r\n".as_slice();
        assert_eq!(uuid(&mut input), Ok(MccLine::Uuid(b"1234".as_ref())));
        assert!(input.is_empty());

        let mut input = b"UUID=".as_slice();
        assert!(uuid(&mut input).is_err());

        let mut input = b"uUID=1234".as_slice();
        assert!(uuid(&mut input).is_err());
    }

    #[test]
    fn test_time_code_rate() {
        let mut input = b"Time Code Rate=30".as_slice();
        assert_eq!(
            time_code_rate(&mut input),
            Ok(MccLine::TimeCodeRate(30, false))
        );
        assert!(input.is_empty());

        let mut input = b"Time Code Rate=30DF".as_slice();
        assert_eq!(
            time_code_rate(&mut input),
            Ok(MccLine::TimeCodeRate(30, true))
        );
        assert!(input.is_empty());

        let mut input = b"Time Code Rate=60".as_slice();
        assert_eq!(
            time_code_rate(&mut input),
            Ok(MccLine::TimeCodeRate(60, false))
        );
        assert!(input.is_empty());

        let mut input = b"Time Code Rate=17F".as_slice();
        assert!(time_code_rate(&mut input).is_err(),);

        let mut input = b"Time Code Rate=256".as_slice();
        assert!(time_code_rate(&mut input).is_err(),);
    }

    #[test]
    fn test_metadata() {
        let mut input = b"Creation Date=Thursday, June 04, 2015".as_slice();
        assert_eq!(
            metadata(&mut input),
            Ok(MccLine::Metadata(
                b"Creation Date".as_ref(),
                b"Thursday, June 04, 2015".as_ref()
            ))
        );
        assert!(input.is_empty());

        let mut input = b"Creation Date= ".as_slice();
        assert_eq!(
            metadata(&mut input),
            Ok(MccLine::Metadata(b"Creation Date".as_ref(), b" ".as_ref()),)
        );
        assert!(input.is_empty());

        let mut input = b"Creation Date".as_slice();
        assert!(metadata(&mut input).is_err());

        let mut input = b"Creation Date\n".as_slice();
        assert!(metadata(&mut input).is_err());

        let mut input = b"Creation Date=".as_slice();
        assert!(metadata(&mut input).is_err());

        let mut input = b"Creation Date=\n".as_slice();
        assert!(metadata(&mut input).is_err());
    }

    #[test]
    fn test_caption() {
        let mut input = b"00:00:00:00\tT52S524F67ZZ72F4QROO7391UC13FFF74ZZAEB4".as_slice();
        assert_eq!(
            caption(true).parse_next(&mut input),
            Ok(MccLine::Caption(
                TimeCode {
                    hours: 0,
                    minutes: 0,
                    seconds: 0,
                    frames: 0,
                    drop_frame: false
                },
                Some(vec![
                    0x61, 0x01, 0x52, 0x96, 0x69, 0x52, 0x4F, 0x67, 0x00, 0x00, 0x72, 0xF4, 0xFC,
                    0x80, 0x80, 0xFD, 0x80, 0x80, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0x73, 0x91, 0xE1, 0x00, 0x00, 0x00,
                    0xC1, 0x3F, 0xFF, 0x74, 0x00, 0x00, 0xAE, 0xB4
                ])
            ))
        );
        assert!(input.is_empty());

        let mut input = b"00:00:00:00.0\tT52S524F67ZZ72F4QROO7391UC13FFF74ZZAEB4".as_slice();
        assert_eq!(
            caption(true).parse_next(&mut input),
            Ok(MccLine::Caption(
                TimeCode {
                    hours: 0,
                    minutes: 0,
                    seconds: 0,
                    frames: 0,
                    drop_frame: false
                },
                Some(vec![
                    0x61, 0x01, 0x52, 0x96, 0x69, 0x52, 0x4F, 0x67, 0x00, 0x00, 0x72, 0xF4, 0xFC,
                    0x80, 0x80, 0xFD, 0x80, 0x80, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0x73, 0x91, 0xE1, 0x00, 0x00, 0x00,
                    0xC1, 0x3F, 0xFF, 0x74, 0x00, 0x00, 0xAE, 0xB4
                ])
            ))
        );
        assert!(input.is_empty());

        let mut input = b"00:00:00:00.0,9\tT52S524F67ZZ72F4QROO7391UC13FFF74ZZAEB4".as_slice();
        assert_eq!(
            caption(true).parse_next(&mut input),
            Ok(MccLine::Caption(
                TimeCode {
                    hours: 0,
                    minutes: 0,
                    seconds: 0,
                    frames: 0,
                    drop_frame: false
                },
                Some(vec![
                    0x61, 0x01, 0x52, 0x96, 0x69, 0x52, 0x4F, 0x67, 0x00, 0x00, 0x72, 0xF4, 0xFC,
                    0x80, 0x80, 0xFD, 0x80, 0x80, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00,
                    0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA,
                    0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00,
                    0x00, 0xFA, 0x00, 0x00, 0xFA, 0x00, 0x00, 0x73, 0x91, 0xE1, 0x00, 0x00, 0x00,
                    0xC1, 0x3F, 0xFF, 0x74, 0x00, 0x00, 0xAE, 0xB4
                ])
            ))
        );
        assert!(input.is_empty());

        let mut input = b"Creation Date=\n".as_slice();
        assert!(caption(true).parse_next(&mut input).is_err());
    }

    #[test]
    fn test_parser() {
        let mcc_file = include_bytes!("../../tests/captions-test_708.mcc");
        let mut reader = crate::line_reader::LineReader::new();
        let mut parser = MccParser::new();
        let mut line_cnt = 0;

        reader.push(Vec::from(mcc_file.as_ref()));

        while let Some(line) = reader.line() {
            let res = match parser.parse_line(line, true) {
                Ok(res) => res,
                Err(err) => panic!("Couldn't parse line {line_cnt}: {err:?}"),
            };

            match line_cnt {
                0 => assert_eq!(res, MccLine::Header),
                1 | 37 | 43 => assert_eq!(res, MccLine::Empty),
                x if (2..=36).contains(&x) => assert_eq!(res, MccLine::Comment),
                38 => assert_eq!(
                    res,
                    MccLine::Uuid(b"CA8BC94D-9931-4EEE-812F-2D68FA74F287".as_ref())
                ),
                39 => assert_eq!(
                    res,
                    MccLine::Metadata(
                        b"Creation Program".as_ref(),
                        b"Adobe Premiere Pro CC (Windows)".as_ref()
                    )
                ),
                40 => assert_eq!(
                    res,
                    MccLine::Metadata(
                        b"Creation Date".as_ref(),
                        b"Thursday, June 04, 2015".as_ref()
                    )
                ),
                41 => assert_eq!(
                    res,
                    MccLine::Metadata(b"Creation Time".as_ref(), b"13:48:25".as_ref())
                ),
                42 => assert_eq!(res, MccLine::TimeCodeRate(30, true)),
                _ => match res {
                    MccLine::Caption(_, _) => (),
                    res => panic!("Expected caption at line {line_cnt}, got {res:?}"),
                },
            }

            line_cnt += 1;
        }
    }
}
