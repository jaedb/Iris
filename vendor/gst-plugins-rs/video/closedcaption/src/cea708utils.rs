// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use cea708_types::{tables::*, Service};

use std::collections::VecDeque;

use gst::glib;
use gst::prelude::MulDiv;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use pango::prelude::*;

use crate::ccutils::recalculate_pango_layout;
use crate::cea608utils::{Cea608Renderer, TextStyle};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea708utils",
        gst::DebugColorFlags::empty(),
        Some("CEA-708 Utilities"),
    )
});

#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum,
)]
#[repr(u32)]
#[enum_type(name = "GstTtToCea708Mode")]
pub enum Cea708Mode {
    PopOn,
    PaintOn,
    RollUp,
}

pub fn textstyle_foreground_color(style: TextStyle) -> cea708_types::tables::Color {
    let color = match style {
        TextStyle::Red => cea608_types::tables::Color::Red,
        TextStyle::Blue => cea608_types::tables::Color::Blue,
        TextStyle::Cyan => cea608_types::tables::Color::Cyan,
        TextStyle::White => cea608_types::tables::Color::White,
        TextStyle::Green => cea608_types::tables::Color::Green,
        TextStyle::Yellow => cea608_types::tables::Color::Yellow,
        TextStyle::Magenta => cea608_types::tables::Color::Magenta,
        TextStyle::ItalicWhite => cea608_types::tables::Color::White,
    };
    cea608_color_to_foreground_color(color)
}

pub fn cea608_color_to_foreground_color(
    color: cea608_types::tables::Color,
) -> cea708_types::tables::Color {
    match color {
        cea608_types::tables::Color::Red => cea708_types::tables::Color {
            r: ColorValue::Full,
            g: ColorValue::None,
            b: ColorValue::None,
        },
        cea608_types::tables::Color::Green => cea708_types::tables::Color {
            r: ColorValue::None,
            g: ColorValue::Full,
            b: ColorValue::None,
        },
        cea608_types::tables::Color::Blue => cea708_types::tables::Color {
            r: ColorValue::None,
            g: ColorValue::None,
            b: ColorValue::Full,
        },
        cea608_types::tables::Color::Cyan => cea708_types::tables::Color {
            r: ColorValue::None,
            g: ColorValue::Full,
            b: ColorValue::Full,
        },
        cea608_types::tables::Color::Yellow => cea708_types::tables::Color {
            r: ColorValue::Full,
            g: ColorValue::Full,
            b: ColorValue::None,
        },
        cea608_types::tables::Color::Magenta => cea708_types::tables::Color {
            r: ColorValue::Full,
            g: ColorValue::None,
            b: ColorValue::Full,
        },
        cea608_types::tables::Color::White => cea708_types::tables::Color {
            r: ColorValue::Full,
            g: ColorValue::Full,
            b: ColorValue::Full,
        },
    }
}

pub fn textstyle_to_pen_color(style: TextStyle) -> SetPenColorArgs {
    let black = Color {
        r: ColorValue::None,
        g: ColorValue::None,
        b: ColorValue::None,
    };
    SetPenColorArgs {
        foreground_color: textstyle_foreground_color(style),
        foreground_opacity: Opacity::Solid,
        background_color: black,
        background_opacity: Opacity::Solid,
        edge_color: black,
    }
}

#[derive(Debug)]
pub(crate) struct Cea708ServiceWriter {
    codes: Vec<Code>,
    service_no: u8,
    active_window: WindowBits,
    hidden_window: WindowBits,
}

impl Cea708ServiceWriter {
    pub fn new(service_no: u8) -> Self {
        Self {
            codes: vec![],
            service_no,
            active_window: WindowBits::ZERO,
            hidden_window: WindowBits::ONE,
        }
    }

    pub fn service_no(&self) -> u8 {
        self.service_no
    }

