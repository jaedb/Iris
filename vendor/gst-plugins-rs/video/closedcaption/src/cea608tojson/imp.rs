// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// TODO:
//
//  * For now the element completely discards data for channel 1,
//    as it has never really been used. Ideally it should handle it,
//    not critical.
//
//  * A few control commands aren't supported, see TODO in
//    decode_control. The only notable command is delete_to_end_of_row,
//    probably hasn't seen wide usage though :)
//
//  * By design the element outputs text only once, leaving one corner case
//    not covered: fill both the display and the non-displayed memory in
//    pop-on mode, then send multiple End Of Caption commands: the expected
//    result is that the text flips back and forth. This is probably never
//    used in practice, and difficult to represent with our output format.
//
//  * The Chunk object could have an "indent" field, that would get translated
//    to tab offsets for small bandwidth savings

use cea608_types::tables::Channel;
use cea608_types::tables::MidRow;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use cea608_types::{Cea608, Cea608State as Cea608StateTracker};

use crate::cea608utils::*;
use crate::ttutils::{Chunk, Line, Lines};

use atomic_refcell::AtomicRefCell;

use std::sync::LazyLock;

use std::collections::BTreeMap;
use std::sync::Mutex;

const DEFAULT_UNBUFFERED: bool = false;

#[derive(Debug)]
struct TimestampedLines {
    lines: Lines,
    pts: Option<gst::ClockTime>,
    duration: Option<gst::ClockTime>,
}

struct Cursor {
    row: u32,
    col: usize,
    style: TextStyle,
    underline: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Char {
        c: char,
        style: TextStyle,
        underline: bool,
    },
    Empty,
}

#[derive(Clone)]
struct Row {
    cells: Vec<Cell>,
    row: u32,
}

impl Row {
    fn new(row: u32) -> Self {
        Self {
            cells: vec![Cell::Empty; 32],
            row,
        }
    }

    fn push(&mut self, cursor: &mut Cursor, c: char) {
        self.cells[cursor.col] = Cell::Char {
            c,
            style: cursor.style,
            underline: cursor.underline,
        };

        // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(v)
        if cursor.col < 31 {
            cursor.col += 1;
        }
    }

    fn push_midrow(&mut self, cursor: &mut Cursor, style: TextStyle, underline: bool) {
        self.cells[cursor.col] = Cell::Empty;
        cursor.style = style;
        cursor.underline = underline;

        // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(v)
        if cursor.col < 31 {
            cursor.col += 1;
        }
    }

    fn pop(&mut self, cursor: &mut Cursor) {
        if cursor.col > 0 {
            self.cells[cursor.col] = Cell::Empty;
            cursor.col -= 1;
        }
    }

    fn is_empty(&self) -> bool {
        self.cells.iter().all(|c| *c == Cell::Empty)
    }
}

impl From<Row> for Line {
    fn from(row: Row) -> Self {
        let mut chunks = vec![];
        let mut current_chunk: Option<Chunk> = None;
        let mut indent = 0;

        let mut trailing = 0;

        for sc in row.cells {
            match sc {
                Cell::Char {
                    c,
                    style,
                    underline,
                } => {
                    if let Some(mut chunk) = current_chunk.take() {
                        current_chunk = {
                            if style != chunk.style || underline != chunk.underline || trailing > 0
                            {
                                let mut text = " ".repeat(trailing);
                                trailing = 0;
                                text.push(c);

                                chunks.push(chunk);
                                Some(Chunk {
                                    style,
                                    underline,
                                    text,
                                })
                            } else {
                                chunk.text.push(c);
                                Some(chunk)
                            }
                        }
                    } else {
                        current_chunk = Some(Chunk {
                            style,
                            underline,
                            text: c.into(),
                        });
                    }
                }
                Cell::Empty => {
                    if current_chunk.is_none() {
                        indent += 1;
                    } else {
                        trailing += 1;
                    }
                }
            }
        }

        if let Some(chunk) = current_chunk.take() {
            chunks.push(chunk);
        }

        Line {
            column: Some(indent),
            row: Some(row.row),
            chunks,
            carriage_return: None,
        }
    }
}

