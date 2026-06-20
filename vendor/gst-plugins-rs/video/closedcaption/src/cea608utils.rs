// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::VecDeque;

use cea608_types::{
    tables::{Channel, Color, MidRow, PreambleAddressCode, PreambleType},
    Cea608, Cea608State, Mode,
};
use gst::glib;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use pango::prelude::*;

use crate::ccutils::recalculate_pango_layout;

use gst::prelude::MulDiv;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea608utils",
        gst::DebugColorFlags::empty(),
        Some("CEA 608 utilities"),
    )
});

#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum,
)]
#[repr(u32)]
#[enum_type(name = "GstTtToCea608Mode")]
pub enum Cea608Mode {
    PopOn,
    PaintOn,
    RollUp2,
    RollUp3,
    RollUp4,
}

impl From<cea608_types::Mode> for Cea608Mode {
    fn from(value: cea608_types::Mode) -> Self {
        match value {
            cea608_types::Mode::PopOn => Self::PopOn,
            cea608_types::Mode::PaintOn => Self::PaintOn,
            cea608_types::Mode::RollUp2 => Self::RollUp2,
            cea608_types::Mode::RollUp3 => Self::RollUp3,
            cea608_types::Mode::RollUp4 => Self::RollUp4,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub enum TextStyle {
    #[default]
    White,
    Green,
    Blue,
    Cyan,
    Red,
    Yellow,
    Magenta,
    ItalicWhite,
}

impl TextStyle {
    pub fn is_italics(&self) -> bool {
        *self == TextStyle::ItalicWhite
    }

    pub fn to_cea608_color(self) -> Option<cea608_types::tables::Color> {
        match self {
            Self::White => Some(cea608_types::tables::Color::White),
            Self::Green => Some(cea608_types::tables::Color::Green),
            Self::Blue => Some(cea608_types::tables::Color::Blue),
            Self::Cyan => Some(cea608_types::tables::Color::Cyan),
            Self::Red => Some(cea608_types::tables::Color::Red),
            Self::Yellow => Some(cea608_types::tables::Color::Yellow),
            Self::Magenta => Some(cea608_types::tables::Color::Magenta),
            Self::ItalicWhite => None,
        }
    }
}

impl From<cea608_types::tables::PreambleAddressCode> for TextStyle {
    fn from(value: cea608_types::tables::PreambleAddressCode) -> Self {
        value.color().into()
    }
}

impl From<cea608_types::tables::MidRow> for TextStyle {
    fn from(value: cea608_types::tables::MidRow) -> Self {
        if value.italics() {
            Self::ItalicWhite
        } else {
            // XXX: MidRow may not change color
            value
                .color()
                .unwrap_or(cea608_types::tables::Color::White)
                .into()
        }
    }
}

impl From<cea608_types::tables::Color> for TextStyle {
    fn from(value: cea608_types::tables::Color) -> Self {
        match value {
            cea608_types::tables::Color::White => Self::White,
            cea608_types::tables::Color::Green => Self::Green,
            cea608_types::tables::Color::Blue => Self::Blue,
            cea608_types::tables::Color::Cyan => Self::Cyan,
            cea608_types::tables::Color::Red => Self::Red,
            cea608_types::tables::Color::Yellow => Self::Yellow,
            cea608_types::tables::Color::Magenta => Self::Magenta,
        }
    }
}

impl From<u32> for TextStyle {
    fn from(val: u32) -> Self {
        match val {
            0 => TextStyle::White,
            1 => TextStyle::Green,
            2 => TextStyle::Blue,
            3 => TextStyle::Cyan,
            4 => TextStyle::Red,
            5 => TextStyle::Yellow,
            6 => TextStyle::Magenta,
            7 => TextStyle::ItalicWhite,
            _ => TextStyle::White,
        }
    }
}

const MAX_ROW: usize = 14;
const MAX_COLUMN: usize = 31;

#[derive(Debug)]
pub struct Cea608Frame {
    display_lines: VecDeque<Cea608Line>,
    undisplay_lines: VecDeque<Cea608Line>,
    mode: Option<cea608_types::Mode>,
    selected_channel: Option<cea608_types::tables::Channel>,
    column: usize,
    row: usize,
    base_row: u8,
    preamble: PreambleAddressCode,
}

impl Cea608Frame {
    pub fn new() -> Self {
        Self {
            display_lines: VecDeque::new(),
            undisplay_lines: VecDeque::new(),
            mode: None,
            selected_channel: None,
            column: 0,
            row: MAX_ROW,
            base_row: MAX_ROW as u8,
            preamble: PreambleAddressCode::new(0, false, PreambleType::Indent0),
        }
    }

    fn write_lines(&mut self) -> Option<&mut VecDeque<Cea608Line>> {
        match self.mode {
            None => None,
            Some(Mode::PopOn) => Some(&mut self.undisplay_lines),
            _ => Some(&mut self.display_lines),
        }
    }

    fn line_mut(&mut self, row: usize) -> Option<&mut Cea608Line> {
        self.write_lines()
            .and_then(|lines| lines.iter_mut().find(|line| line.no == row))
    }

    fn ensure_cell(&mut self, row: usize, column: usize) {
        let line = if let Some(line) = self.line_mut(row) {
            line
        } else {
            let Some(lines) = self.write_lines() else {
                return;
            };
            lines.push_back(Cea608Line {
                no: row,
                line: VecDeque::new(),
                initial_preamble: None,
            });
            lines.make_contiguous().sort_by_key(|line| line.no);
            lines.iter_mut().find(|line| line.no == row).unwrap()
        };
        while line.line.len() <= column {
            line.line.push_back(Cea608Cell::Empty);
        }
    }

    fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cea608Cell> {
        self.write_lines().and_then(|lines| {
            lines
                .iter_mut()
                .find(|line| line.no == row)
                .and_then(|line| line.line.get_mut(column))
        })
    }

    fn reset(&mut self) {
        self.display_lines.clear();
        self.undisplay_lines.clear();
        self.mode = None;
        self.column = 0;
        self.selected_channel = None;
    }

    fn row(&self) -> usize {
        self.mode.map_or(self.row, |mode| {
            if mode.is_rollup() {
                self.base_row as usize
            } else {
                self.row
            }
        })
    }

    pub fn set_channel(&mut self, channel: Channel) {
        if Some(channel) != self.selected_channel {
            gst::log!(CAT, "reset (channel change)");
            self.reset();
            self.selected_channel = Some(channel);
        }
    }

    pub fn push_code(&mut self, cea608: Cea608) -> bool {
        if self.selected_channel.is_none() {
            self.selected_channel = Some(cea608.channel());
        }
        if Some(cea608.channel()) != self.selected_channel {
            return false;
        }
        match cea608 {
            Cea608::Text(text) => {
                let mut ret = false;
                if text.needs_backspace {
                    ret |= self.backspace();
                }
                if let Some(c) = text.char1 {
                    ret |= self.push_char(c);
                }
                if let Some(c) = text.char2 {
                    ret |= self.push_char(c);
                }
                ret
            }
            Cea608::NewMode(_chan, new_mode) => self.new_mode(new_mode),
            Cea608::Preamble(_chan, preamble) => self.preamble(preamble),
            Cea608::EraseDisplay(_chan) => {
                self.display_lines.clear();
                true
            }
            Cea608::EraseNonDisplay(_chan) => {
                self.undisplay_lines.clear();
                false
            }
            Cea608::EndOfCaption(_chan) => {
                std::mem::swap(&mut self.display_lines, &mut self.undisplay_lines);
                self.new_mode(cea608_types::Mode::PopOn);
                true
            }
            Cea608::Backspace(_chan) => self.backspace(),
            Cea608::TabOffset(_chan, n_tabs) => {
                self.column = (self.column + n_tabs as usize).min(MAX_COLUMN);
                false
            }
            Cea608::MidRowChange(_chan, midrow) => self.midrow(midrow),
            Cea608::CarriageReturn(_chan) => self.carriage_return(),
            Cea608::DeleteToEndOfRow(_chan) => self.delete_to_end_of_row(),
        }
    }

    fn push_char(&mut self, c: char) -> bool {
        let row = self.row();
        self.ensure_cell(row, self.column);
        if self.column == 0 {
            let preamble = self.preamble;
            let Some(line) = self.line_mut(row) else {
                return false;
            };
            line.initial_preamble = Some(preamble);
        }
        let Some(cell) = self.cell_mut(row, self.column) else {
            return false;
        };
        *cell = Cea608Cell::Char(c);
        self.column = (self.column + 1).min(MAX_COLUMN);
        true
    }

    fn new_mode(&mut self, new_mode: cea608_types::Mode) -> bool {
        if Some(new_mode) == self.mode {
            return false;
        }

        // if we are changing to roll up mode, we need to reset
        if new_mode.is_rollup()
            && !self
                .mode
                .map_or(!new_mode.is_rollup(), |mode| mode.is_rollup())
        {
            self.base_row = MAX_ROW as u8;
            gst::log!(CAT, "reset (mode change)");
            self.reset();
        }

        self.mode = Some(new_mode);
        if new_mode.is_rollup() {
            self.column = 0;
            // XXX: do we need to move any existing captions?
        }
        true
    }

    fn preamble(&mut self, preamble: PreambleAddressCode) -> bool {
        self.preamble = preamble;
        self.column = preamble.column() as usize;
        let Some(mode) = self.mode else {
            self.row = preamble.row() as usize;
            return false;
        };
        match mode {
            Mode::PopOn | Mode::PaintOn => {
                self.row = preamble.row() as usize;
            }
            Mode::RollUp2 | Mode::RollUp3 | Mode::RollUp4 => {
                let base_row = preamble.row().max(mode.rollup_rows().unwrap_or(0) - 1);
                if self.base_row != base_row {
                    gst::debug!(
                        CAT,
                        "roll up base row change from {} to {base_row}",
                        self.base_row
                    );
                    let diff = base_row as i8 - self.base_row as i8;
                    self.display_lines.retain(|line| {
                        (0..=MAX_ROW as isize).contains(&(line.no as isize + diff as isize))
                    });
                    for line in self.display_lines.iter_mut() {
                        line.no = (line.no as isize + diff as isize) as usize;
                    }
                    self.base_row = preamble.row();
                }
            }
        }
        true
    }

    fn midrow(&mut self, midrow: MidRow) -> bool {
        let row = self.row();
        self.ensure_cell(row, self.column);
        let Some(cell) = self.cell_mut(row, self.column) else {
            return false;
        };
        *cell = Cea608Cell::MidRow(midrow);
        self.column = (self.column + 1).min(MAX_COLUMN);
        true
    }

    fn carriage_return(&mut self) -> bool {
        if !matches!(
            self.mode,
            Some(
                cea608_types::Mode::RollUp2
                    | cea608_types::Mode::RollUp3
                    | cea608_types::Mode::RollUp4
            )
        ) {
            // no-op for non roll up modes
            return false;
        }
        let n_rows = self.mode.unwrap().rollup_rows().unwrap();
        self.display_lines
            .retain(|line| line.no > self.base_row as usize + 1 - n_rows as usize);
        for line in self.display_lines.iter_mut() {
            line.no -= 1;
        }
        self.column = 0;
        true
    }

    fn backspace(&mut self) -> bool {
        if self.column == 0 {
            return false;
        }
        let row = self.row();
        self.ensure_cell(row, self.column - 1);
        let Some(cell) = self.cell_mut(row, self.column - 1) else {
            return false;
        };
        *cell = Cea608Cell::Empty;
        self.column -= 1;
        true
    }

    fn delete_to_end_of_row(&mut self) -> bool {
        let column = self.column;
        let row = self.row();
        let Some(line) = self.line_mut(row) else {
            return false;
        };
        while line.line.len() > column {
            line.line.pop_back();
        }
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &Cea608Line> {
        self.display_lines.iter()
    }

    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for (i, line) in self.iter().enumerate() {
            let mut seen_non_space = false;
            if i != 0 {
                text.push('\r');
                text.push('\n');
            }
            for (_i, c) in line.display_iter() {
                match c {
                    Cea608Cell::Empty | Cea608Cell::MidRow(_) => {
                        if seen_non_space {
                            text.push(' ')
                        }
                    }
                    Cea608Cell::Char(c) => {
                        if *c != ' ' {
                            seen_non_space = true;
                        }
                        text.push(*c);
                    }
                }
            }
        }
        text
    }

    pub fn mode(&self) -> Option<cea608_types::Mode> {
        self.mode
    }
}

#[derive(Debug)]
pub struct Cea608Line {
    no: usize,
    line: VecDeque<Cea608Cell>,
    initial_preamble: Option<PreambleAddressCode>,
}

impl Cea608Line {
    pub fn initial_preamble(&self) -> PreambleAddressCode {
        self.initial_preamble
            .unwrap_or_else(|| PreambleAddressCode::new(0, false, PreambleType::Indent0))
    }

    pub fn display_iter(&self) -> impl Iterator<Item = (usize, &Cea608Cell)> {
        self.line.iter().enumerate()
    }
}

#[derive(Debug)]
pub enum Cea608Cell {
    Empty,
    Char(char),
    MidRow(cea608_types::tables::MidRow),
}

fn pango_foreground_color_from_608(color: Color) -> pango::AttrColor {
    let (r, g, b) = match color {
        Color::White => (u16::MAX, u16::MAX, u16::MAX),
        Color::Green => (0, u16::MAX, 0),
        Color::Blue => (0, 0, u16::MAX),
        Color::Cyan => (0, u16::MAX, u16::MAX),
        Color::Red => (u16::MAX, 0, 0),
        Color::Yellow => (u16::MAX, u16::MAX, 0),
        Color::Magenta => (u16::MAX, 0, u16::MAX),
    };
    pango::AttrColor::new_foreground(r, g, b)
}

// SAFETY: Required because `pango::Layout` is not `Send` but the whole `Cea608Renderer` needs to be.
// We ensure that no additional references to the layout are ever created, which makes it safe
// to send it to other threads as long as only a single thread uses it concurrently.
unsafe impl Send for Cea608Renderer {}

pub struct Cea608Renderer {
    frame: Cea608Frame,
    state: Cea608State,
    context: pango::Context,
    layout: pango::Layout,
    rectangle: Option<gst_video::VideoOverlayRectangle>,
    video_width: u32,
    video_height: u32,
    left_alignment: i32,
    black_background: bool,

    safe_width: f32,
    safe_height: f32,
    max_layout_width: i32,
    max_layout_height: i32,
}

impl Cea608Renderer {
    pub fn new() -> Self {
        let video_width = 0;
        let video_height = 0;
        let fontmap = pangocairo::FontMap::new();
        let context = fontmap.create_context();
        context.set_language(Some(&pango::Language::from_string("en_US")));
        context.set_base_dir(pango::Direction::Ltr);
        let layout = pango::Layout::new(&context);
        layout.set_alignment(pango::Alignment::Left);
        let (max_layout_width, max_layout_height) =
            recalculate_pango_layout(&layout, video_width, video_height);
        Self {
            frame: Cea608Frame::new(),
            state: Cea608State::default(),
            context,
            layout,
            rectangle: None,
            video_width,
            video_height,
            left_alignment: 0,
            black_background: false,
            safe_width: 0.8,
            safe_height: 0.8,
            max_layout_width,
            max_layout_height,
        }
    }

    pub fn push_pair(&mut self, pair: [u8; 2]) -> Result<bool, cea608_types::ParserError> {
        if let Some(cea608) = self.state.decode(pair)? {
            gst::trace!(CAT, "Decoded {cea608:?}");
            if self.frame.push_code(cea608) {
                self.rectangle.take();
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn set_channel(&mut self, channel: cea608_types::tables::Channel) {
        if self.frame.selected_channel != Some(channel) {
            self.frame.set_channel(channel);
            self.rectangle.take();
        }
    }

    pub fn channel(&self) -> Option<cea608_types::tables::Channel> {
        self.frame.selected_channel
    }

    pub fn set_video_size(&mut self, width: u32, height: u32) {
        if width != self.video_width || height != self.video_height {
            self.video_width = width;
            self.video_height = height;
            self.recalculate_window_position();
        }
    }

    pub fn set_safe_title_area(&mut self, safe_width: f32, safe_height: f32) {
        if safe_width != self.safe_width || safe_height != self.safe_height {
            self.safe_width = safe_width;
            self.safe_height = safe_height;
            self.recalculate_window_position();
        }
    }

    fn recalculate_window_position(&mut self) {
        self.layout = pango::Layout::new(&self.context);
        self.layout.set_alignment(pango::Alignment::Left);
        let width = (self.video_width as f32 * self.safe_width) as u32;
        let height = (self.video_height as f32 * self.safe_height) as u32;
        let (max_layout_width, max_layout_height) =
            recalculate_pango_layout(&self.layout, width, height);
        self.left_alignment = (self.video_width as i32 - max_layout_width) / 2;
        self.max_layout_width = max_layout_width;
        self.max_layout_height = max_layout_height;
        self.rectangle.take();
    }

    pub fn set_black_background(&mut self, bg: bool) {
        self.black_background = bg;
        self.rectangle.take();
    }

    pub fn generate_rectangle(&mut self) -> Option<gst_video::VideoOverlayRectangle> {
        if let Some(rectangle) = self.rectangle.clone() {
            return Some(rectangle);
        }

        // 1. generate pango layout
        let mut text = String::new();
        let attrs = pango::AttrList::new();
        if self.black_background {
            let attr = pango::AttrColor::new_background(0, 0, 0);
            attrs.insert(attr);
        }

        let mut first_row = 0;
        let mut last_row = None;
        for line in self.frame.iter() {
            if let Some(last_row) = last_row {
                for _ in 0..line.no - last_row {
                    text.push('\n');
                }
            } else {
                first_row = line.no;
            }
            last_row = Some(line.no);

            let idx = text.len() as u32;
            let initial = line.initial_preamble();

            let mut foreground_color_attr = pango_foreground_color_from_608(initial.color());
            foreground_color_attr.set_start_index(idx);
            let mut last_color = initial.color();

            let mut underline_attr = pango::AttrInt::new_underline(if initial.underline() {
                pango::Underline::Single
            } else {
                pango::Underline::None
            });
            underline_attr.set_start_index(idx);
            let mut last_underline = initial.underline();

            let mut italic_attr = if initial.italics() {
                let mut attr = pango::AttrInt::new_style(pango::Style::Italic);
                attr.set_start_index(idx);
                Some(attr)
            } else {
                None
            };

            for (_char_no, cell) in line.display_iter() {
                match cell {
                    Cea608Cell::MidRow(midrow) => {
                        text.push(' ');
                        let idx = text.len() as u32;

                        if last_underline != midrow.underline() {
                            underline_attr.set_end_index(idx);
                            attrs.insert(underline_attr);
                            underline_attr = pango::AttrInt::new_underline(if midrow.underline() {
                                pango::Underline::Single
                            } else {
                                pango::Underline::None
                            });
                            underline_attr.set_start_index(idx);
                            last_underline = midrow.underline();
                        }

                        if !midrow.italics() {
                            if let Some(mut italic) = italic_attr.take() {
                                italic.set_end_index(idx);
                                attrs.insert(italic);
                            }
                        } else if midrow.italics() && italic_attr.is_none() {
                            let mut attr = pango::AttrInt::new_style(pango::Style::Italic);
                            attr.set_start_index(idx);
                            italic_attr = Some(attr);
                        }

                        if let Some(color) = midrow.color() {
                            if color != last_color {
                                foreground_color_attr.set_end_index(idx);
                                attrs.insert(foreground_color_attr);
                                foreground_color_attr = pango_foreground_color_from_608(color);
                                foreground_color_attr.set_start_index(idx);
                                last_color = color;
                            }
                        }
                    }
                    Cea608Cell::Empty => text.push(' '),
                    Cea608Cell::Char(c) => text.push(*c),
                }
            }

            let idx = text.len() as u32;

            foreground_color_attr.set_end_index(idx);
            attrs.insert(foreground_color_attr);
            underline_attr.set_end_index(idx);
            attrs.insert(underline_attr);
            if let Some(mut italics) = italic_attr.take() {
                italics.set_end_index(idx);
                attrs.insert(italics);
            }
        }

        // 2. render text
        self.layout.set_text(&text);
        self.layout.set_attributes(Some(&attrs));
        let (_ink_rect, logical_rect) = self.layout.extents();
        let height = logical_rect.height() / pango::SCALE;
        let width = logical_rect.width() / pango::SCALE;

        // No text actually needs rendering
        if width == 0 || height == 0 {
            return None;
        }

        let render_buffer = || -> Option<gst::Buffer> {
            let mut buffer = gst::Buffer::with_size((width * height) as usize * 4).ok()?;

            gst_video::VideoMeta::add(
                buffer.get_mut().unwrap(),
                gst_video::VideoFrameFlags::empty(),
                #[cfg(target_endian = "little")]
                gst_video::VideoFormat::Bgra,
                #[cfg(target_endian = "big")]
                gst_video::VideoFormat::Argb,
                width as u32,
                height as u32,
            )
            .ok()?;
            let buffer = buffer.into_mapped_buffer_writable().unwrap();

            // Pass ownership of the buffer to the cairo surface but keep around
            // a raw pointer so we can later retrieve it again when the surface
            // is done
            let buffer_ptr = buffer.buffer().as_ptr();
            let surface = cairo::ImageSurface::create_for_data(
                buffer,
                cairo::Format::ARgb32,
                width,
                height,
                width * 4,
            )
            .ok()?;

            let cr = cairo::Context::new(&surface).ok()?;

            // Clear background
            cr.set_operator(cairo::Operator::Source);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            cr.paint().ok()?;

            // Render text outline
            cr.save().ok()?;
            cr.set_operator(cairo::Operator::Over);

            cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);

            pangocairo::functions::layout_path(&cr, &self.layout);
            cr.stroke().ok()?;
            cr.restore().ok()?;

            // Render text
            cr.save().ok()?;
            cr.set_source_rgba(255.0, 255.0, 255.0, 1.0);

            pangocairo::functions::show_layout(&cr, &self.layout);

            cr.restore().ok()?;
            drop(cr);

            // Safety: The surface still owns a mutable reference to the buffer but our reference
            // to the surface here is the last one. After dropping the surface the buffer would be
            // freed, so we keep an additional strong reference here before dropping the surface,
            // which is then returned. As such it's guaranteed that nothing is using the buffer
            // anymore mutably.
            unsafe {
                assert_eq!(
                    cairo::ffi::cairo_surface_get_reference_count(surface.to_raw_none()),
                    1
                );
                let buffer = glib::translate::from_glib_none(buffer_ptr);
                drop(surface);
                buffer
            }
        };

        let buffer = match render_buffer() {
            Some(buffer) => buffer,
            None => {
                gst::error!(CAT, "Failed to render buffer");
                return None;
            }
        };

        let safe_height = (self.video_height as f32 * self.safe_height) as i32;
        let vertical_padding =
            (2 * self.video_height as i32 - safe_height - self.max_layout_height) / 2;
        let first_row_position = self
            .max_layout_height
            .mul_div_round(first_row as i32, MAX_ROW as i32 + 1)
            .unwrap();

        gst::debug!(
            CAT,
            "overlay size {width}x{height} at {}x{}, text {text}",
            self.left_alignment,
            first_row_position + vertical_padding
        );
        let rect = gst_video::VideoOverlayRectangle::new_raw(
            &buffer,
            self.left_alignment,
            first_row_position + vertical_padding,
            width as u32,
            height as u32,
            gst_video::VideoOverlayFormatFlags::PREMULTIPLIED_ALPHA,
        );

        self.rectangle = Some(rect.clone());
        Some(rect)
    }

    pub fn clear(&mut self) {
        gst::log!(CAT, "clearing");
        self.state.reset();
        let channel = self.channel();
        self.frame.reset();
        if let Some(channel) = channel {
            self.set_channel(channel);
        }
    }
}
