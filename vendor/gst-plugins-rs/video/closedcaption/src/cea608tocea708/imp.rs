// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use cea608_types::tables::{Channel, PreambleAddressCode};
use cea708_types::{tables::*, DTVCCPacket};
use cea708_types::{CCDataWriter, Framerate};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use atomic_refcell::AtomicRefCell;

use crate::cea608utils::*;
use crate::cea708utils::{
    cea608_color_to_foreground_color, textstyle_to_pen_color, Cea708ServiceWriter,
};

use cea608_types::{Cea608, Cea608State as Cea608StateTracker};

use std::sync::LazyLock;

#[derive(Debug, Copy, Clone)]
enum Cea608Format {
    S334_1A,
    RawField0,
    RawField1,
}

struct Cea608State {
    tracker: [Cea608StateTracker; 2],
    format: Cea608Format,
    service: [Cea608ServiceState; 4],
}

impl Default for Cea608State {
    fn default() -> Self {
        Self {
            tracker: [Cea608StateTracker::default(), Cea608StateTracker::default()],
            format: Cea608Format::RawField0,
            service: [
                Cea608ServiceState::default(),
                Cea608ServiceState::default(),
                Cea608ServiceState::default(),
                Cea608ServiceState::default(),
            ],
        }
    }
}

struct Cea608ServiceState {
    mode: Option<Cea608Mode>,
    base_row: u8,
}

impl Default for Cea608ServiceState {
    fn default() -> Self {
        Self {
            mode: None,
            base_row: 14,
        }
    }
}

struct Cea708ServiceState {
    writer: Cea708ServiceWriter,
    pen_location: SetPenLocationArgs,
    pen_color: SetPenColorArgs,
    pen_attributes: SetPenAttributesArgs,
}

fn cea608_mode_visible_rows(mode: Cea608Mode) -> u8 {
    match mode {
        Cea608Mode::RollUp2 => 2,
        Cea608Mode::RollUp3 => 3,
        Cea608Mode::RollUp4 => 4,
        _ => unreachable!(),
    }
}

impl Cea708ServiceState {
    fn new(service_no: u8) -> Self {
        Self {
            writer: Cea708ServiceWriter::new(service_no),
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
        }
    }

    fn new_mode(&mut self, cea608_mode: Cea608Mode, base_row: u8) {
        let new_row = if cea608_mode.is_rollup() {
            cea608_mode_visible_rows(cea608_mode) - 1
        } else {
            0
        };
        match cea608_mode {
            Cea608Mode::PopOn => self.writer.popon_preamble(),
            Cea608Mode::PaintOn => self.writer.paint_on_preamble(),
            Cea608Mode::RollUp2 => self.writer.rollup_preamble(2, base_row),
            Cea608Mode::RollUp3 => self.writer.rollup_preamble(3, base_row),
            Cea608Mode::RollUp4 => self.writer.rollup_preamble(4, base_row),
        };
        // we have redefined then window so all the attributes have been reset
        self.pen_location.row = new_row;
        self.pen_location.column = 0;
        self.pen_color = textstyle_to_pen_color(TextStyle::White);
        self.pen_attributes.underline = false;
        self.pen_attributes.italics = false;
    }

    fn handle_text(&mut self, text: cea608_types::Text) {
        if text.needs_backspace {
            self.writer.push_codes(&[Code::BS]);
        }
        if let Some(c) = text.char1 {
            if self.pen_location.column > 31 {
                self.writer.push_codes(&[Code::BS]);
            }
            self.writer.write_char(c);
            self.pen_location.column = std::cmp::min(self.pen_location.column + 1, 32);
        }
        if let Some(c) = text.char2 {
            if self.pen_location.column > 31 {
                self.writer.push_codes(&[Code::BS]);
            }
            self.writer.write_char(c);
            self.pen_location.column = std::cmp::min(self.pen_location.column + 1, 32);
        }
    }