#[derive(Clone)]
struct Settings {
    unbuffered: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unbuffered: DEFAULT_UNBUFFERED,
        }
    }
}

struct State {
    mode: Option<Cea608Mode>,
    rows: BTreeMap<u32, Row>,
    first_pts: Option<gst::ClockTime>,
    current_pts: Option<gst::ClockTime>,
    current_duration: Option<gst::ClockTime>,
    carriage_return: Option<bool>,
    clear: Option<bool>,
    cursor: Cursor,
    pending_lines: Option<TimestampedLines>,
    settings: Settings,
    cea608_state: Cea608StateTracker,
}

impl Default for State {
    fn default() -> Self {
        State {
            mode: None,
            rows: BTreeMap::new(),
            first_pts: gst::ClockTime::NONE,
            current_pts: gst::ClockTime::NONE,
            current_duration: gst::ClockTime::NONE,
            carriage_return: None,
            clear: None,
            cursor: Cursor {
                row: 14,
                col: 0,
                style: TextStyle::White,
                underline: false,
            },
            pending_lines: None,
            settings: Settings::default(),
            cea608_state: Cea608StateTracker::default(),
        }
    }
}

pub struct Cea608ToJson {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,

    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea608tojson",
        gst::DebugColorFlags::empty(),
        Some("CEA-608 to JSON Element"),
    )
});

fn dump(
    imp: &Cea608ToJson,
    cc_data: [u8; 2],
    pts: impl Into<Option<gst::ClockTime>>,
    duration: impl Into<Option<gst::ClockTime>>,
) {
    let pts = pts.into();
    let end = pts.opt_add(duration.into());

    if cc_data != [0x80, 0x80] {
        let Ok(code) = cea608_types::tables::Code::from_data(cc_data) else {
            return;
        };
        gst::debug!(
            CAT,
            imp = imp,
            "{} -> {}: {:?}",
            pts.display(),
            end.display(),
            code
        );
    } else {
        gst::trace!(
            CAT,
            imp = imp,
            "{} -> {}: padding",
            pts.display(),
            end.display()
        );
    }
}

impl State {
    fn update_mode(&mut self, imp: &Cea608ToJson, mode: Cea608Mode) -> Option<TimestampedLines> {
        if mode.is_rollup() && self.mode == Some(Cea608Mode::PopOn) {
            // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(2)(v)
            let _ = self.drain(imp, true);
        }

        let ret = if Some(mode) != self.mode {
            if self.mode == Some(Cea608Mode::PopOn) {
                self.drain_pending(imp)
            } else {
                self.drain(imp, true)
            }
        } else {
            None
        };

        if mode.is_rollup()
            && (self.mode == Some(Cea608Mode::PopOn)
                || self.mode == Some(Cea608Mode::PaintOn)
                || self.mode.is_none())
        {
            // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(ii)
            self.cursor.row = 14;
            self.cursor.col = 0;

            self.rows.insert(self.cursor.row, Row::new(self.cursor.row));
        }

        self.mode = Some(mode);

        ret
    }

    fn drain(&mut self, imp: &Cea608ToJson, flush: bool) -> Option<TimestampedLines> {
        gst::log!(CAT, imp = imp, "Draining");

        let pts = if self.settings.unbuffered {
            self.current_pts
        } else {
            self.first_pts
        };

        let duration = if self.settings.unbuffered {
            self.current_duration
        } else {
            match self.mode {
                Some(Cea608Mode::PopOn) => gst::ClockTime::NONE,
                _ => self
                    .current_pts
                    .opt_add(self.current_duration)
                    .opt_checked_sub(self.first_pts)
                    .ok()
                    .flatten(),
            }
        };

        self.first_pts = gst::ClockTime::NONE;

        let mut lines: Vec<Line> = vec![];

        if flush {
            for (_idx, row) in std::mem::take(&mut self.rows).into_iter() {
                if !row.is_empty() {
                    let mut line: Line = row.into();
                    line.carriage_return = self.carriage_return.take();
                    lines.push(line);
                }
            }

            self.rows.clear();
            self.cea608_state.reset();
        } else {
            for row in self.rows.values() {
                if !row.is_empty() {
                    let mut line: Line = row.clone().into();
                    line.carriage_return = self.carriage_return.take();
                    lines.push(line);
                }
            }
        }

        let clear = self.clear.take();

        if !lines.is_empty() {
            Some(TimestampedLines {
                lines: Lines {
                    lines,
                    mode: self.mode,
                    clear,
                },
                pts,
                duration,
            })
        } else if clear == Some(true) {
            Some(TimestampedLines {
                lines: Lines {
                    lines,
                    mode: self.mode,
                    clear,
                },
                pts: self.current_pts,
                duration: Some(gst::ClockTime::ZERO),
            })
        } else {
            None
        }
    }

