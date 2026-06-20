// Copyright (C) 2024 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_video::prelude::*;

use std::sync::LazyLock;

use std::sync::Mutex;

use crate::ccutils::extract_cdp;
use crate::cea708utils::{Cea708Renderer, ServiceOrChannel};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea708overlay",
        gst::DebugColorFlags::empty(),
        Some("CEA 708 overlay element"),
    )
});

const DEFAULT_CEA608_CHANNEL: i32 = -1;
const DEFAULT_SERVICE: i32 = 1;
const DEFAULT_SAFE_WIDTH: f32 = 0.8;
const DEFAULT_SAFE_HEIGHT: f32 = 0.8;

#[derive(Debug, Clone)]
struct Settings {
    changed: bool,
    cea608_channel: i32,
    service: i32,
    timeout: Option<gst::ClockTime>,
    safe_width: f32,
    safe_height: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            changed: true,
            cea608_channel: DEFAULT_CEA608_CHANNEL,
            service: DEFAULT_SERVICE,
            timeout: gst::ClockTime::NONE,
            safe_width: DEFAULT_SAFE_WIDTH,
            safe_height: DEFAULT_SAFE_HEIGHT,
        }
    }
}

struct State {
    selected: Option<ServiceOrChannel>,
    enabled_608: bool,
    enabled_708: bool,

    upstream_caps: Option<gst::Caps>,
    video_info: Option<gst_video::VideoInfo>,
    cc_data_parser: cea708_types::CCDataParser,
    cea708_renderer: Cea708Renderer,
    attach: bool,
    last_cc_pts: Option<gst::ClockTime>,
}

impl Default for State {
    fn default() -> Self {
        let mut cc_data_parser = cea708_types::CCDataParser::default();
        cc_data_parser.handle_cea608();
        Self {
            selected: None,
            enabled_608: true,
            enabled_708: true,
            upstream_caps: None,
            video_info: None,
            cc_data_parser,
            cea708_renderer: Cea708Renderer::new(),
            attach: false,
            last_cc_pts: gst::ClockTime::NONE,
        }
    }
}

pub struct Cea708Overlay {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl Cea708Overlay {
    fn render(&self, state: &mut State) -> Option<gst_video::VideoOverlayComposition> {
        state.cea708_renderer.generate_composition()
    }

    fn check_service_channel(&self, state: &mut State) {
        let mut settings = self.settings.lock().unwrap();
        if !settings.changed {
            return;
        }
        state.selected = match settings.service {
            -1 => state.selected,
            0 => {
                if matches!(state.selected, Some(ServiceOrChannel::Service(_))) {
                    None
                } else {
                    state.selected
                }
            }
            val => Some(ServiceOrChannel::Service(val as u8)),
        };
        if state.selected.is_none() || settings.cea608_channel == 0 {
            state.selected = match settings.cea608_channel {
                -1 => state.selected,
                0 => {
                    if matches!(state.selected, Some(ServiceOrChannel::Cea608Channel(_))) {
                        None
                    } else {
                        state.selected
                    }
                }
                val => Some(ServiceOrChannel::Cea608Channel(
                    cea608_types::Id::from_value(val as i8),
                )),
            };
        }
        state.enabled_608 = settings.cea608_channel != 0;
        state.enabled_708 = settings.service != 0;
        gst::info!(
            CAT,
            "set service channel {:?}, from settings: {settings:?}",
            state.selected
        );

        state.cea708_renderer.set_service_channel(state.selected);
        state
            .cea708_renderer
            .set_safe_title_area(settings.safe_width, settings.safe_height);
        settings.changed = false;
    }

