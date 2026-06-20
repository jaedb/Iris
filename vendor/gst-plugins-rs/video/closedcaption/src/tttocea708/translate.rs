// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::VecDeque;

use cea708_types::tables::*;
use cea708_types::*;

use gst::prelude::*;
use std::sync::LazyLock;

use crate::cea608utils::{Cea608Mode, TextStyle};
use crate::cea708utils::{
    textstyle_foreground_color, textstyle_to_pen_color, Cea708Mode, Cea708ServiceWriter,
};
use crate::tttocea608::translate::{TextToCea608, TimedCea608};
use crate::ttutils::{Chunk, Lines};

pub const DEFAULT_FPS_N: i32 = 30;
pub const DEFAULT_FPS_D: i32 = 1;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "tttocea708translator",
        gst::DebugColorFlags::empty(),
        Some("TT CEA 608 translator"),
    )
});

fn fraction_to_framerate(fraction: gst::Fraction) -> Framerate {
    Framerate::new(fraction.numer() as u32, fraction.denom() as u32)
}

fn is_punctuation(word: &str) -> bool {
    word == "." || word == "," || word == "?" || word == "!" || word == ";" || word == ":"
}

fn peek_word_length(chars: std::iter::Peekable<std::str::Chars>) -> u32 {
    chars.take_while(|c| !c.is_ascii_whitespace()).count() as u32
}

#[derive(Debug)]
pub struct TextToCea708 {
    cea608: TextToCea608,

    // settings
    mode: Cea708Mode,
    roll_up_count: u8,
    cea608_channel: Option<cea608_types::Id>,
    origin_column: u32,
    roll_up_timeout: Option<gst::ClockTime>,
    framerate: gst::Fraction,

    // state
    service_writer: Cea708ServiceWriter,
    cc_data_writer: CCDataWriter,
    output_packets: VecDeque<TimedCea708>,
    sequence_no: u8,
    pen_location: SetPenLocationArgs,
    pen_color: SetPenColorArgs,
    pen_attributes: SetPenAttributesArgs,
    send_roll_up_preamble: bool,
    erase_display_frame_no: Option<u64>,
    last_frame_no: u64,
    base_row: u8,
}

impl Default for TextToCea708 {
    fn default() -> Self {
        let mut cc_data_writer = CCDataWriter::default();
        cc_data_writer.set_output_padding(true);
        cc_data_writer.set_output_cea608_padding(true);
        Self {
            cea608: TextToCea608::default(),
            mode: Cea708Mode::RollUp,
            roll_up_count: 2,
            cea608_channel: Some(cea608_types::Id::CC1),
            origin_column: 0,
            framerate: gst::Fraction::new(DEFAULT_FPS_N, DEFAULT_FPS_D),
            roll_up_timeout: None,
            output_packets: VecDeque::new(),
            sequence_no: 0,
            service_writer: Cea708ServiceWriter::new(1),
            cc_data_writer,
            pen_location: SetPenLocationArgs::new(0, 0),
            pen_color: textstyle_to_pen_color(TextStyle::White),
            pen_attributes: SetPenAttributesArgs::new(
                PenSize::Standard,
                FontStyle::Default,
                TextTag::Dialog,
                TextOffset::Normal,
                false,
                false,
                EdgeType::None,
            ),
            send_roll_up_preamble: false,
            erase_display_frame_no: None,
            last_frame_no: 0,
            base_row: 14,
        }
    }
}

#[derive(Debug)]
pub struct TimedCea708 {
    pub packet: Vec<u8>,
    pub frame_no: u64,
}

impl TextToCea708 {
    pub fn pop_output(&mut self) -> Option<TimedCea708> {
        self.output_packets.pop_front()
    }

    pub fn set_origin_column(&mut self, origin_column: u32) {
        self.origin_column = origin_column;
        self.cea608.set_origin_column(origin_column as u8);
    }

    pub fn set_column(&mut self, column: u8) {
        self.pen_location.column = column;
        self.cea608.set_column(column);
    }