    pub fn take_service(&mut self, available_bytes: usize) -> Option<Service> {
        if self.codes.is_empty() {
            return None;
        }

        gst::trace!(CAT, "New service block {}", self.service_no);
        let mut service = Service::new(self.service_no);
        let mut i = 0;
        for code in self.codes.iter() {
            if code.byte_len() > service.free_space() {
                gst::trace!(CAT, "service is full");
                break;
            }
            let service_header_bytes = if service.is_empty() {
                if service.number() >= 7 {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            if service_header_bytes + service.len() + code.byte_len() > available_bytes {
                gst::trace!(CAT, "packet is full");
                break;
            }
            gst::trace!(CAT, "Adding code {code:?} to service");
            match service.push_code(code) {
                Ok(_) => i += 1,
                Err(_) => break,
            }
        }
        if i == 0 {
            return None;
        }
        self.codes = self.codes.split_off(i);
        Some(service)
    }

    pub fn popon_preamble(&mut self) {
        gst::trace!(CAT, "popon_preamble");
        let window = match self.hidden_window {
            // switch up the newly defined window
            WindowBits::ZERO => 0,
            WindowBits::ONE => 1,
            _ => unreachable!(),
        };
        let args = DefineWindowArgs::new(
            window,
            0,
            Anchor::BottomMiddle,
            true,
            100,
            50,
            14,
            31,
            true,
            true,
            false,
            2,
            1,
        );
        gst::trace!(CAT, "active window {:?}", self.active_window);
        let codes = [
            Code::DeleteWindows(!self.active_window),
            Code::DefineWindow(args),
        ];
        self.push_codes(&codes)
    }

    pub fn clear_current_window(&mut self) {
        gst::trace!(CAT, "clear_current_window {:?}", self.active_window);
        self.push_codes(&[Code::ClearWindows(self.active_window)])
    }

    pub fn clear_hidden_window(&mut self) {
        gst::trace!(CAT, "clear_hidden_window");
        self.push_codes(&[Code::ClearWindows(self.hidden_window)])
    }

    pub fn end_of_caption(&mut self) {
        gst::trace!(CAT, "end_of_caption");
        self.push_codes(&[Code::ToggleWindows(self.active_window | self.hidden_window)]);
        std::mem::swap(&mut self.active_window, &mut self.hidden_window);
        gst::trace!(CAT, "active window {:?}", self.active_window);
    }

    pub fn paint_on_preamble(&mut self) {
        gst::trace!(CAT, "paint_on_preamble");
        let window = match self.active_window {
            WindowBits::ZERO => 0,
            WindowBits::ONE => 1,
            _ => unreachable!(),
        };
        self.push_codes(&[
            // FIXME: assumes positioning in a 16:9 ratio
            Code::DefineWindow(DefineWindowArgs::new(
                window,
                0,
                Anchor::BottomMiddle,
                true,
                100,
                50,
                14,
                31,
                true,
                true,
                true,
                2,
                1,
            )),
        ])
    }

    pub fn rollup_preamble(&mut self, rollup_count: u8, base_row: u8) {
        let base_row = std::cmp::max(rollup_count, base_row);
        let anchor_vertical = (base_row as u32 * 100 / 14) as u8;
        gst::trace!(
            CAT,
            "rollup_preamble base {base_row} count {rollup_count}, anchor-v {anchor_vertical}"
        );
        let codes = [
            Code::DeleteWindows(!WindowBits::ZERO),
            Code::DefineWindow(DefineWindowArgs::new(
                0,
                0,
                Anchor::BottomMiddle,
                true,
                anchor_vertical,
                50,
                rollup_count - 1,
                31,
                true,
                true,
                true,
                2,
                1,
            )),
            Code::SetPenLocation(SetPenLocationArgs::new(rollup_count - 1, 0)),
        ];
        self.active_window = WindowBits::ZERO;
        self.hidden_window = WindowBits::ONE;
        self.push_codes(&codes)
    }

    pub fn write_char(&mut self, c: char) {
        if let Some(code) = Code::from_char(c) {
            self.push_codes(&[code])
        }
    }

    pub fn push_codes(&mut self, codes: &[Code]) {
        gst::log!(CAT, "pushing codes: {codes:?}");
        self.codes.extend(codes.iter().cloned());
    }

    pub fn etx(&mut self) {
        self.push_codes(&[Code::ETX])
    }

    pub fn carriage_return(&mut self) {
        self.push_codes(&[Code::CR])
    }

    pub fn set_pen_attributes(&mut self, args: SetPenAttributesArgs) {
        self.push_codes(&[Code::SetPenAttributes(args)])
    }

    pub fn set_pen_location(&mut self, args: SetPenLocationArgs) {
        self.push_codes(&[Code::SetPenLocation(args)])
    }

    pub fn set_pen_color(&mut self, args: SetPenColorArgs) {
        self.push_codes(&[Code::SetPenColor(args)])
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ServiceOrChannel {
    Service(u8),
    Cea608Channel(cea608_types::Id),
}

pub struct Cea708Renderer {
    selected: Option<ServiceOrChannel>,
    cea608: Cea608Renderer,
    service: Option<ServiceState>,
    video_width: u32,
    video_height: u32,
    composition: Option<gst_video::VideoOverlayComposition>,

    safe_width: f32,
    safe_height: f32,
}

impl Cea708Renderer {
    pub fn new() -> Self {
        let mut cea608 = Cea608Renderer::new();
        cea608.set_black_background(true);
        Self {
            selected: None,
            cea608,
            service: None,
            video_width: 0,
            video_height: 0,
            composition: None,
            safe_width: 0.8,
            safe_height: 0.8,
        }
    }

    pub fn set_video_size(&mut self, width: u32, height: u32) {
        if width != self.video_width || height != self.video_height {
            self.video_width = width;
            self.video_height = height;
            self.cea608.set_video_size(width, height);
            if let Some(service) = self.service.as_mut() {
                service.set_video_size(width, height);
            }
            self.composition.take();
        }
    }

    pub fn set_safe_title_area(&mut self, safe_width: f32, safe_height: f32) {
        if safe_width != self.safe_width || safe_height != self.safe_height {
            self.safe_width = safe_width;
            self.safe_height = safe_height;
            self.cea608.set_safe_title_area(safe_width, safe_height);
            if let Some(service) = self.service.as_mut() {
                service.set_safe_title_area(safe_width, safe_height);
            }
            self.composition.take();
        }
    }

    pub fn set_service_channel(&mut self, service_channel: Option<ServiceOrChannel>) {
        if self.selected != service_channel {
            self.selected = service_channel;
            match service_channel {
                Some(ServiceOrChannel::Cea608Channel(id)) => self.cea608.set_channel(id.channel()),
                None => self.cea608.set_channel(cea608_types::tables::Channel::ONE),
                _ => (),
            }
        }
    }

    pub fn push_service(&mut self, service: &Service) {
        for code in service.codes() {
            let overlay_service = self.service.get_or_insert_with(|| {
                let mut service = ServiceState::new();
                service.set_video_size(self.video_width, self.video_height);
                service.set_safe_title_area(self.safe_width, self.safe_height);
                service
            });
            overlay_service.handle_code(code);
        }
    }

    pub fn push_cea608(
        &mut self,
        field: cea608_types::tables::Field,
        pair: [u8; 2],
    ) -> Result<bool, cea608_types::ParserError> {
        match self.selected {
            Some(ServiceOrChannel::Cea608Channel(channel)) => {
                if channel.field() != field {
                    // data is not for the configured field, ignore
                    return Ok(false);
                } else {
                    channel
                }
            }
            None => cea608_types::Id::from_caption_field_channel(
                field,
                cea608_types::tables::Channel::ONE,
            ),
            // data is not for the configured service, ignore
            _ => return Ok(false),
        };

        let ret = self.cea608.push_pair(pair);
        if let Ok(changed) = ret {
            if self.selected.is_none() {
                if let Some(chan) = self.cea608.channel() {
                    self.selected = Some(ServiceOrChannel::Cea608Channel(
                        cea608_types::Id::from_caption_field_channel(field, chan),
                    ));
                }
            }
            if changed {
                self.composition.take();
            }
        }
        ret
    }

    pub fn clear_composition(&mut self) {
        self.composition.take();
        if let Some(service) = self.service.as_mut() {
            service.clear();
        }
        self.cea608.clear();
    }

    pub fn generate_composition(&mut self) -> Option<gst_video::VideoOverlayComposition> {
        let Some(selected) = self.selected else {
            self.composition.take();
            return None;
        };

        if matches!(selected, ServiceOrChannel::Service(_)) {
            let service = self.service.as_mut()?;

            let mut composition: Option<gst_video::VideoOverlayComposition> = None;
            for window in service.windows.iter_mut() {
                if let Some(rectangle) = window.generate_rectangle() {
                    if let Some(composition) = composition.as_mut() {
                        composition.get_mut().unwrap().add_rectangle(&rectangle);
                    } else {
                        composition =
                            gst_video::VideoOverlayComposition::new(Some(&rectangle)).ok();
                    }
                }
            }

            self.composition = composition;
        } else if let Some(rectangle) = self.cea608.generate_rectangle() {
            self.composition = gst_video::VideoOverlayComposition::new(Some(&rectangle)).ok();
        }
        self.composition.clone()
    }
}

// SAFETY: Required because `pango::Layout` / `pango::Context` are not `Send` but the whole
// `ServiceState` needs to be.
// We ensure that no additional references to the layout are ever created, which makes it safe
// to send it to other threads as long as only a single thread uses it concurrently.
unsafe impl Send for ServiceState {}

struct ServiceState {
    windows: VecDeque<Window>,
    current_window: usize,
    pango_context: pango::Context,
    video_width: u32,
    video_height: u32,
    width_ratio: f32,
    height_ratio: f32,
}

impl ServiceState {
    fn new() -> Self {
        let fontmap = pangocairo::FontMap::new();
        let context = fontmap.create_context();
        // XXX: may need a different language sometimes
        context.set_language(Some(&pango::Language::from_string("en_US")));
        // XXX: May need a different direction
        context.set_base_dir(pango::Direction::Ltr);
        Self {
            windows: VecDeque::new(),
            current_window: usize::MAX,
            pango_context: context,
            video_width: 0,
            video_height: 0,
            width_ratio: 0.8,
            height_ratio: 0.8,
        }
    }

    fn window_mut(&mut self, id: usize) -> Option<&mut Window> {
        self.windows
            .iter_mut()
            .find(|window| window.define.window_id as usize == id)
    }

    fn define_window(&mut self, args: &DefineWindowArgs) {
        if let Some(window) = self.window_mut(args.window_id as usize) {
            if &window.define != args {
                // we only change these if they are different from the previous define_window
                // command
                window.attrs = args.window_attributes();
                window.pen_attrs = args.pen_attributes();
                window.pen_color = args.pen_color();
            }
            window.define = *args;
            window.recalculate_window_position();
        } else {
            let layout = pango::Layout::new(&self.pango_context);
            // XXX: May need a different alignment
            layout.set_alignment(pango::Alignment::Left);
            self.windows.push_back(Window::new(
                args.visible,
                *args,
                args.window_attributes(),
                args.pen_attributes(),
                args.pen_color(),
                layout,
                Dimensions {
                    w: self.video_width,
                    h: self.video_height,
                },
                self.width_ratio,
                self.height_ratio,
            ));
        };
        self.current_window = args.window_id as usize;
    }

    fn set_current_window(&mut self, window_id: u8) {
        self.current_window = window_id as usize;
    }

    fn clear_windows(&mut self, args: &WindowBits) {
        for window in self.windows.iter_mut() {
            if (WindowBits::from_window_id(window.define.window_id) & *args) != WindowBits::NONE {
                window.pen_location = SetPenLocationArgs::default();
                window.lines.clear();
                window.rectangle = None;
            }
        }
    }

    fn delete_windows(&mut self, args: &WindowBits) {
        self.windows.retain(|window| {
            (WindowBits::from_window_id(window.define.window_id) & *args) == WindowBits::NONE
        });
    }

    fn display_windows(&mut self, args: &WindowBits) {
        for window in self.windows.iter_mut() {
            if (WindowBits::from_window_id(window.define.window_id) & *args) != WindowBits::NONE {
                window.visible = true;
            }
        }
    }

    fn hide_windows(&mut self, args: &WindowBits) {
        for window in self.windows.iter_mut() {
            if (WindowBits::from_window_id(window.define.window_id) & *args) != WindowBits::NONE {
                window.visible = false;
            }
        }
    }

    fn toggle_windows(&mut self, args: &WindowBits) {
        for window in self.windows.iter_mut() {
            if (WindowBits::from_window_id(window.define.window_id) & *args) != WindowBits::NONE {
                window.visible = !window.visible;
            }
        }
    }

    fn set_window_attributes(&mut self, attrs: &SetWindowAttributesArgs) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        if &window.attrs != attrs {
            window.lines.clear();
            window.attrs = *attrs;
            window.rectangle = None;
        }
    }

    fn set_pen_attributes(&mut self, attrs: &SetPenAttributesArgs) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.pen_attrs = *attrs;
    }

    fn set_pen_color(&mut self, color: &SetPenColorArgs) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.pen_color = *color;
    }

    fn set_pen_location(&mut self, location: &SetPenLocationArgs) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.pen_location = *location;
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn clear(&mut self) {
        for window in self.windows.iter_mut() {
            window.lines.clear();
            window.pen_location = SetPenLocationArgs::default();
            window.rectangle.take();
        }
    }

    fn handle_code(&mut self, code: &Code) {
        match code {
            Code::DefineWindow(args) => self.define_window(args),
            Code::SetCurrentWindow0 => self.set_current_window(0),
            Code::SetCurrentWindow1 => self.set_current_window(1),
            Code::SetCurrentWindow2 => self.set_current_window(2),
            Code::SetCurrentWindow3 => self.set_current_window(3),
            Code::SetCurrentWindow4 => self.set_current_window(4),
            Code::SetCurrentWindow5 => self.set_current_window(5),
            Code::SetCurrentWindow6 => self.set_current_window(6),
            Code::SetCurrentWindow7 => self.set_current_window(7),
            Code::ClearWindows(args) => self.clear_windows(args),
            Code::DeleteWindows(args) => self.delete_windows(args),
            Code::DisplayWindows(args) => self.display_windows(args),
            Code::HideWindows(args) => self.hide_windows(args),
            Code::ToggleWindows(args) => self.toggle_windows(args),
            Code::SetWindowAttributes(args) => self.set_window_attributes(args),
            Code::SetPenAttributes(args) => self.set_pen_attributes(args),
            Code::SetPenColor(args) => self.set_pen_color(args),
            Code::SetPenLocation(args) => self.set_pen_location(args),
            Code::BS => self.backspace(),
            Code::CR => self.carriage_return(),
            Code::FF => {
                self.clear_windows(&WindowBits::from_window_id(self.current_window as u8));
                self.set_pen_location(&SetPenLocationArgs { row: 0, column: 0 });
            }
            Code::ETX => (),
            Code::HCR => self.horizontal_carriage_return(),
            Code::Reset => self.reset(),
            _ => {
                if let Some(ch) = code.char() {
                    self.push_char(ch);
                }
            }
        }
    }

    fn push_char(&mut self, ch: char) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.push_char(ch);
    }

