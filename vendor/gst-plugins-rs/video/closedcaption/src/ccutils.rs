// Copyright (C) 2020 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use byteorder::{BigEndian, ByteOrder};
use std::fmt;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy)]
pub enum ParseErrorCode {
    WrongLength,
    WrongMagicSequence,
    WrongLayout,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub code: ParseErrorCode,
    pub byte: usize,
    pub msg: String,
}

pub fn extract_cdp(mut data: &[u8]) -> Result<&[u8], ParseError> {
    /* logic from ccconverter */
    let data_len = data.len();

    if data.len() < 11 {
        return Err(ParseError {
            code: ParseErrorCode::WrongLength,
            byte: data_len - data.len(),
            msg: format!(
                "cdp packet too short {}. expected at least {}",
                data.len(),
                11
            ),
        });
    }

    if 0x9669 != BigEndian::read_u16(&data[..2]) {
        return Err(ParseError {
            code: ParseErrorCode::WrongMagicSequence,
            byte: data_len - data.len(),
            msg: String::from("cdp packet does not have initial magic bytes of 0x9669"),
        });
    }
    data = &data[2..];

    if (data[0] as usize) != data_len {
        return Err(ParseError {
            code: ParseErrorCode::WrongLength,
            byte: data_len - data.len(),
            msg: format!(
                "advertised cdp packet length {} does not match length of data {}",
                data[0], data_len
            ),
        });
    }
    data = &data[1..];

    /* skip framerate value */
    data = &data[1..];

    let flags = data[0];
    data = &data[1..];

    if flags & 0x40 == 0 {
        /* no cc_data */
        return Ok(&[]);
    }

    /* skip sequence counter */
    data = &data[2..];

    /* timecode present? */
    if flags & 0x80 == 0x80 {
        if data.len() < 5 {
            return Err(ParseError {
                code: ParseErrorCode::WrongLength,
                byte: data_len - data.len(),
                msg: String::from(
                    "cdp packet signals a timecode but is not large enough to contain a timecode",
                ),
            });
        }
        data = &data[5..];
    }

    /* cc_data */
    if data.len() < 2 {
        return Err(ParseError {
            code: ParseErrorCode::WrongLength,
            byte: data_len - data.len(),
            msg: String::from(
                "cdp packet signals cc_data but is not large enough to contain cc_data",
            ),
        });
    }

    if data[0] != 0x72 {
        return Err(ParseError {
            code: ParseErrorCode::WrongMagicSequence,
            byte: data_len - data.len(),
            msg: String::from("ccp is missing start code 0x72"),
        });
    }
    data = &data[1..];

    let cc_count = data[0];
    data = &data[1..];
    if cc_count & 0xe0 != 0xe0 {
        return Err(ParseError {
            code: ParseErrorCode::WrongMagicSequence,
            byte: data_len - data.len(),
            msg: format!("reserved bits are not 0xe0, found {:02x}", cc_count & 0xe0),
        });
    }
    let cc_count = cc_count & 0x1f;
    let len = 3 * cc_count as usize;

    if len > data.len() {
        return Err(ParseError {
            code: ParseErrorCode::WrongLength,
            byte: data_len - data.len(),
            msg: String::from("cc_data length extends past the end of the cdp packet"),
        });
    }

    /* TODO: validate checksum */

    Ok(&data[..len])
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} at byte {}: {}", self.code, self.byte, self.msg)
    }
}

impl std::error::Error for ParseError {}

// FIXME: we want to render the text in the largest 32 x 15 characters
// that will fit the viewport. This is a truly terrible way to determine
// the appropriate font size, but we only need to run that on resolution
// changes, and the API that would allow us to precisely control the
// line height has not yet been exposed by the bindings:
//
// https://blogs.gnome.org/mclasen/2019/07/27/more-text-rendering-updates/
//
// TODO: switch to the API presented in this post once it's been exposed
pub(crate) fn recalculate_pango_layout(
    layout: &pango::Layout,
    video_width: u32,
    video_height: u32,
) -> (i32, i32) {
    let mut font_desc = pango::FontDescription::from_string("monospace");

    let mut font_size = 1;
    loop {
        font_desc.set_size(font_size * pango::SCALE);
        layout.set_font_description(Some(&font_desc));
        layout
            .set_text("12345678901234567890123456789012\n2\n3\n4\n5\n6\n7\n8\n9\n0\n1\n2\n3\n4\n5");
        let (_ink_rect, logical_rect) = layout.extents();
        if logical_rect.width() > video_width as i32 * pango::SCALE
            || logical_rect.height() > video_height as i32 * pango::SCALE
        {
            font_desc.set_size((font_size - 1) * pango::SCALE);
            layout.set_font_description(Some(&font_desc));
            break;
        }
        font_size += 1;
    }

    let (_ink_rect, logical_rect) = layout.extents();
    (
        logical_rect.width() / pango::SCALE,
        logical_rect.height() / pango::SCALE,
    )
}