    pub fn set_roll_up_timeout(&mut self, timeout: Option<gst::ClockTime>) {
        self.roll_up_timeout = timeout;
        self.cea608.set_roll_up_timeout(timeout);
    }

    pub fn set_roll_up_count(&mut self, roll_up_count: u8) {
        self.roll_up_count = roll_up_count;
        if self.mode == Cea708Mode::RollUp {
            let cea608_mode = match self.roll_up_count {
                0..=2 => Cea608Mode::RollUp2,
                3 => Cea608Mode::RollUp3,
                _ => Cea608Mode::RollUp4,
            };
            self.cea608.set_mode(cea608_mode);
        }
    }

    pub fn set_framerate(&mut self, framerate: gst::Fraction) {
        self.framerate = framerate;
        self.cea608.set_framerate(framerate);
    }

    pub fn framerate(&self) -> gst::Fraction {
        self.framerate
    }

    pub fn set_cea608_channel(&mut self, channel: Option<cea608_types::Id>) {
        if self.cea608_channel != channel {
            self.cea608.flush();
            if let Some(id) = channel {
                self.cea608.set_caption_id(id);
            }
        }
        self.cea608_channel = channel;
    }

    pub fn set_mode(&mut self, mode: Cea708Mode) {
        self.mode = mode;
        if self.mode != Cea708Mode::PopOn {
            self.send_roll_up_preamble = true;
        }
        let cea608_mode = match mode {
            Cea708Mode::PopOn => Cea608Mode::PopOn,
            Cea708Mode::PaintOn => Cea608Mode::PaintOn,
            Cea708Mode::RollUp => match self.roll_up_count {
                0..=2 => Cea608Mode::RollUp2,
                3 => Cea608Mode::RollUp3,
                _ => Cea608Mode::RollUp4,
            },
        };
        self.cea608.set_mode(cea608_mode);
    }

    pub fn set_service_no(&mut self, service_no: u8) {
        if service_no != self.service_writer.service_no() {
            self.service_writer = Cea708ServiceWriter::new(service_no);
        }
    }

    pub fn last_frame_no(&self) -> u64 {
        self.last_frame_no
    }

    pub fn erase_display_frame_no(&self) -> Option<u64> {
        self.erase_display_frame_no
    }

    pub fn flush(&mut self) {
        self.erase_display_frame_no = None;
        self.output_packets.clear();
        self.send_roll_up_preamble = true;
        self.cea608.flush();
    }

    fn open_line(&mut self, chunk: &Chunk, carriage_return: Option<bool>) {
        let do_preamble = match self.mode {
            Cea708Mode::PopOn | Cea708Mode::PaintOn => true,
            Cea708Mode::RollUp => {
                if let Some(carriage_return) = carriage_return {
                    if carriage_return {
                        self.service_writer.push_codes(&[Code::CR]);
                        self.pen_location.column = self.origin_column as u8;
                        true
                    } else {
                        self.send_roll_up_preamble
                    }
                } else {
                    self.send_roll_up_preamble
                }
            }
        };

        if do_preamble {
            if self.mode == Cea708Mode::RollUp {
                self.service_writer
                    .rollup_preamble(self.roll_up_count, self.base_row);
            }

            self.send_roll_up_preamble = false;
        }

        let mut need_pen_attributes = false;
        if self.pen_attributes.italics != chunk.style.is_italics() {
            need_pen_attributes = true;
            self.pen_attributes.italics = chunk.style.is_italics();
        }

        if self.pen_attributes.underline != (chunk.underline) {
            need_pen_attributes = true;
            self.pen_attributes.underline = chunk.underline;
        }

        if need_pen_attributes {
            self.service_writer.set_pen_attributes(self.pen_attributes);
        }

        if self.pen_color.foreground_color != textstyle_foreground_color(chunk.style) {
            self.pen_color.foreground_color = textstyle_foreground_color(chunk.style);
            self.service_writer.set_pen_color(self.pen_color);
        }
    }