    fn drain_pending(&mut self, imp: &Cea608ToJson) -> Option<TimestampedLines> {
        if let Some(mut pending) = self.pending_lines.take() {
            gst::log!(CAT, imp = imp, "Draining pending");
            pending.duration = self
                .current_pts
                .opt_add(self.current_duration)
                .opt_checked_sub(pending.pts)
                .ok()
                .flatten();
            Some(pending)
        } else {
            None
        }
    }

    fn handle_preamble(
        &mut self,
        imp: &Cea608ToJson,
        preamble: cea608_types::tables::PreambleAddressCode,
    ) -> Option<TimestampedLines> {
        gst::log!(CAT, imp = imp, "preamble: {:?}", preamble);

        let drain_roll_up = self.cursor.row != preamble.row() as u32;

        // In unbuffered mode, we output the whole roll-up window
        // and need to move it when the preamble relocates it
        // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(ii)
        if self.settings.unbuffered {
            if let Some(mode) = self.mode {
                if mode.is_rollup() && self.cursor.row != preamble.row() as u32 {
                    let offset = match mode {
                        Cea608Mode::RollUp2 => 1,
                        Cea608Mode::RollUp3 => 2,
                        Cea608Mode::RollUp4 => 3,
                        _ => unreachable!(),
                    };

                    let current_top_row = self.cursor.row.saturating_sub(offset);
                    let new_row_offset = preamble.row() as i32 - self.cursor.row as i32;

                    for row in current_top_row..self.cursor.row {
                        if let Some(mut row) = self.rows.remove(&row) {
                            let new_row = row.row as i32 + new_row_offset;
                            if new_row >= 0 {
                                row.row = new_row as u32;
                                self.rows.insert(row.row, row);
                            }
                        }
                    }
                }
            }
        }

        self.cursor.row = preamble.row() as u32;
        self.cursor.col = preamble.column() as usize;
        self.cursor.underline = preamble.underline();
        self.cursor.style = if preamble.italics() {
            TextStyle::ItalicWhite
        } else {
            match preamble.color() {
                cea608_types::tables::Color::White => TextStyle::White,
                cea608_types::tables::Color::Green => TextStyle::Green,
                cea608_types::tables::Color::Blue => TextStyle::Blue,
                cea608_types::tables::Color::Cyan => TextStyle::Cyan,
                cea608_types::tables::Color::Red => TextStyle::Red,
                cea608_types::tables::Color::Yellow => TextStyle::Yellow,
                cea608_types::tables::Color::Magenta => TextStyle::Magenta,
            }
        };

        if let Some(mode) = self.mode {
            match mode {
                Cea608Mode::RollUp2
                | Cea608Mode::RollUp3
                | Cea608Mode::RollUp4
                | Cea608Mode::PaintOn => {
                    if self.settings.unbuffered {
                        /* We only need to drain when the roll-up window was relocated */
                        let ret = if drain_roll_up {
                            self.drain(imp, true)
                        } else {
                            None
                        };

                        if let std::collections::btree_map::Entry::Vacant(e) =
                            self.rows.entry(self.cursor.row)
                        {
                            e.insert(Row::new(self.cursor.row));
                        }

                        ret
                    // The relocation is potentially destructive, let us drain
                    } else {
                        let ret = self.drain(imp, true);
                        self.rows.insert(self.cursor.row, Row::new(self.cursor.row));

                        ret
                    }
                }
                Cea608Mode::PopOn => {
                    let row = self.cursor.row;
                    self.rows.entry(row).or_insert_with(|| Row::new(row));

                    None
                }
            }
        } else {
            None
        }
    }

