// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use std::collections::VecDeque;
use std::sync::LazyLock;

use crate::cea608utils::{Cea608Mode, TextStyle};
use crate::ttutils::{Chunk, Lines};

pub const DEFAULT_FPS_N: i32 = 30;
pub const DEFAULT_FPS_D: i32 = 1;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "tttocea608translator",
        gst::DebugColorFlags::empty(),
        Some("TT CEA 608 translator"),
    )
});

fn is_punctuation(word: &str) -> bool {
    word == "." || word == "," || word == "?" || word == "!" || word == ";" || word == ":"
}

fn peek_word_length(chars: std::iter::Peekable<std::str::Chars>) -> u8 {
    chars.take_while(|c| !c.is_ascii_whitespace()).count() as u8
}

#[derive(Debug)]
pub struct TimedCea608 {
    pub cea608: [u8; 2],
    pub frame_no: u64,
}

#[derive(Debug)]
pub struct TextToCea608 {
    // settings
    origin_column: u8,
    framerate: gst::Fraction,
    roll_up_timeout: Option<gst::ClockTime>,
    // state
    output_frames: VecDeque<TimedCea608>,
    erase_display_frame_no: Option<u64>,
    last_frame_no: u64,
    send_roll_up_preamble: bool,
    style: TextStyle,
    underline: bool,
    column: u8,
    mode: Cea608Mode,
    writer: cea608_types::Cea608Writer,
    caption_id: cea608_types::Id,
}

impl Default for TextToCea608 {
    fn default() -> Self {
        Self {
            origin_column: 0,
            framerate: gst::Fraction::new(DEFAULT_FPS_N, DEFAULT_FPS_D),
            roll_up_timeout: None,
            output_frames: VecDeque::new(),
            erase_display_frame_no: None,
            last_frame_no: 0,
            column: 0,
            send_roll_up_preamble: false,
            style: TextStyle::White,
            underline: false,
            mode: Cea608Mode::PopOn,
            writer: cea608_types::Cea608Writer::default(),
            caption_id: cea608_types::Id::CC1,
        }
    }
}

impl TextToCea608 {
    pub fn set_mode(&mut self, mode: Cea608Mode) {
        self.mode = mode;
        if self.mode != Cea608Mode::PopOn {
            self.send_roll_up_preamble = true;
            self.column = self.origin_column;
        }
    }

    pub fn set_caption_id(&mut self, id: cea608_types::Id) {
        self.caption_id = id;
    }

    pub fn set_framerate(&mut self, framerate: gst::Fraction) {
        self.framerate = framerate;
    }

    pub fn framerate(&self) -> gst::Fraction {
        self.framerate
    }

    pub fn set_roll_up_timeout(&mut self, timeout: Option<gst::ClockTime>) {
        self.roll_up_timeout = timeout;
    }

    pub fn set_origin_column(&mut self, origin_column: u8) {
        self.origin_column = origin_column
    }

    pub fn pop_output(&mut self) -> Option<TimedCea608> {
        self.output_frames.pop_front()
    }

    pub fn last_frame_no(&self) -> u64 {
        self.last_frame_no
    }

    pub fn erase_display_frame_no(&self) -> Option<u64> {
        self.erase_display_frame_no
    }

    pub fn set_column(&mut self, column: u8) {
        self.column = column
    }

    pub fn flush(&mut self) {
        self.erase_display_frame_no = None;
        self.output_frames.clear();
        self.send_roll_up_preamble = true;
    }

    fn check_erase_display(&mut self) -> bool {
        if let Some(erase_display_frame_no) = self.erase_display_frame_no {
            if self.last_frame_no == erase_display_frame_no - 1 {
                self.column = 0;
                self.send_roll_up_preamble = true;
                self.erase_display_memory();
                return true;
            }
        }

        false
    }

    fn cc_data(&mut self, cc_data: [u8; 2]) {
        self.check_erase_display();

        self.output_frames.push_back(TimedCea608 {
            cea608: cc_data,
            frame_no: self.last_frame_no,
        });

        self.last_frame_no += 1;
    }