    fn check_erase_display(&mut self) -> bool {
        if let Some(erase_display_frame_no) = self.erase_display_frame_no {
            if self.last_frame_no == erase_display_frame_no - 1 {
                self.erase_display_frame_no = None;
                self.send_roll_up_preamble = true;
                self.service_writer.clear_current_window();
                return true;
            }
        }

        false
    }

    fn cc_data(&mut self) {
        self.check_erase_display();

        let seq_no = self.sequence_no;

        let mut packet = DTVCCPacket::new(seq_no);
        gst::trace!(CAT, "New packet {}", packet.sequence_no());
        while let Some(service) = self.service_writer.take_service(packet.free_space()) {
            gst::trace!(CAT, "adding service {service:?} to packet");
            packet.push_service(service).unwrap();
            if seq_no == self.sequence_no {
                self.sequence_no = (self.sequence_no + 1) & 0x3;
            }
        }
        gst::trace!(CAT, "push packet to writer");
        self.cc_data_writer.push_packet(packet);
        if let Some(cea608_id) = self.cea608_channel {
            let tcea608 = self.cea608.pop_output().unwrap_or(TimedCea608 {
                cea608: [0x80, 0x80],
                frame_no: self.last_frame_no,
            });
            let (byte0, byte1) = (tcea608.cea608[0], tcea608.cea608[1]);
            match cea608_id.field() {
                cea608_types::tables::Field::ONE => self
                    .cc_data_writer
                    .push_cea608(cea708_types::Cea608::Field1(byte0, byte1)),
                cea608_types::tables::Field::TWO => self
                    .cc_data_writer
                    .push_cea608(cea708_types::Cea608::Field2(byte0, byte1)),
            }
        }

        let mut cc_data = vec![];
        gst::trace!(CAT, "write packet to data");
        self.cc_data_writer
            .write(fraction_to_framerate(self.framerate), &mut cc_data)
            .unwrap();

        gst::trace!(CAT, "add data to buffer list");
        self.output_packets.push_back(TimedCea708 {
            packet: cc_data[2..].to_vec(),
            frame_no: self.last_frame_no,
        });

        self.last_frame_no += 1;
    }

    fn pad(&mut self, frame_no: u64) {
        while self.last_frame_no < frame_no {
            if !self.check_erase_display() {
                self.cc_data();
            }
        }
    }