    fn handle_preamble(&mut self, preamble: PreambleAddressCode) {
        let mut need_pen_location = false;
        // TODO: may need a better algorithm than compressing the top four rows
        let new_row = std::cmp::max(0, preamble.row());
        if self.pen_location.row != new_row {
            need_pen_location = true;
            self.pen_location.row = new_row;
        }

        if self.pen_location.column != preamble.column() {
            need_pen_location = true;
            self.pen_location.column = preamble.column();
        }

        if need_pen_location {
            self.writer.set_pen_location(self.pen_location);
        }

        let mut need_pen_attributes = false;
        if self.pen_attributes.italics != preamble.italics() {
            need_pen_attributes = true;
            self.pen_attributes.italics = preamble.italics();
        }

        if self.pen_attributes.underline != preamble.underline() {
            need_pen_attributes = true;
            self.pen_attributes.underline = preamble.underline();
        }

        if need_pen_attributes {
            self.writer.set_pen_attributes(self.pen_attributes);
        }

        if self.pen_color.foreground_color != cea608_color_to_foreground_color(preamble.color()) {
            self.pen_color.foreground_color = cea608_color_to_foreground_color(preamble.color());
            self.writer.set_pen_color(self.pen_color);
        }
    }

    fn handle_midrowchange(&mut self, midrowchange: cea608_types::tables::MidRow) {
        self.writer.write_char(' ');
        if let Some(color) = midrowchange.color() {
            if self.pen_color.foreground_color != cea608_color_to_foreground_color(color) {
                self.pen_color.foreground_color = cea608_color_to_foreground_color(color);
                self.writer.set_pen_color(self.pen_color);
            }
        }

        let mut need_pen_attributes = false;
        if self.pen_attributes.italics != midrowchange.italics() {
            need_pen_attributes = true;
            self.pen_attributes.italics = midrowchange.italics();
        }

        if self.pen_attributes.underline != midrowchange.underline() {
            need_pen_attributes = true;
            self.pen_attributes.underline = midrowchange.underline();
        }

        if need_pen_attributes {
            self.writer.set_pen_attributes(self.pen_attributes);
        }
    }

    fn carriage_return(&mut self) {
        self.writer.carriage_return();
    }
}

struct Cea708State {
    packet_counter: u8,
    writer: CCDataWriter,
    framerate: Framerate,
    service_state: [Cea708ServiceState; 4],
}

impl Cea708State {
    fn take_buffer(&mut self, s334_1a_data: &[u8]) -> Option<gst::Buffer> {
        let mut packet = DTVCCPacket::new(self.packet_counter);
        self.packet_counter += 1;
        self.packet_counter &= 0x3;

        for state in self.service_state.iter_mut() {
            while let Some(service) = state.writer.take_service(packet.free_space()) {
                if let Err(e) = packet.push_service(service.clone()) {
                    gst::warning!(
                        CAT,
                        "failed to add service:{} to outgoing packet: {:?}",
                        service.number(),
                        e
                    );
                }
            }
        }
        self.writer.push_packet(packet);

        assert!(s334_1a_data.len() % 3 == 0);
        for triple in s334_1a_data.chunks(3) {
            if (triple[0] & 0x80) > 0 {
                self.writer
                    .push_cea608(cea708_types::Cea608::Field1(triple[1], triple[2]))
            } else {
                self.writer
                    .push_cea608(cea708_types::Cea608::Field2(triple[1], triple[2]))
            }
        }
        let mut cc_data = Vec::with_capacity(128);
        self.writer.write(self.framerate, &mut cc_data).unwrap();
        gst::trace!(
            CAT,
            "take_buffer produced cc_data_len:{} cc_data:{cc_data:?}",
            cc_data.len()
        );
        if cc_data.len() > 2 {
            // ignore the 2 byte cc_data header that is unused in GStreamer
            let cc_data = cc_data.split_off(2);
            Some(gst::Buffer::from_slice(cc_data))
        } else {
            None
        }
    }
}

struct State {
    cea608: Cea608State,
    cea708: Cea708State,
}