    fn handle_text(&mut self, imp: &Cea608ToJson, text: cea608_types::Text) {
        if let Some(row) = self.rows.get_mut(&self.cursor.row) {
            if text.needs_backspace {
                row.pop(&mut self.cursor);
            }

            if (text.char1.is_some() || text.char2.is_some()) && self.first_pts.is_none() {
                if let Some(mode) = self.mode {
                    if mode.is_rollup() || mode == Cea608Mode::PaintOn {
                        self.first_pts = self.current_pts;
                    }
                }
            }

            if let Some(c) = text.char1 {
                row.push(&mut self.cursor, c);
            }

            if let Some(c) = text.char2 {
                row.push(&mut self.cursor, c);
            }
        } else {
            gst::warning!(CAT, imp = imp, "No row to append decoded text to!");
        }
    }

    fn handle_midrowchange(&mut self, midrowchange: MidRow) {
        if let Some(row) = self.rows.get_mut(&self.cursor.row) {
            row.push_midrow(
                &mut self.cursor,
                midrowchange.into(),
                midrowchange.underline(),
            );
        }
    }

    fn handle_cc_data(
        &mut self,
        imp: &Cea608ToJson,
        pts: Option<gst::ClockTime>,
        duration: Option<gst::ClockTime>,
        cc_data: [u8; 2],
    ) -> Option<TimestampedLines> {
        let Ok(Some(cea608)) = self.cea608_state.decode(cc_data) else {
            return None;
        };

        self.current_pts = pts;
        self.current_duration = duration;

        if cea608.channel() != Channel::ONE {
            return None;
        }

        match cea608 {
            Cea608::EraseDisplay(_chan) => {
                return match self.mode {
                    Some(Cea608Mode::PopOn) => {
                        self.clear = Some(true);
                        self.drain_pending(imp)
                    }
                    _ => {
                        let ret = self.drain(imp, true);
                        self.clear = Some(true);
                        ret
                    }
                };
            }
            Cea608::NewMode(_chan, mode) => return self.update_mode(imp, mode.into()),
            Cea608::CarriageReturn(_chan) => {
                gst::log!(CAT, imp = imp, "carriage return");

                if let Some(mode) = self.mode {
                    // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(2)(i) (f)(3)(i)
                    if mode.is_rollup() {
                        let ret = if self.settings.unbuffered {
                            let offset = match mode {
                                Cea608Mode::RollUp2 => 1,
                                Cea608Mode::RollUp3 => 2,
                                Cea608Mode::RollUp4 => 3,
                                _ => unreachable!(),
                            };

                            let top_row = self.cursor.row.saturating_sub(offset);

                            // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(iii)
                            self.rows.remove(&top_row);

                            for row in top_row + 1..self.cursor.row + 1 {
                                if let Some(mut row) = self.rows.remove(&row) {
                                    row.row -= 1;
                                    self.rows.insert(row.row, row);
                                }
                            }

                            self.rows.insert(self.cursor.row, Row::new(self.cursor.row));
                            self.drain(imp, false)
                        } else {
                            let ret = self.drain(imp, true);
                            self.carriage_return = Some(true);
                            ret
                        };

                        return ret;
                    }
                }
            }
            Cea608::Backspace(_chan) => {
                if let Some(row) = self.rows.get_mut(&self.cursor.row) {
                    row.pop(&mut self.cursor);
                }
            }
            Cea608::EraseNonDisplay(_chan) => {
                if self.mode == Some(Cea608Mode::PopOn) {
                    self.rows.clear();
                }
            }
            Cea608::EndOfCaption(_chan) => {
                // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(2)
                self.update_mode(imp, Cea608Mode::PopOn);
                self.first_pts = self.current_pts;
                let ret = if self.settings.unbuffered {
                    self.drain(imp, true)
                } else {
                    let ret = self.drain_pending(imp);
                    self.pending_lines = self.drain(imp, true);
                    ret
                };
                return ret;
            }
            Cea608::TabOffset(_chan, count) => {
                self.cursor.col += count as usize;
                // C.13 Right Margin Limitation
                self.cursor.col = std::cmp::min(self.cursor.col, 31);
            }
            Cea608::Text(text) => {
                if let Some(mode) = self.mode {
                    self.mode?;
                    gst::log!(CAT, imp = imp, "text");
                    self.handle_text(imp, text);

                    if mode.is_rollup() && self.settings.unbuffered {
                        return self.drain(imp, false);
                    }
                }
            }
            // TODO: implement
            Cea608::DeleteToEndOfRow(_chan) => (),
            Cea608::Preamble(_chan, preamble) => return self.handle_preamble(imp, preamble),
            Cea608::MidRowChange(_chan, change) => self.handle_midrowchange(change),
        }
        None
    }
}