    fn negotiate(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        let Some(caps) = state.upstream_caps.as_ref() else {
            gst::element_imp_error!(
                self,
                gst::CoreError::Negotiation,
                ["Element hasn't received valid video caps at negotiation time"]
            );
            self.srcpad.mark_reconfigure();
            return Err(gst::FlowError::NotNegotiated);
        };

        let Some(video_info) = state.video_info.clone() else {
            gst::element_imp_error!(
                self,
                gst::CoreError::Negotiation,
                ["Element hasn't received valid video caps at negotiation time"]
            );
            self.srcpad.mark_reconfigure();
            return Err(gst::FlowError::NotNegotiated);
        };

        let mut downstream_accepts_meta = false;
        let mut caps = caps.clone();

        let upstream_has_meta = state
            .upstream_caps
            .as_ref()
            .and_then(|caps| {
                caps.features(0)
                    .map(|f| f.contains(gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION))
            })
            .unwrap_or(false);

        if !upstream_has_meta {
            let mut caps_clone = caps.clone();
            let overlay_caps = caps_clone.make_mut();

            if let Some(features) = overlay_caps.features_mut(0) {
                let is_sysmem = features.is_empty()
                    || features
                        .iter()
                        .all(|f| f == gst::CAPS_FEATURE_MEMORY_SYSTEM_MEMORY);
                features.add(gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION);
                drop(state);
                let peercaps = self.srcpad.peer_query_caps(Some(&caps_clone));
                downstream_accepts_meta = !peercaps.is_empty();
                if downstream_accepts_meta {
                    caps = caps_clone;
                } else if !is_sysmem {
                    gst::element_imp_error!(
                        self,
                        gst::CoreError::Negotiation,
                        ("Provided input caps with features but downstream does not support meta::GstVideoOverlayComposition")
                    );
                    self.srcpad.mark_reconfigure();
                    return Err(gst::FlowError::NotNegotiated);
                }
                state = self.state.lock().unwrap();
            }
        }

        state.attach = upstream_has_meta || downstream_accepts_meta;
        state
            .cea708_renderer
            .set_video_size(video_info.width(), video_info.height());

        if !self.srcpad.push_event(gst::event::Caps::new(&caps)) {
            self.srcpad.mark_reconfigure();
            Err(gst::FlowError::NotNegotiated)
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn have_cea608(
        &self,
        state: &mut State,
        field: cea608_types::tables::Field,
        cea608: [u8; 2],
        pts: gst::ClockTime,
    ) {
        gst::trace!(
            CAT,
            imp = self,
            "Handling CEA-608 for field {field:?} (selected: {:?}) data: {cea608:x?}",
            state.selected
        );

        match state.cea708_renderer.push_cea608(field, cea608) {
            Err(e) => gst::warning!(CAT, imp = self, "Failed to parse CEA-608 data: {e:?}"),
            Ok(true) => self.reset_timeout(state, pts),
            _ => (),
        }
    }

    fn handle_cc_data(&self, state: &mut State, pts: gst::ClockTime) {
        let cea608 = state.cc_data_parser.cea608().map(|c| c.to_vec());

        while let Some(packet) = state.cc_data_parser.pop_packet() {
            if !state.enabled_708
                || !matches!(state.selected, None | Some(ServiceOrChannel::Service(_)))
            {
                continue;
            }

            for service in packet.services() {
                if state.selected.is_none() {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Automatic selection chose CEA-708 service {}",
                        service.number()
                    );
                    state.selected = Some(ServiceOrChannel::Service(service.number()));
                }
                if Some(ServiceOrChannel::Service(service.number())) != state.selected {
                    continue;
                }

                state.cea708_renderer.push_service(service);
                self.reset_timeout(state, pts);
            }
        }

        let Some(cea608) = cea608 else {
            gst::log!(CAT, imp = self, "No CEA-608");
            return;
        };

        if !state.enabled_608
            || !matches!(
                state.selected,
                None | Some(ServiceOrChannel::Cea608Channel(_))
            )
        {
            gst::log!(
                CAT,
                imp = self,
                "CEA-608 not to be used (enabled {}, selected {:?})",
                state.enabled_608,
                state.selected
            );
            return;
        }

        for pair in cea608 {
            let (field, pair) = match pair {
                cea708_types::Cea608::Field1(byte0, byte1) => {
                    (cea608_types::tables::Field::ONE, [byte0, byte1])
                }
                cea708_types::Cea608::Field2(byte0, byte1) => {
                    (cea608_types::tables::Field::TWO, [byte0, byte1])
                }
            };

            self.have_cea608(state, field, pair, pts);
        }
    }

    fn decode_s334_1a(&self, state: &mut State, data: &[u8], pts: gst::ClockTime) {
        if data.len() % 3 != 0 {
            gst::warning!(CAT, "cc_data length is not a multiple of 3, truncating");
        }

        for triple in data.chunks_exact(3) {
            let field = if (triple[0] & 0x80) == 0x80 {
                cea608_types::tables::Field::ONE
            } else {
                cea608_types::tables::Field::TWO
            };

            self.have_cea608(state, field, [triple[1], triple[2]], pts);
        }
    }

    fn reset_timeout(&self, state: &mut State, pts: gst::ClockTime) {
        gst::trace!(CAT, "resetting timeout to {pts:?}");
        state.last_cc_pts = Some(pts);
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let pts = buffer.pts().ok_or_else(|| {
            gst::error!(CAT, obj = pad, "Require timestamped buffers");
            gst::FlowError::Error
        })?;

        let settings = self.settings.lock().unwrap();
        let caption_timeout = settings.timeout;
        drop(settings);

        if self.srcpad.check_reconfigure() {
            self.negotiate()?;
        }

        let mut state = self.state.lock().unwrap();
        self.check_service_channel(&mut state);

        for meta in buffer.iter_meta::<gst_video::VideoCaptionMeta>() {
            gst::log!(
                CAT,
                imp = self,
                "Have caption meta of type {:?}",
                meta.caption_type()
            );

            if meta.caption_type() == gst_video::VideoCaptionType::Cea708Cdp {
                match extract_cdp(meta.data()) {
                    Ok(data) => {
                        let mut cc_data = vec![0x80 | 0x40 | ((data.len() / 3) & 0x1f) as u8, 0xFF];
                        cc_data.extend(data);
                        match state.cc_data_parser.push(&cc_data) {
                            Ok(_) => self.handle_cc_data(&mut state, pts),
                            Err(e) => {
                                gst::warning!(CAT, "Failed to parse incoming data: {e}");
                                gst::element_imp_warning!(
                                    self,
                                    gst::StreamError::Decode,
                                    ["Failed to parse incoming data {e}"]
                                );
                                state.cc_data_parser.flush();
                            }
                        }
                    }
                    Err(e) => {
                        gst::warning!(CAT, "{e}");
                        gst::element_imp_warning!(self, gst::StreamError::Decode, ["{e}"]);
                    }
                }
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea708Raw {
                let mut cc_data = vec![0; 2];
                // reserved | process_cc_data | length
                cc_data[0] = 0x80 | 0x40 | ((meta.data().len() / 3) & 0x1f) as u8;
                cc_data[1] = 0xFF;
                cc_data.extend(meta.data());
                match state.cc_data_parser.push(&cc_data) {
                    Ok(_) => self.handle_cc_data(&mut state, pts),
                    Err(e) => {
                        gst::warning!(CAT, "Failed to parse incoming data: {e}");
                        gst::element_imp_warning!(
                            self,
                            gst::StreamError::Decode,
                            ["Failed to parse incoming data: {e}"]
                        );
                        state.cc_data_parser.flush();
                    }
                }
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea608S3341a {
                self.decode_s334_1a(&mut state, meta.data(), pts);
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea608Raw {
                let data = meta.data();
                assert!(data.len() % 2 == 0);
                for pair in data.chunks_exact(2) {
                    self.have_cea608(
                        &mut state,
                        cea608_types::tables::Field::ONE,
                        [pair[0], pair[1]],
                        pts,
                    );
                }
            }
        }

        let composition = self.render(&mut state);

        if let Some(timeout) = caption_timeout {
            if let Some(interval) = pts.opt_saturating_sub(state.last_cc_pts) {
                if interval > timeout {
                    gst::info!(CAT, imp = self, "Reached timeout, clearing overlay");
                    state.cea708_renderer.clear_composition();
                    state.last_cc_pts.take();
                }
            }
        }

        if let Some(composition) = &composition {
            let buffer = buffer.make_mut();
            if state.attach {
                gst_video::VideoOverlayCompositionMeta::add(buffer, composition);
            } else {
                let mut frame = gst_video::VideoFrameRef::from_buffer_ref_writable(
                    buffer,
                    state.video_info.as_ref().unwrap(),
                )
                .unwrap();

                if composition.blend(&mut frame).is_err() {
                    gst::error!(CAT, obj = pad, "Failed to blend composition");
                }
            }
        }
        drop(state);

        self.srcpad.push(buffer)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(c) => {
                let mut state = self.state.lock().unwrap();
                state.upstream_caps = Some(c.caps_owned());
                state.video_info = gst_video::VideoInfo::from_caps(c.caps()).ok();
                drop(state);
                self.srcpad.check_reconfigure();
                let _ = self.negotiate();
                true
            }
            EventView::FlushStop(..) => {
                let settings = self.settings.lock().unwrap().clone();
                let mut state = self.state.lock().unwrap();
                state.cea708_renderer = Cea708Renderer::new();
                state
                    .cea708_renderer
                    .set_safe_title_area(settings.safe_width, settings.safe_height);
                if let Some((width, height)) = state
                    .video_info
                    .as_ref()
                    .map(|vinfo| (vinfo.width(), vinfo.height()))
                {
                    state.cea708_renderer.set_video_size(width, height);
                }
                drop(state);

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cea708Overlay {
    const NAME: &'static str = "GstCea708Overlay";
    type Type = super::Cea708Overlay;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cea708Overlay::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |overlay| overlay.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cea708Overlay::catch_panic_pad_function(
                    parent,
                    || false,
                    |overlay| overlay.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::PROXY_CAPS)
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for Cea708Overlay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecInt::builder("cea608-channel")
                    .nick("CEA-608 Channel")
                    .blurb("The cea608 channel (CC1-4) to render the caption for when available, (-1=automatic, 0=disabled)")
                    .minimum(-1)
                    .maximum(4)
                    .default_value(DEFAULT_CEA608_CHANNEL)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecInt::builder("service")
                    .nick("Service")
                    .blurb("The service to render the caption for when available, (-1=automatic, 0=disabled)")
                    .minimum(-1)
                    .maximum(31)
                    .default_value(DEFAULT_SERVICE)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("timeout")
                    .nick("Timeout")
                    .blurb("Duration after which to erase overlay when no cc data has arrived for the selected service/channel")
                    .minimum(16.seconds().nseconds())
                    .default_value(u64::MAX)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("safe-title-height")
                    .nick("Safe Title Height")
                    .blurb("Ratio of the video height to use as the safe area for caption display")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(0.8)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecFloat::builder("safe-title-width")
                    .nick("Safe Title Width")
                    .blurb("Ratio of the video width to use as the safe area for caption display")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(0.8)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "cea608-channel" => {
                let mut settings = self.settings.lock().unwrap();

                let new = value.get().expect("type checked upstream");
                if new != settings.cea608_channel {
                    settings.cea608_channel = new;
                    settings.changed = true;
                }
            }
            "service" => {
                let mut settings = self.settings.lock().unwrap();

                let new = value.get().expect("type checked upstream");
                if new != settings.service {
                    settings.service = new;
                    settings.changed = true;
                }
            }
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();

                let timeout = value.get().expect("type checked upstream");

                settings.timeout = match timeout {
                    u64::MAX => gst::ClockTime::NONE,
                    _ => Some(timeout.nseconds()),
                };
            }
            "safe-title-width" => {
                let mut settings = self.settings.lock().unwrap();

                let new = value.get().expect("type checked upstream");
                if new != settings.safe_width {
                    settings.safe_width = new;
                    settings.changed = true;
                }
            }
            "safe-title-height" => {
                let mut settings = self.settings.lock().unwrap();

                let new = value.get().expect("type checked upstream");
                if new != settings.safe_height {
                    settings.safe_height = new;
                    settings.changed = true;
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "cea608-channel" => settings.cea608_channel.to_value(),
            "service" => settings.service.to_value(),
            "timeout" => {
                if let Some(timeout) = settings.timeout {
                    timeout.nseconds().to_value()
                } else {
                    u64::MAX.to_value()
                }
            }
            "safe-title-width" => settings.safe_width.to_value(),
            "safe-title-height" => settings.safe_height.to_value(),
            _ => unimplemented!(),
        }
    }
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for Cea708Overlay {}

impl ElementImpl for Cea708Overlay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CEA 708 overlay",
                "Video/Overlay/Subtitle",
                "Renders CEA 708 closed caption meta over raw video frames",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoFormat::iter_raw()
                .into_video_caps()
                .unwrap()
                .any_features()
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused | gst::StateChange::PausedToReady => {
                // Reset the whole state
                let mut state = self.state.lock().unwrap();
                *state = State::default();
                drop(state);
                let mut settings = self.settings.lock().unwrap();
                settings.changed = true;
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