impl Default for State {
    fn default() -> Self {
        State {
            cea608: Cea608State::default(),
            cea708: Cea708State {
                packet_counter: 0,
                writer: CCDataWriter::default(),
                framerate: Framerate::new(30, 1),
                service_state: [
                    Cea708ServiceState::new(1),
                    Cea708ServiceState::new(2),
                    Cea708ServiceState::new(3),
                    Cea708ServiceState::new(4),
                ],
            },
        }
    }
}

enum BufferOrEvent {
    Buffer(gst::Buffer),
    Event(gst::Event),
}

impl State {
    fn field_channel_to_index(&self, field: u8, channel: Channel) -> usize {
        match (field, channel) {
            (0, Channel::ONE) => 0,
            (0, Channel::TWO) => 2,
            (1, Channel::ONE) => 1,
            (1, Channel::TWO) => 3,
            _ => unreachable!(),
        }
    }

    fn service_state_from_608_field_channel(
        &mut self,
        field: u8,
        channel: Channel,
    ) -> &mut Cea708ServiceState {
        &mut self.cea708.service_state[self.field_channel_to_index(field, channel)]
    }

    fn new_mode(&mut self, field: u8, channel: Channel, cea608_mode: Cea608Mode) {
        let idx = self.field_channel_to_index(field, channel);
        if let Some(old_mode) = self.cea608.service[idx].mode {
            if cea608_mode.is_rollup()
                && matches!(old_mode, Cea608Mode::PopOn | Cea608Mode::PaintOn)
            {
                // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(x)
                gst::trace!(CAT, "change to rollup from pop/paint-on");
                self.cea708.service_state[idx].writer.clear_hidden_window();
                self.cea708.service_state[idx].writer.clear_current_window();
                self.cea608.service[idx].base_row = 15;
            }
            if old_mode.is_rollup() && cea608_mode.is_rollup() {
                // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(x)
                let old_count = cea608_mode_visible_rows(old_mode);
                let new_count = cea608_mode_visible_rows(cea608_mode);
                gst::trace!(
                    CAT,
                    "change of rollup row count from {old_count} to {new_count}",
                );
                if old_count > new_count {
                    // push the captions on the top of the window before we shrink the size of the
                    // window
                    for _ in new_count..old_count {
                        self.cea708.service_state[idx]
                            .writer
                            .push_codes(&[Code::CR]);
                    }
                }
            }
        }
        self.cea608.service[idx].mode = Some(cea608_mode);
        let base_row = if cea608_mode.is_rollup() {
            self.cea608.service[idx].base_row
        } else {
            0
        };
        self.cea708.service_state[idx].new_mode(cea608_mode, base_row);
    }