    fn backspace(&mut self) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.backspace();
    }

    fn carriage_return(&mut self) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.carriage_return();
    }

    fn horizontal_carriage_return(&mut self) {
        let Some(window) = self.window_mut(self.current_window) else {
            return;
        };
        window.horizontal_carriage_return();
    }

    fn set_video_size(&mut self, video_width: u32, video_height: u32) {
        for window in self.windows.iter_mut() {
            window.set_video_size(video_width, video_height);
        }
        self.video_width = video_width;
        self.video_height = video_height;
    }

    fn set_safe_title_area(&mut self, width_ratio: f32, height_ratio: f32) {
        for window in self.windows.iter_mut() {
            window.set_safe_title_area(width_ratio, height_ratio);
        }
        self.width_ratio = width_ratio;
        self.height_ratio = height_ratio;
    }
}

fn color_value_as_u16(val: ColorValue) -> u16 {
    match val {
        ColorValue::None => 0,
        ColorValue::OneThird => u16::MAX / 3,
        ColorValue::TwoThirds => u16::MAX / 3 * 2,
        ColorValue::Full => u16::MAX,
    }
}

fn opacity_as_u16(val: Opacity) -> u16 {
    match val {
        Opacity::Transparent => 0,
        Opacity::Translucent => u16::MAX / 3,
        // FIXME
        Opacity::Flash => u16::MAX / 3 * 2,
        Opacity::Solid => u16::MAX,
    }
}