    // XXX: range for frame_no?
    pub fn generate(&mut self, frame_no: u64, end_frame_no: u64, lines: Lines) {
        let origin_column = self.origin_column;
        let mut row = 13;

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

        if self.cea608_channel.is_some() {
            self.cea608.generate(frame_no, end_frame_no, lines.clone());
        }

        if self.mode == Cea708Mode::PopOn || self.mode == Cea708Mode::PaintOn {
            self.pen_location.column = 0;
        };

        let (fps_n, fps_d) = (self.framerate.numer() as u64, self.framerate.denom() as u64);

        self.pad(frame_no);

        let mut cleared = false;
        let mut need_pen_location = false;
        if let Some(mode) = lines.mode {
            if (mode.is_rollup() && self.mode != Cea708Mode::RollUp)
                || (mode == Cea608Mode::PaintOn && self.mode != Cea708Mode::PaintOn)
                || (mode == Cea608Mode::PopOn && self.mode == Cea708Mode::PopOn)
            {
                /* Always erase the display when going to or from pop-on */
                if self.mode == Cea708Mode::PopOn || mode == Cea608Mode::PopOn {
                    self.erase_display_frame_no = None;
                    self.service_writer.clear_current_window();
                    cleared = true;
                }

                self.mode = match mode {
                    Cea608Mode::PopOn => Cea708Mode::PopOn,
                    Cea608Mode::PaintOn => Cea708Mode::PaintOn,
                    Cea608Mode::RollUp2 | Cea608Mode::RollUp3 | Cea608Mode::RollUp4 => {
                        Cea708Mode::RollUp
                    }
                };
                match self.mode {
                    Cea708Mode::RollUp => {
                        self.send_roll_up_preamble = true;
                    }
                    _ => {
                        self.pen_location.column = origin_column as u8;
                        need_pen_location = true;
                    }
                }
            }
        }

        if let Some(clear) = lines.clear {
            if clear && !cleared {
                self.erase_display_frame_no = None;
                self.service_writer.clear_current_window();
                if self.mode != Cea708Mode::PopOn && self.mode != Cea708Mode::PaintOn {
                    self.send_roll_up_preamble = true;
                }
                self.pen_location.column = origin_column as u8;
                need_pen_location = true;
                cleared = true;
            }
        }

        if !lines.lines.is_empty() {
            if self.mode == Cea708Mode::PopOn {
                self.service_writer.popon_preamble();
            } else if self.mode == Cea708Mode::PaintOn {
                self.service_writer.paint_on_preamble();
            }
        }

        for line in &lines.lines {
            gst::log!(CAT, "Processing {:?}", line);

            if let Some(line_row) = line.row {
                row = line_row;
            }

            if row > 14 {
                gst::warning!(CAT, "Dropping line after 15th row: {:?}", line);
                continue;
            }

            if let Some(line_column) = line.column {
                self.pen_location.column = line_column as u8;
                need_pen_location = true;
            } else if self.mode == Cea708Mode::PopOn || self.mode == Cea708Mode::PaintOn {
                self.pen_location.column = origin_column as u8;
                need_pen_location = true;
            }

            if self.pen_location.row != row as u8 {
                need_pen_location = true;
                self.pen_location.row = row as u8;
            }

            if need_pen_location {
                self.service_writer.set_pen_location(self.pen_location);
                if !matches!(self.mode, Cea708Mode::PopOn | Cea708Mode::PaintOn) {
                    self.base_row = self.pen_location.row;
                    self.send_roll_up_preamble = true;
                }
            }

            for (i, chunk) in line.chunks.iter().enumerate() {
                let (cr, mut prepend_space) = if i == 0 {
                    (
                        line.carriage_return,
                        line.carriage_return.map(|cr| !cr).unwrap_or(!cleared),
                    )
                } else {
                    (Some(false), false)
                };
                self.open_line(chunk, cr);

                cleared = false;

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

                    let mut push_code = true;

                    if self.mode == Cea708Mode::RollUp {
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

                        if (next_word_length <= 32 - origin_column
                            && self.pen_location.column as u32 + next_word_length > 31)
                            || self.pen_location.column > 31
                        {
                            self.pen_location.column = self.origin_column as u8;

                            self.open_line(chunk, Some(true));

                            push_code = !c.is_ascii_whitespace();
                        }
                    }

                    if push_code {
                        let code = Code::from_char(c).unwrap_or(Code::Space);
                        self.service_writer.push_codes(&[code]);
                        self.pen_location.column += 1;
                    }

                    if self.mode != Cea708Mode::RollUp && self.pen_location.column > 31 {
                        if chars.peek().is_some() {
                            gst::warning!(CAT, "Dropping characters after 32nd column: {}", c);
                        }
                        break;
                    }
                }
            }

            if self.mode == Cea708Mode::PopOn || self.mode == Cea708Mode::PaintOn {
                row += 1;
            }
            need_pen_location = false;
        }

        if !lines.lines.is_empty() {
            if self.mode == Cea708Mode::PopOn {
                /* No need to erase the display at this point, end_of_caption will be equivalent */
                self.erase_display_frame_no = None;
                self.service_writer.end_of_caption();
            }
            self.service_writer.push_codes(&[Code::ETX]);
        }
        self.cc_data();

        if self.mode == Cea708Mode::PopOn {
            self.erase_display_frame_no =
                Some(self.last_frame_no + end_frame_no.saturating_sub(frame_no));
        } else if let Some(timeout) = self.roll_up_timeout {
            self.erase_display_frame_no =
                Some(self.last_frame_no + timeout.mul_div_round(fps_n, fps_d).unwrap().seconds());
        }
        self.pad(end_frame_no);
    }
}