    fn handle_cc_data(&mut self, imp: &Cea608ToCea708, field: u8, cc_data: [u8; 2]) {
        if let Ok(Some(cea608)) = self.cea608.tracker[field as usize].decode(cc_data) {
            gst::trace!(
                CAT,
                imp = imp,
                "have field:{field} channel:{:?} {cea608:?}",
                cea608.channel()
            );
            match cea608 {
                Cea608::NewMode(chan, new_mode) => {
                    self.new_mode(field, chan, new_mode.into());
                }
                Cea608::Text(text) => {
                    let state = self.service_state_from_608_field_channel(field, text.channel);
                    state.handle_text(text);
                }
                Cea608::EndOfCaption(chan) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.writer.end_of_caption();
                    state.writer.etx();
                }
                Cea608::Preamble(chan, mut preamble) => {
                    let idx = self.field_channel_to_index(field, chan);
                    let rollup_count = self.cea608.service[idx]
                        .mode
                        .map(|mode| {
                            if mode.is_rollup() {
                                cea608_mode_visible_rows(mode)
                            } else {
                                0
                            }
                        })
                        .unwrap_or(0);
                    if rollup_count > 0 {
                        // https://www.law.cornell.edu/cfr/text/47/79.101 (f)(1)(ii)
                        let old_base_row = self.cea608.service[idx].base_row;
                        self.cea608.service[idx].base_row = preamble.row();
                        let state = self.service_state_from_608_field_channel(field, chan);
                        if old_base_row != preamble.row() {
                            state.writer.rollup_preamble(rollup_count, preamble.row());
                        }
                        state.pen_location.row = rollup_count - 1;
                        preamble = PreambleAddressCode::new(
                            rollup_count - 1,
                            preamble.underline(),
                            preamble.code(),
                        );
                    }
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.handle_preamble(preamble);
                }
                Cea608::MidRowChange(chan, midrowchange) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.handle_midrowchange(midrowchange);
                }
                Cea608::Backspace(chan) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    // TODO: handle removing a midrowchange
                    state.pen_location.column = std::cmp::max(state.pen_location.column - 1, 0);
                    state.writer.push_codes(&[cea708_types::tables::Code::BS]);
                }
                Cea608::CarriageReturn(chan) => {
                    if let Some(mode) =
                        self.cea608.service[self.field_channel_to_index(field, chan)].mode
                    {
                        if mode.is_rollup() {
                            let state = self.service_state_from_608_field_channel(field, chan);
                            state.carriage_return();
                        }
                    }
                }
                Cea608::EraseDisplay(chan) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.writer.clear_current_window();
                }
                Cea608::EraseNonDisplay(chan) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.writer.clear_hidden_window();
                }
                Cea608::TabOffset(chan, count) => {
                    let state = self.service_state_from_608_field_channel(field, chan);
                    state.pen_location.column =
                        std::cmp::min(state.pen_location.column + count, 32);
                }
                Cea608::DeleteToEndOfRow(_chan) => (),
            }
            let idx = self.field_channel_to_index(field, cea608.channel());
            if let Some(
                Cea608Mode::RollUp2
                | Cea608Mode::RollUp3
                | Cea608Mode::RollUp4
                | Cea608Mode::PaintOn,
            ) = self.cea608.service[idx].mode
            {
                // FIXME: actually track state for when things have changed
                // and we need to send ETX
                self.cea708.service_state[idx]
                    .writer
                    .push_codes(&[cea708_types::tables::Code::ETX]);
            }
        }
    }

    fn take_buffer(
        &mut self,
        s334_1a_data: &[u8],
        pts: gst::ClockTime,
        duration: Option<gst::ClockTime>,
    ) -> BufferOrEvent {
        if let Some(mut buffer) = self.cea708.take_buffer(s334_1a_data) {
            {
                let buffer_ref = buffer.get_mut().unwrap();
                buffer_ref.set_pts(Some(pts));
                buffer_ref.set_duration(duration);
            }
            BufferOrEvent::Buffer(buffer)
        } else {
            BufferOrEvent::Event(gst::event::Gap::builder(pts).duration(duration).build())
        }
    }
}

pub struct Cea608ToCea708 {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,

    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea608tocea708",
        gst::DebugColorFlags::empty(),
        Some("CEA-608 to CEA-708 Element"),
    )
});

impl Cea608ToCea708 {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.borrow_mut();

        let buffer_pts = buffer.pts().ok_or_else(|| {
            gst::error!(CAT, obj = pad, "Require timestamped buffers");
            gst::FlowError::Error
        })?;

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;
        let mut data_len = data.len();