    fn pad(&mut self, frame_no: u64) {
        while self.last_frame_no < frame_no {
            if !self.check_erase_display() {
                self.cc_data([0x80, 0x80]);
            }
        }
    }

    fn control_code(&mut self, control: cea608_types::tables::Control) {
        let code = cea608_types::tables::ControlCode::new(
            self.caption_id.field(),
            self.caption_id.channel(),
            control,
        );
        let mut vec = smallvec::SmallVec::<[u8; 2]>::new();
        let _ = cea608_types::tables::Code::Control(code).write(&mut vec);
        self.cc_data([vec[0], vec[1]]);
    }

    fn erase_non_displayed_memory(&mut self) {
        self.control_code(cea608_types::tables::Control::EraseNonDisplayedMemory);
    }

    fn resume_caption_loading(&mut self) {
        self.control_code(cea608_types::tables::Control::ResumeCaptionLoading);
    }

    fn resume_direct_captioning(&mut self) {
        self.control_code(cea608_types::tables::Control::ResumeDirectionCaptioning);
    }

    fn delete_to_end_of_row(&mut self) {
        self.control_code(cea608_types::tables::Control::DeleteToEndOfRow);
    }

    fn roll_up_2(&mut self) {
        self.control_code(cea608_types::tables::Control::RollUp2);
    }

    fn roll_up_3(&mut self) {
        self.control_code(cea608_types::tables::Control::RollUp3);
    }

    fn roll_up_4(&mut self) {
        self.control_code(cea608_types::tables::Control::RollUp4);
    }

    fn carriage_return(&mut self) {
        self.control_code(cea608_types::tables::Control::CarriageReturn);
    }

    fn end_of_caption(&mut self) {
        self.control_code(cea608_types::tables::Control::EndOfCaption);
    }

    fn tab_offset(&mut self, offset: u8) {
        let Some(offset) = cea608_types::tables::Control::tab_offset(offset) else {
            return;
        };
        self.control_code(offset);
    }

    fn preamble_indent(&mut self, row: u8, col: u8, underline: bool) {
        let Some(preamble) = cea608_types::tables::PreambleType::from_indent(col) else {
            return;
        };
        self.control_code(cea608_types::tables::Control::PreambleAddress(
            cea608_types::tables::PreambleAddressCode::new(row, underline, preamble),
        ));
    }

    fn preamble_style(&mut self, row: u8, style: TextStyle, underline: bool) {
        let preamble = if style.is_italics() {
            cea608_types::tables::PreambleType::WhiteItalics
        } else {
            cea608_types::tables::PreambleType::Color(style.to_cea608_color().unwrap())
        };
        self.control_code(cea608_types::tables::Control::PreambleAddress(
            cea608_types::tables::PreambleAddressCode::new(row, underline, preamble),
        ));
    }

    fn midrow_change(&mut self, style: TextStyle, underline: bool) {
        let midrow = if style.is_italics() {
            cea608_types::tables::MidRow::new_italics(underline)
        } else {
            cea608_types::tables::MidRow::new_color(style.to_cea608_color().unwrap(), underline)
        };
        self.control_code(cea608_types::tables::Control::MidRow(midrow));
    }

    fn erase_display_memory(&mut self) {
        self.erase_display_frame_no = None;
        self.control_code(cea608_types::tables::Control::EraseDisplayedMemory);
    }