impl Cea608ToJson {
    fn output(&self, lines: TimestampedLines) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, imp = self, "outputting: {:?}", lines);

        let json = serde_json::to_string(&lines.lines).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ["Failed to serialize as json {}", err]
            );

            gst::FlowError::Error
        })?;

        let mut buf = gst::Buffer::from_mut_slice(json.into_bytes());
        {
            let buf_mut = buf.get_mut().unwrap();
            buf_mut.set_pts(lines.pts);
            buf_mut.set_duration(lines.duration);
        }

        gst::log!(CAT, imp = self, "Pushing {:?}", buf);

        self.srcpad.push(buf)
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.borrow_mut();

        let pts = buffer.pts();
        if pts.is_none() {
            gst::error!(CAT, obj = pad, "Require timestamped buffers");
            return Err(gst::FlowError::Error);
        }

        let duration = buffer.duration();
        if duration.is_none() {
            gst::error!(CAT, obj = pad, "Require buffers with duration");
            return Err(gst::FlowError::Error);
        }

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        if data.len() < 2 {
            gst::error!(CAT, obj = pad, "Invalid closed caption packet size");

            return Ok(gst::FlowSuccess::Ok);
        }

        let cc_data = [data[0], data[1]];

        dump(self, cc_data, pts, duration);

        if let Some(lines) = state.handle_cc_data(self, pts, duration, cc_data) {
            drop(state);
            self.output(lines)
        } else if state.settings.unbuffered {
            drop(state);
            self.srcpad.push_event(
                gst::event::Gap::builder(pts.unwrap())
                    .duration(duration)
                    .build(),
            );
            Ok(gst::FlowSuccess::Ok)
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(..) => {
                // We send our own caps downstream
                let caps = gst::Caps::builder("application/x-json")
                    .field("format", "cea608")
                    .build();
                self.srcpad.push_event(gst::event::Caps::new(&caps))
            }
            EventView::FlushStop(..) => {
                let mut state = self.state.borrow_mut();
                let old_settings = state.settings.clone();
                *state = State::default();
                state.settings = old_settings;
                drop(state);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Eos(..) => {
                if let Some(lines) = self.state.borrow_mut().drain_pending(self) {
                    let _ = self.output(lines);
                }
                if let Some(lines) = self.state.borrow_mut().drain(self, true) {
                    let _ = self.output(lines);
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cea608ToJson {
    const NAME: &'static str = "GstCea608ToJson";
    type Type = super::Cea608ToJson;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cea608ToJson::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cea608ToJson::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            state: AtomicRefCell::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for Cea608ToJson {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("unbuffered")
                .nick("Unbuffered")
                .blurb(
                    "Whether captions should be output at display time, \
                     instead of waiting to determine durations. Useful with live input",
                )
                .default_value(DEFAULT_UNBUFFERED)
                .mutable_ready()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "unbuffered" => {
                self.settings.lock().unwrap().unbuffered =
                    value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "unbuffered" => {
                let settings = self.settings.lock().unwrap();
                settings.unbuffered.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Cea608ToJson {}

impl ElementImpl for Cea608ToJson {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CEA-608 to TT",
                "Generic",
                "Converts CEA-608 Closed Captions to JSON",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-json").build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.borrow_mut();
                *state = State::default();
                state.settings = self.settings.lock().unwrap().clone();
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let mut state = self.state.borrow_mut();
                *state = State::default();
            }
            _ => (),
        }

        Ok(ret)
    }
}