        let s334_1a_data = match state.cea608.format {
            Cea608Format::S334_1A => {
                if data_len % 3 != 0 {
                    gst::warning!(
                        CAT,
                        obj = pad,
                        "Invalid closed caption packet size, truncating"
                    );
                    data_len -= data_len % 3;
                }
                if data_len < 3 {
                    gst::warning!(
                        CAT,
                        obj = pad,
                        "Invalid closed caption packet size, dropping"
                    );
                    return Ok(gst::FlowSuccess::Ok);
                }
                for triple in data.chunks_exact(3) {
                    let field = if (triple[0] & 0x80) > 0 { 0 } else { 1 };
                    state.handle_cc_data(self, field, [triple[1], triple[2]]);
                }
                data.to_vec()
            }
            Cea608Format::RawField0 | Cea608Format::RawField1 => {
                if data_len % 2 != 0 {
                    gst::warning!(
                        CAT,
                        obj = pad,
                        "Invalid closed caption packet size, truncating"
                    );
                    data_len -= data_len % 3;
                }
                if data_len < 2 {
                    gst::warning!(
                        CAT,
                        obj = pad,
                        "Invalid closed caption packet size, dropping"
                    );
                    return Ok(gst::FlowSuccess::Ok);
                }
                let field = match state.cea608.format {
                    Cea608Format::RawField0 => 0,
                    Cea608Format::RawField1 => 1,
                    _ => unreachable!(),
                };
                let mut s334_1a_data = Vec::with_capacity(data.len() / 2 * 3);
                for pair in data.chunks_exact(2) {
                    state.handle_cc_data(self, field, [pair[0], pair[1]]);
                    if field == 0 {
                        s334_1a_data.push(0x80);
                    } else {
                        s334_1a_data.push(0x00);
                    }
                    s334_1a_data.push(pair[0]);
                    s334_1a_data.push(pair[1]);
                }
                s334_1a_data
            }
        };

        let buffer_or_event = state.take_buffer(&s334_1a_data, buffer_pts, buffer.duration());
        drop(state);

        match buffer_or_event {
            BufferOrEvent::Buffer(buffer) => self.srcpad.push(buffer),
            BufferOrEvent::Event(event) => {
                self.srcpad.push_event(event);
                Ok(gst::FlowSuccess::Ok)
            }
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(event) => {
                let mut state = self.state.borrow_mut();
                let caps = event.caps();
                let structure = caps.structure(0).expect("Caps has no structure");
                let framerate = structure.get::<gst::Fraction>("framerate").ok();
                state.cea608.format = match structure.get::<&str>("format") {
                    Ok("raw") => {
                        if structure.has_field("field") {
                            match structure.get("field") {
                                Ok(0) => Cea608Format::RawField0,
                                Ok(1) => Cea608Format::RawField1,
                                _ => {
                                    gst::error!(
                                        CAT,
                                        imp = self,
                                        "unknown \'field\' value in caps, {caps:?}"
                                    );
                                    return false;
                                }
                            }
                        } else {
                            Cea608Format::RawField0
                        }
                    }
                    Ok("s334-1a") => Cea608Format::S334_1A,
                    v => {
                        gst::error!(
                            CAT,
                            imp = self,
                            "unknown or missing \'format\' value {v:?} in caps, {caps:?}"
                        );
                        return false;
                    }
                };
                let framerate = framerate.unwrap_or(gst::Fraction::new(30, 1));
                state.cea708.framerate =
                    Framerate::new(framerate.numer() as u32, framerate.denom() as u32);
                drop(state);

                let caps = gst::Caps::builder("closedcaption/x-cea-708")
                    .field("format", "cc_data")
                    .field("framerate", framerate)
                    .build();
                let new_event = gst::event::Caps::new(&caps);

                return self.srcpad.push_event(new_event);
            }
            EventView::FlushStop(..) => {
                let mut state = self.state.borrow_mut();
                let cea608_format = state.cea608.format;
                *state = State::default();
                state.cea608.format = cea608_format;
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cea608ToCea708 {
    const NAME: &'static str = "GstCea608ToCea708";
    type Type = super::Cea608ToCea708;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cea608ToCea708::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cea608ToCea708::catch_panic_pad_function(
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
        }
    }
}

impl ObjectImpl for Cea608ToCea708 {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for Cea608ToCea708 {}

impl ElementImpl for Cea608ToCea708 {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CEA-608 to CEA-708",
                "Converter",
                "Converts CEA-608 Closed Captions to CEA-708 Closed Captions",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("closedcaption/x-cea-708")
                .field("format", "cc_data")
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &[
                    gst::Structure::builder("closedcaption/x-cea-608")
                        .field("format", "s334-1a")
                        .build(),
                    gst::Structure::builder("closedcaption/x-cea-608")
                        .field("format", "raw")
                        .field("field", gst::List::new([0, 1]))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
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