    fn open_chunk(&mut self, chunk: &Chunk, col: u8) -> bool {
        if (chunk.style != self.style || chunk.underline != self.underline) && col < 31 {
            self.midrow_change(chunk.style, chunk.underline);
            self.style = chunk.style;
            self.underline = chunk.underline;
            true
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_line(
        &mut self,
        chunk: &Chunk,
        col: &mut u8,
        row: u8,
        carriage_return: Option<bool>,
    ) -> bool {
        let mut ret = true;

        let do_preamble = match self.mode {
            Cea608Mode::PopOn | Cea608Mode::PaintOn => true,
            Cea608Mode::RollUp2 | Cea608Mode::RollUp3 | Cea608Mode::RollUp4 => {
                if let Some(carriage_return) = carriage_return {
                    if carriage_return {
                        *col = self.origin_column;
                        self.carriage_return();
                        true
                    } else {
                        self.send_roll_up_preamble
                    }
                } else {
                    self.send_roll_up_preamble
                }
            }
        };

        let mut indent = *col / 4;
        let mut offset = *col % 4;

        if do_preamble {
            match self.mode {
                Cea608Mode::RollUp2 => self.roll_up_2(),
                Cea608Mode::RollUp3 => self.roll_up_3(),
                Cea608Mode::RollUp4 => self.roll_up_4(),
                _ => (),
            }

            if chunk.style != TextStyle::White && indent == 0 {
                self.preamble_style(row, chunk.style, chunk.underline);
                self.style = chunk.style;
            } else {
                if chunk.style != TextStyle::White {
                    if offset > 0 {
                        offset -= 1;
                    } else {
                        indent -= 1;
                        offset = 3;
                    }
                    *col -= 1;
                }

                self.style = TextStyle::White;
                self.preamble_indent(row, indent * 4, chunk.underline);
            }

            if self.mode == Cea608Mode::PaintOn {
                self.delete_to_end_of_row();
            }

            self.tab_offset(offset);

            self.underline = chunk.underline;
            self.send_roll_up_preamble = false;
            ret = false;
        } else if *col == self.origin_column {
            ret = false;
        }

        if self.open_chunk(chunk, *col) {
            *col += 1;
            ret = false
        }

        ret
    }

    // XXX: use a range for frame_no?
    pub fn generate(&mut self, frame_no: u64, end_frame_no: u64, lines: Lines) {
        let origin_column = self.origin_column;
        let mut row = 13;
        let (fps_n, fps_d) = (self.framerate.numer() as u64, self.framerate.denom() as u64);

        if self.last_frame_no == 0 {
            gst::debug!(CAT, "Initial skip to frame no {}", frame_no);
            self.last_frame_no = frame_no;
        }

        gst::trace!(
            CAT,
            "generate from frame {frame_no} to {end_frame_no}, erase frame no: {:?}",
            self.erase_display_frame_no
        );

        let frame_no = frame_no.max(self.last_frame_no);
        let end_frame_no = end_frame_no.max(frame_no);

        let mut col = if self.mode == Cea608Mode::PopOn || self.mode == Cea608Mode::PaintOn {
            0
        } else {
            self.column
        };

        self.pad(frame_no);

        let mut cleared = false;
        if let Some(mode) = lines.mode {
            if mode != self.mode {
                /* Always erase the display when going to or from pop-on */
                if self.mode == Cea608Mode::PopOn || mode == Cea608Mode::PopOn {
                    self.erase_display_memory();
                    cleared = true;
                }

                self.mode = mode;
                match self.mode {
                    Cea608Mode::RollUp2 | Cea608Mode::RollUp3 | Cea608Mode::RollUp4 => {
                        self.send_roll_up_preamble = true;
                    }
                    _ => col = origin_column,
                }
            }
        }

        if let Some(clear) = lines.clear {
            if clear && !cleared {
                self.erase_display_memory();
                if self.mode != Cea608Mode::PopOn && self.mode != Cea608Mode::PaintOn {
                    self.send_roll_up_preamble = true;
                }
                col = origin_column;
            }
        }

        if !lines.lines.is_empty() {
            if self.mode == Cea608Mode::PopOn {
                self.resume_caption_loading();
                self.erase_non_displayed_memory();
            } else if self.mode == Cea608Mode::PaintOn {
                self.resume_direct_captioning();
            }
        }

        for line in &lines.lines {
            gst::log!(CAT, "Processing {:?}", line);

            if let Some(line_row) = line.row {
                row = line_row as u8;
            }

            if row > 14 {
                gst::warning!(CAT, "Dropping line after 15th row: {:?}", line);
                continue;
            }

            if let Some(line_column) = line.column {
                if self.mode != Cea608Mode::PopOn && self.mode != Cea608Mode::PaintOn {
                    self.send_roll_up_preamble = true;
                }
                col = line_column as u8;
            } else if self.mode == Cea608Mode::PopOn || self.mode == Cea608Mode::PaintOn {
                col = origin_column;
            }

            for (j, chunk) in line.chunks.iter().enumerate() {
                let mut prepend_space = true;
                while self.writer.n_codes() > 0 {
                    let data = self.writer.pop();
                    self.cc_data(data);
                }

                if j == 0 {
                    prepend_space = self.open_line(chunk, &mut col, row, line.carriage_return);
                } else if self.open_chunk(chunk, col) {
                    prepend_space = false;
                    col += 1;
                }

                if is_punctuation(&chunk.text) {
                    prepend_space = false;
                }

                let text = {
                    if prepend_space {
                        let mut text = " ".to_string();
                        text.push_str(&chunk.text);
                        text
                    } else {
                        chunk.text.clone()
                    }
                };

                let mut chars = text.chars().peekable();

                while let Some(c) = chars.next() {
                    if c == '\r' {
                        continue;
                    }

                    let code = cea608_types::tables::Code::from_char(
                        c,
                        cea608_types::tables::Channel::ONE,
                    )
                    .unwrap_or_else(|| {
                        gst::warning!(CAT, "Not translating UTF8: {}", c);
                        cea608_types::tables::Code::Space
                    });

                    self.writer.push(code);

                    if let cea608_types::tables::Code::Control(_) = code {
                        while self.writer.n_codes() > 0 {
                            let data = self.writer.pop();
                            self.cc_data(data);
                        }
                        // adapted from libcaption's generation code:
                        // specialna are treated as control characters. Duplicated control characters are discarded
                        // So we write a resume after a specialna as a noop control command to break repetition detection
                        match self.mode {
                            Cea608Mode::RollUp2 => self.roll_up_2(),
                            Cea608Mode::RollUp3 => self.roll_up_3(),
                            Cea608Mode::RollUp4 => self.roll_up_4(),
                            Cea608Mode::PopOn => self.resume_caption_loading(),
                            Cea608Mode::PaintOn => self.resume_direct_captioning(),
                        }
                    }

                    col += 1;

                    if self.mode.is_rollup() {
                        /* In roll-up mode, we introduce carriage returns automatically.
                         * Instead of always wrapping once the last column is reached, we
                         * want to look ahead and check whether the following word will fit
                         * on the current row. If it won't, we insert a carriage return,
                         * unless it won't fit on a full row either, in which case it will need
                         * to be broken up.
                         */
                        let next_word_length = if c.is_ascii_whitespace() {
                            peek_word_length(chars.clone())
                        } else {
                            0
                        };

                        if (next_word_length <= 32 - origin_column && col + next_word_length > 31)
                            || col > 31
                        {
                            while self.writer.n_codes() > 0 {
                                let data = self.writer.pop();
                                self.cc_data(data);
                            }

                            self.open_line(chunk, &mut col, row, Some(true));
                        }
                    } else if col > 31 {
                        if chars.peek().is_some() {
                            gst::warning!(CAT, "Dropping characters after 32nd column: {}", c);
                        }
                        break;
                    }
                }
            }

            if self.mode == Cea608Mode::PopOn || self.mode == Cea608Mode::PaintOn {
                while self.writer.n_codes() > 0 {
                    let data = self.writer.pop();
                    self.cc_data(data);
                }
                row += 1;
            }
        }

        if !lines.lines.is_empty() {
            while self.writer.n_codes() > 0 {
                let data = self.writer.pop();
                self.cc_data(data);
            }

            if self.mode == Cea608Mode::PopOn {
                /* No need to erase the display at this point, end_of_caption will be equivalent */
                self.erase_display_frame_no = None;
                self.end_of_caption();
            }
        }

        self.column = col;

        if self.mode == Cea608Mode::PopOn {
            self.erase_display_frame_no =
                Some(self.last_frame_no + end_frame_no.saturating_sub(frame_no));
        } else if let Some(timeout) = self.roll_up_timeout {
            self.erase_display_frame_no =
                Some(self.last_frame_no + timeout.mul_div_round(fps_n, fps_d).unwrap().seconds());
        }

        self.pad(end_frame_no);
    }
}