fn pango_foreground_color_from_708(args: &SetPenColorArgs) -> pango::AttrColor {
    pango::AttrColor::new_foreground(
        color_value_as_u16(args.foreground_color.r),
        color_value_as_u16(args.foreground_color.g),
        color_value_as_u16(args.foreground_color.b),
    )
}

fn pango_foreground_opacity_from_708(args: &SetPenColorArgs) -> pango::AttrInt {
    pango::AttrInt::new_foreground_alpha(opacity_as_u16(args.foreground_opacity))
}

fn pango_background_color_from_708(args: &SetPenColorArgs) -> pango::AttrColor {
    pango::AttrColor::new_background(
        color_value_as_u16(args.background_color.r),
        color_value_as_u16(args.background_color.g),
        color_value_as_u16(args.background_color.b),
    )
}

fn pango_background_opacity_from_708(args: &SetPenColorArgs) -> pango::AttrInt {
    pango::AttrInt::new_background_alpha(opacity_as_u16(args.background_opacity))
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Dimensions {
    w: u32,
    h: u32,
}

pub enum Align {
    First,
    Center,
    Last,
}

impl Align {
    fn horizontal_from_anchor(anchor: Anchor) -> Self {
        match anchor {
            Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => Self::First,
            Anchor::TopMiddle | Anchor::CenterMiddle | Anchor::BottomMiddle => Self::Center,
            Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => Self::Last,
            _ => Self::Center,
        }
    }

    fn vertical_from_anchor(anchor: Anchor) -> Self {
        match anchor {
            Anchor::TopLeft | Anchor::TopMiddle | Anchor::TopRight => Self::First,
            Anchor::CenterLeft | Anchor::CenterMiddle | Anchor::CenterRight => Self::Center,
            Anchor::BottomLeft | Anchor::BottomMiddle | Anchor::BottomRight => Self::Last,
            _ => Self::Last,
        }
    }
}

struct Window {
    visible: bool,
    define: DefineWindowArgs,
    attrs: SetWindowAttributesArgs,
    pen_attrs: SetPenAttributesArgs,
    pen_color: SetPenColorArgs,
    pen_location: SetPenLocationArgs,
    lines: VecDeque<WindowLine>,

    safe_width: f32,
    safe_height: f32,
    window_position: Dimensions,
    video_dims: Dimensions,
    window_dims: Dimensions,
    max_layout_dims: Dimensions,
    rectangle: Option<gst_video::VideoOverlayRectangle>,
    layout: pango::Layout,
}

impl Window {
    #[allow(clippy::too_many_arguments)]
    fn new(
        visible: bool,
        define: DefineWindowArgs,
        attrs: SetWindowAttributesArgs,
        pen_attrs: SetPenAttributesArgs,
        pen_color: SetPenColorArgs,
        layout: pango::Layout,
        video_dims: Dimensions,
        width_ratio: f32,
        height_ratio: f32,
    ) -> Self {
        let mut ret = Self {
            visible,
            define,
            attrs,
            pen_attrs,
            pen_color,
            pen_location: SetPenLocationArgs::default(),
            lines: VecDeque::new(),
            rectangle: None,
            layout,
            video_dims: Dimensions::default(),
            window_position: Dimensions::default(),
            window_dims: Dimensions::default(),
            max_layout_dims: Dimensions::default(),
            safe_width: width_ratio,
            safe_height: height_ratio,
        };
        ret.set_video_size(video_dims.w, video_dims.h);
        ret
    }

    fn dump(&self) {
        for line in self.lines.iter() {
            let mut string = line.no.to_string();
            string.push(' ');
            for cell in line.line.iter() {
                string.push(cell.character.unwrap_or(' '));
            }
            string.push('|');
            gst::trace!(CAT, "dump: {string}");
        }
    }

    fn ensure_cell(&mut self, row: usize, column: usize) {
        let line = if let Some(line) = self.lines.iter_mut().find(|line| line.no == row) {
            line
        } else {
            self.lines.push_back(WindowLine {
                no: row,
                line: VecDeque::new(),
            });
            self.lines.back_mut().unwrap()
        };
        while line.line.len() <= column {
            line.line
                .push_back(Cell::new_empty(self.pen_attrs, self.pen_color));
        }
    }

    fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        self.lines
            .iter_mut()
            .find(|line| line.no == row)
            .and_then(|line| line.line.get_mut(column))
    }

    fn backspace(&mut self) {
        match self.attrs.print_direction {
            Direction::LeftToRight => {
                self.pen_location.column = self.pen_location.column.max(1) - 1;
            }
            Direction::RightToLeft => {
                self.pen_location.column = (self.pen_location.column + 1).min(self.column_count());
            }
            Direction::TopToBottom => {
                self.pen_location.row = self.pen_location.row.max(1) - 1;
            }
            Direction::BottomToTop => {
                self.pen_location.row = (self.pen_location.row + 1).min(self.row_count());
            }
        }
        self.ensure_cell(
            self.pen_location.row as usize,
            self.pen_location.column as usize,
        );
        let cell = self
            .cell_mut(
                self.pen_location.row as usize,
                self.pen_location.column as usize,
            )
            .unwrap();
        if cell.character.take().is_some() {
            self.rectangle.take();
        }
    }

    fn row_count(&self) -> u8 {
        self.define.row_count + 1
    }

    fn column_count(&self) -> u8 {
        self.define.column_count + 1
    }

    fn move_to_line_beginning(&mut self) {
        match self.attrs.print_direction {
            Direction::LeftToRight => {
                self.pen_location.column = 0;
            }
            Direction::RightToLeft => {
                self.pen_location.column = self.define.column_count;
            }
            Direction::TopToBottom => {
                self.pen_location.row = 0;
            }
            Direction::BottomToTop => {
                self.pen_location.row = self.row_count();
            }
        }
    }

    fn scroll_top_to_bottom(&mut self) {
        if self.pen_location.row == 0 {
            let row_count = self.row_count() as usize;
            self.lines
                .retain(|line| (0..=row_count - 1).contains(&line.no));
            for line in self.lines.iter_mut() {
                line.no += 1;
            }
        } else {
            self.pen_location.row -= 1;
        }
    }

    fn scroll_bottom_to_top(&mut self) {
        if self.pen_location.row >= self.define.row_count {
            let row_count = self.row_count() as usize;
            self.lines.retain(|line| (1..=row_count).contains(&line.no));
            for line in self.lines.iter_mut() {
                line.no -= 1
            }
        } else {
            self.pen_location.row += 1;
        }
    }

    fn scroll_left_to_right(&mut self) {
        if self.pen_location.column == 0 {
            gst::warning!(CAT, "Unsupported scroll direction left-to-right");
        } else {
            self.pen_location.column -= 1;
        }
    }

    fn scroll_right_to_left(&mut self) {
        if self.pen_location.column >= self.column_count() {
            gst::warning!(CAT, "Unsupported scroll direction right-to-left");
        } else {
            self.pen_location.column += 1;
        }
    }

    fn carriage_return(&mut self) {
        match (self.attrs.print_direction, self.attrs.scroll_direction) {
            (Direction::LeftToRight, Direction::TopToBottom) => {
                self.scroll_top_to_bottom();
                self.move_to_line_beginning();
            }
            (Direction::LeftToRight, Direction::BottomToTop) => {
                self.scroll_bottom_to_top();
                self.move_to_line_beginning();
            }
            (Direction::RightToLeft, Direction::TopToBottom) => {
                self.scroll_top_to_bottom();
                self.move_to_line_beginning();
            }
            (Direction::RightToLeft, Direction::BottomToTop) => {
                self.scroll_bottom_to_top();
                self.move_to_line_beginning();
            }
            (Direction::TopToBottom, Direction::LeftToRight) => {
                self.scroll_left_to_right();
                self.move_to_line_beginning();
            }
            (Direction::TopToBottom, Direction::RightToLeft) => {
                self.scroll_right_to_left();
                self.move_to_line_beginning();
            }
            (Direction::BottomToTop, Direction::LeftToRight) => {
                self.scroll_left_to_right();
                self.move_to_line_beginning();
            }
            (Direction::BottomToTop, Direction::RightToLeft) => {
                self.scroll_right_to_left();
                self.move_to_line_beginning();
            }
            // all other variants invalid
            (print, scroll) => {
                gst::warning!(
                    CAT,
                    "Unspecified print direction ({print:?}) and scroll direction ({scroll:?})"
                );
                return;
            }
        }
        gst::trace!(
            CAT,
            "carriage return after position {},{}",
            self.pen_location.row,
            self.pen_location.column
        );
        self.rectangle.take();
    }

    fn horizontal_carriage_return(&mut self) {
        let min_row;
        let max_row;
        let min_column;
        let max_column;
        match self.attrs.print_direction {
            Direction::LeftToRight => {
                min_row = self.pen_location.row;
                max_row = self.pen_location.row;
                max_column = self.pen_location.column;
                min_column = 0;
                self.pen_location.column = 0;
            }
            Direction::RightToLeft => {
                min_row = self.pen_location.row;
                max_row = self.pen_location.row;
                min_column = self.pen_location.column;
                max_column = self.row_count();
                self.pen_location.column = self.row_count();
            }
            Direction::TopToBottom => {
                min_column = self.pen_location.column;
                max_column = self.pen_location.column;
                min_row = self.pen_location.row;
                max_row = 0;
                self.pen_location.row = 0;
            }
            Direction::BottomToTop => {
                min_column = self.pen_location.column;
                max_column = self.pen_location.column;
                min_row = self.pen_location.row;
                max_row = self.column_count();
                self.pen_location.row = self.column_count();
            }
        }
        for row in min_row..=max_row {
            for column in min_column..=max_column {
                self.ensure_cell(row as usize, column as usize);
                let cell = self.cell_mut(row as usize, column as usize).unwrap();
                cell.character = None;
            }
        }
        self.rectangle.take();
    }

    fn push_char(&mut self, ch: char) {
        if self.pen_location.row > self.row_count() {
            gst::warning!(
                CAT,
                "row {} outside configured window row count {}",
                self.pen_location.row,
                self.row_count()
            );
            return;
        }
        if self.pen_location.column > self.column_count() {
            gst::warning!(
                CAT,
                "column {} outside configured window column count {}",
                self.pen_location.column,
                self.column_count()
            );
            return;
        }
        gst::trace!(
            CAT,
            "push char \'{ch}\' at row {} column {}",
            self.pen_location.row,
            self.pen_location.column
        );
        self.ensure_cell(
            self.pen_location.row as usize,
            self.pen_location.column as usize,
        );
        let cell = self
            .cell_mut(
                self.pen_location.row as usize,
                self.pen_location.column as usize,
            )
            .unwrap();
        cell.character = Some(ch);
        self.rectangle.take();

        match self.attrs.print_direction {
            Direction::LeftToRight => {
                self.pen_location.column = (self.pen_location.column + 1).min(self.column_count());
            }
            Direction::RightToLeft => {
                self.pen_location.column = self.pen_location.column.max(1) - 1;
            }
            Direction::TopToBottom => {
                self.pen_location.row = (self.pen_location.row + 1).min(self.row_count());
            }
            Direction::BottomToTop => {
                self.pen_location.row = self.pen_location.row.max(1) - 1;
            }
        }
    }

    fn recalculate_window_position(&mut self) {
        self.rectangle.take();

        // XXX: may need a better implementation for 'skinny' (horizontal or vertical) output
        // sizes.

        let safe_area = Dimensions {
            w: (self.video_dims.w as f32 * self.safe_width) as u32,
            h: (self.video_dims.h as f32 * self.safe_height) as u32,
        };
        let (max_layout_width, max_layout_height) =
            recalculate_pango_layout(&self.layout, safe_area.w, safe_area.h);
        self.max_layout_dims = Dimensions {
            w: max_layout_width as u32,
            h: max_layout_height as u32,
        };

        let char_width = max_layout_width as u32 / 32;
        let char_height = max_layout_height as u32 / 15;
        let height = self.row_count() as u32 * char_height;
        let width = self.column_count() as u32 * char_width;
        self.window_dims = Dimensions {
            w: width,
            h: height,
        };

        let padding = Dimensions {
            w: (self.video_dims.w - safe_area.w) / 2,
            h: (self.video_dims.h - safe_area.h) / 2,
        };

        self.window_position = if self.define.relative_positioning {
            let halign = Align::horizontal_from_anchor(self.define.anchor_point);
            let valign = Align::vertical_from_anchor(self.define.anchor_point);
            let x = safe_area
                .w
                .mul_div_round(self.define.anchor_horizontal.min(100) as u32, 100)
                .unwrap();
            let x = padding.w
                + match halign {
                    Align::First => x,
                    Align::Center => x.max(self.max_layout_dims.w / 2) - self.max_layout_dims.w / 2,
                    Align::Last => x.max(self.window_dims.w) - self.window_dims.w,
                };
            let y = safe_area
                .h
                .mul_div_round(self.define.anchor_vertical.min(100) as u32, 100)
                .unwrap();
            let y = padding.h
                + match valign {
                    Align::First => y,
                    Align::Center => y.max(self.max_layout_dims.h / 2),
                    Align::Last => y.max(self.window_dims.h) - self.window_dims.h,
                };
            Dimensions { w: x, h: y }
        } else {
            // Non-relative positioning implementation
            let aspect_ratio = self.video_dims.w as f32 / self.video_dims.h as f32;
            let is_widescreen = (aspect_ratio - 1.777).abs() < 0.01; // Allow a small tolerance
            let (grid_width, grid_height) = if is_widescreen {
                (209_f32, 74_f32) // 16:9 format
            } else {
                (159_f32, 74_f32) // 4:3 format
            };

            // Map anchor_horizontal and anchor_vertical to absolute grid coordinates
            let x = self.define.anchor_horizontal.min(grid_width as u8) as f32;
            let y = self.define.anchor_vertical.min(grid_height as u8) as f32;

            // Convert grid coordinates to pixel coordinates within the safe-title area
            let grid_x = padding.w + ((safe_area.w as f32) * (x / grid_width)) as u32;
            let grid_y = padding.h + ((safe_area.h as f32) * (y / grid_height)) as u32;

            // Calculate the final x position with horizontal alignment applied
            let halign = Align::horizontal_from_anchor(self.define.anchor_point);
            let valign = Align::vertical_from_anchor(self.define.anchor_point);

            let x = match halign {
                Align::First => grid_x,
                Align::Center => grid_x.saturating_sub(self.max_layout_dims.w / 2),
                Align::Last => grid_x.saturating_sub(self.window_dims.w),
            };

            // Calculate the final y position with vertical alignment applied
            let y = match valign {
                Align::First => grid_y,
                Align::Center => grid_y.saturating_sub(self.max_layout_dims.h / 2),
                Align::Last => grid_y.saturating_sub(self.window_dims.h),
            };

            let center = x + (self.video_dims.w / 2) - (self.window_dims.w / 2);

            Dimensions { w: center, h: y }
        };

        gst::trace!(
            CAT,
            "char sizes {char_width}x{char_height}, row/columns {}x{}, safe area {:?} window dimensions: {:?}, window position: {:?}, max layout {:?}, define {:?}",
            self.row_count(),
            self.column_count(),
            safe_area,
            self.window_dims,
            self.window_position,
            self.max_layout_dims,
            self.define,
        );
    }

    fn set_video_size(&mut self, video_width: u32, video_height: u32) {
        let new_dims = Dimensions {
            w: video_width,
            h: video_height,
        };
        if new_dims == self.video_dims {
            return;
        }
        self.video_dims = new_dims;

        self.recalculate_window_position();
    }

    fn set_safe_title_area(&mut self, width_ratio: f32, height_ratio: f32) {
        if self.safe_width == width_ratio && self.safe_height == height_ratio {
            return;
        }
        self.safe_width = width_ratio;
        self.safe_height = height_ratio;

        self.recalculate_window_position();
    }

    fn generate_rectangle(&mut self) -> Option<gst_video::VideoOverlayRectangle> {
        if !self.visible {
            return None;
        }

        if self.rectangle.is_some() {
            return self.rectangle.clone();
        }
        self.dump();

        // 1. generate the pango layout for the text
        let mut text = String::new();
        let attrs = pango::AttrList::new();
        let mut last_color = None;
        let mut last_attrs = None;
        let mut background_color_attr = pango::AttrColor::new_background(0, 0, 0);
        let mut background_opacity_attr = pango::AttrInt::new_background_alpha(0);
        let mut foreground_color_attr =
            pango::AttrColor::new_background(u16::MAX, u16::MAX, u16::MAX);
        let mut foreground_opacity_attr = pango::AttrInt::new_background_alpha(u16::MAX);
        let mut underline_attr = pango::AttrInt::new_underline(pango::Underline::None);
        let mut italic_attr = None::<pango::AttrInt>;
        let mut last_row = 0;
        for line in self.lines.iter() {
            for _ in 0..line.no - last_row {
                text.push('\n');
            }
            last_row = line.no;
            for c in line.line.iter() {
                // XXX: Need to double check these indices with more complicated text characters
                let start_idx = text.len();
                if last_color.map(|col| col != c.pen_color).unwrap_or(true) {
                    background_color_attr.set_end_index(start_idx as u32);
                    attrs.insert(background_color_attr.clone());
                    background_color_attr = pango_background_color_from_708(&c.pen_color);
                    background_color_attr.set_start_index(start_idx as u32);

                    background_opacity_attr.set_end_index(start_idx as u32);
                    attrs.insert(background_opacity_attr.clone());
                    background_opacity_attr = pango_background_opacity_from_708(&c.pen_color);
                    background_opacity_attr.set_start_index(start_idx as u32);

                    foreground_color_attr.set_end_index(start_idx as u32);
                    attrs.insert(foreground_color_attr.clone());
                    foreground_color_attr = pango_foreground_color_from_708(&c.pen_color);
                    foreground_color_attr.set_start_index(start_idx as u32);

                    foreground_opacity_attr.set_end_index(start_idx as u32);
                    attrs.insert(foreground_opacity_attr.clone());
                    foreground_opacity_attr = pango_foreground_opacity_from_708(&c.pen_color);
                    foreground_opacity_attr.set_start_index(start_idx as u32);

                    last_color = Some(c.pen_color);
                }
                if last_attrs.map(|attrs| attrs != c.pen_attrs).unwrap_or(true) {
                    underline_attr.set_end_index(start_idx as u32);
                    attrs.insert(underline_attr.clone());
                    let underline_type = if c.pen_attrs.underline {
                        pango::Underline::Single
                    } else {
                        pango::Underline::None
                    };
                    underline_attr = pango::AttrInt::new_underline(underline_type);
                    underline_attr.set_start_index(start_idx as u32);

                    if !c.pen_attrs.italics {
                        if let Some(mut italic) = italic_attr.take() {
                            italic.set_end_index(start_idx as u32);
                            attrs.insert(italic.clone());
                        }
                    } else if c.pen_attrs.italics && italic_attr.is_none() {
                        let mut attr = pango::AttrInt::new_style(pango::Style::Italic);
                        attr.set_start_index(start_idx as u32);
                        italic_attr = Some(attr);
                    }

                    last_attrs = Some(c.pen_attrs);
                }

                let Some(character) = c.character else {
                    text.push(' ');
                    continue;
                };
                text.push(character);
            }
        }
        let start_idx = text.len();
        background_color_attr.set_end_index(start_idx as u32);
        attrs.insert(background_color_attr.clone());
        background_opacity_attr.set_end_index(start_idx as u32);
        attrs.insert(background_opacity_attr.clone());
        foreground_color_attr.set_end_index(start_idx as u32);
        attrs.insert(foreground_color_attr.clone());
        foreground_opacity_attr.set_end_index(start_idx as u32);
        attrs.insert(foreground_opacity_attr.clone());
        underline_attr.set_end_index(start_idx as u32);
        attrs.insert(underline_attr.clone());
        if let Some(mut italic) = italic_attr {
            italic.set_end_index(start_idx as u32);
            attrs.insert(italic);
        }

        self.layout.set_text(&text);
        self.layout.set_attributes(Some(&attrs));
        let (_ink_rect, logical_rect) = self.layout.extents();
        let height = logical_rect.height() / pango::SCALE;
        let width = logical_rect.width() / pango::SCALE;

        // 2. render text and window
        let render_buffer = || -> Result<gst::Buffer, anyhow::Error> {
            let mut buffer = gst::Buffer::with_size((width * height) as usize * 4)?;

            gst_video::VideoMeta::add(
                buffer.get_mut().unwrap(),
                gst_video::VideoFrameFlags::empty(),
                #[cfg(target_endian = "little")]
                gst_video::VideoFormat::Bgra,
                #[cfg(target_endian = "big")]
                gst_video::VideoFormat::Argb,
                width as u32,
                height as u32,
            )?;
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
            )?;

            let cr = cairo::Context::new(&surface)?;

            // Clear background
            cr.set_operator(cairo::Operator::Source);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            cr.paint()?;

            // Render text outline
            cr.save()?;
            cr.set_operator(cairo::Operator::Over);

            cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);

            pangocairo::functions::layout_path(&cr, &self.layout);
            cr.stroke()?;
            cr.restore()?;

            // Render text
            cr.save()?;
            cr.set_source_rgba(255.0, 255.0, 255.0, 1.0);

            pangocairo::functions::show_layout(&cr, &self.layout);

            cr.restore()?;
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
                Ok(buffer)
            }
        };

        let buffer = if width > 0 && height > 0 {
            match render_buffer() {
                Ok(buffer) => buffer,
                Err(e) => {
                    self.dump();
                    gst::error!(CAT, "Failed to render buffer: \"{e}\"");
                    return None;
                }
            }
        } else {
            return None;
        };
        gst::trace!(
            CAT,
            "sizes: video {:?}, window {:?} overlay {}x{}",
            self.video_dims,
            self.window_dims,
            width,
            height
        );

        // 3. generate overlay rectangle
        // FIXME: use the window location values to place the overlay
        let ret = Some(gst_video::VideoOverlayRectangle::new_raw(
            &buffer,
            self.window_position.w as i32,
            self.window_position.h as i32,
            width as u32,
            height as u32,
            gst_video::VideoOverlayFormatFlags::PREMULTIPLIED_ALPHA,
        ));
        self.rectangle.clone_from(&ret);
        ret
    }
}

struct WindowLine {
    no: usize,
    line: VecDeque<Cell>,
}

impl PartialOrd for WindowLine {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WindowLine {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.no.cmp(&other.no)
    }
}

impl PartialEq for WindowLine {
    fn eq(&self, other: &Self) -> bool {
        self.no == other.no
    }
}

impl Eq for WindowLine {}

struct Cell {
    character: Option<char>,
    pen_attrs: SetPenAttributesArgs,
    pen_color: SetPenColorArgs,
}

impl Cell {
    fn new_empty(attrs: SetPenAttributesArgs, color: SetPenColorArgs) -> Self {
        Self {
            character: None,
            pen_attrs: attrs,
            pen_color: color,
        }
    }
}
