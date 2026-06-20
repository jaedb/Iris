// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
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
use crate::cea608utils::Cea608Renderer;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cea608overlay",
        gst::DebugColorFlags::empty(),
        Some("CEA 608 overlay element"),
    )
});

const DEFAULT_FIELD: i32 = -1;
const DEFAULT_BLACK_BACKGROUND: bool = false;

#[derive(Debug)]
struct Settings {
    field: i32,
    black_background: bool,
    timeout: Option<gst::ClockTime>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            field: DEFAULT_FIELD,
            black_background: DEFAULT_BLACK_BACKGROUND,
            timeout: gst::ClockTime::NONE,
        }
    }
}

struct State {
    video_info: Option<gst_video::VideoInfo>,
    renderer: Cea608Renderer,
    composition: Option<gst_video::VideoOverlayComposition>,
    attach: bool,
    selected_field: Option<u8>,
    last_cc_pts: Option<gst::ClockTime>,
    upstream_caps: Option<gst::Caps>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            video_info: None,
            renderer: Cea608Renderer::new(),
            composition: None,
            attach: false,
            selected_field: None,
            last_cc_pts: gst::ClockTime::NONE,
            upstream_caps: None,
        }
    }
}

pub struct Cea608Overlay {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl Cea608Overlay {
    fn overlay(&self, state: &mut State) {
        if let Some(rect) = state.renderer.generate_rectangle() {
            state.composition = gst_video::VideoOverlayComposition::new(Some(&rect)).ok();
        }
    }

    fn negotiate(&self, state: &mut State) -> Result<gst::FlowSuccess, gst::FlowError> {
        let Some(caps) = state.upstream_caps.as_ref() else {
            gst::element_imp_error!(
                self,
                gst::CoreError::Negotiation,
                ["Element hasn't received valid video caps at negotiation time"]
            );
            self.srcpad.mark_reconfigure();
            return Err(gst::FlowError::NotNegotiated);
        };

        let video_info = match state.video_info.as_ref() {
            Some(video_info) => Ok(video_info),
            None => {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Negotiation,
                    ["Element hasn't received valid video caps at negotiation time"]
                );
                Err(gst::FlowError::NotNegotiated)
            }
        }?;

        let mut downstream_accepts_meta = false;
        let mut caps = caps.clone();

        let upstream_has_meta = caps
            .features(0)
            .map(|f| f.contains(gst_video::CAPS_FEATURE_META_GST_VIDEO_OVERLAY_COMPOSITION))
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
                    return Err(gst::FlowError::NotNegotiated);
                }
            }
        }

        state.attach = upstream_has_meta || downstream_accepts_meta;

        state
            .renderer
            .set_video_size(video_info.width(), video_info.height());
        state
            .renderer
            .set_channel(cea608_types::tables::Channel::ONE);

        if !self.srcpad.push_event(gst::event::Caps::new(&caps)) {
            Err(gst::FlowError::NotNegotiated)
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn decode_cc_data(&self, state: &mut State, data: &[u8], pts: gst::ClockTime) {
        if data.len() % 3 != 0 {
            gst::warning!(CAT, "cc_data length is not a multiple of 3, truncating");
        }

        for triple in data.chunks_exact(3) {
            let cc_valid = (triple[0] & 0x04) == 0x04;
            let cc_type = triple[0] & 0x03;

            if !cc_valid {
                continue;
            }
            if cc_type == 0x00 || cc_type == 0x01 {
                if state.selected_field.is_none() {
                    state.selected_field = Some(cc_type);
                    gst::info!(CAT, imp = self, "Selected field {} automatically", cc_type);
                }

                if Some(cc_type) != state.selected_field {
                    continue;
                };
                match state.renderer.push_pair([triple[1], triple[2]]) {
                    Err(e) => {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Failed to parse incoming CEA-608 ({:x?}): {e:?}",
                            [triple[1], triple[2]]
                        );
                        continue;
                    }
                    Ok(true) => {
                        state.composition.take();
                    }
                    Ok(false) => continue,
                };

                self.overlay(state);

                self.reset_timeout(state, pts);
            } else {
                break;
            }
        }
    }

    fn decode_s334_1a(&self, state: &mut State, data: &[u8], pts: gst::ClockTime) {
        if data.len() % 3 != 0 {
            gst::warning!(CAT, "cc_data length is not a multiple of 3, truncating");
        }

        for triple in data.chunks_exact(3) {
            let field = if (triple[0] & 0x80) == 0x80 { 0 } else { 1 };

            if state.selected_field.is_none() {
                state.selected_field = Some(field);
                gst::info!(CAT, imp = self, "Selected field {field} automatically");
            }

            if Some(field) != state.selected_field {
                continue;
            };

            match state.renderer.push_pair([triple[1], triple[2]]) {
                Err(e) => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to parse incoming CEA-608 ({:x?}): {e:?}",
                        [triple[1], triple[2]]
                    );
                    continue;
                }
                Ok(true) => {
                    state.composition.take();
                }
                Ok(false) => continue,
            };
            self.overlay(state);

            self.reset_timeout(state, pts);
        }
    }

    fn reset_timeout(&self, state: &mut State, pts: gst::ClockTime) {
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

        let mut state = self.state.lock().unwrap();

        if self.srcpad.check_reconfigure() {
            self.negotiate(&mut state)?;
        }

        for meta in buffer.iter_meta::<gst_video::VideoCaptionMeta>() {
            if meta.caption_type() == gst_video::VideoCaptionType::Cea708Cdp {
                match extract_cdp(meta.data()) {
                    Ok(data) => {
                        self.decode_cc_data(&mut state, data, pts);
                    }
                    Err(e) => {
                        gst::warning!(CAT, "{e}");
                        gst::element_imp_warning!(self, gst::StreamError::Decode, ["{e}"]);
                    }
                }
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea708Raw {
                self.decode_cc_data(&mut state, meta.data(), pts);
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea608S3341a {
                self.decode_s334_1a(&mut state, meta.data(), pts);
            } else if meta.caption_type() == gst_video::VideoCaptionType::Cea608Raw {
                let data = meta.data();
                assert!(data.len() % 2 == 0);
                for pair in data.chunks_exact(2) {
                    match state.renderer.push_pair([pair[0], pair[1]]) {
                        Err(e) => {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Failed to parse incoming CEA-608 ({:x?}): {e:?}",
                                [pair[0], pair[1]]
                            );
                            continue;
                        }
                        Ok(true) => {
                            state.composition.take();
                        }
                        Ok(false) => continue,
                    };

                    self.overlay(&mut state);

                    self.reset_timeout(&mut state, pts);
                }
            }
        }

        if let Some(timeout) = self.settings.lock().unwrap().timeout {
            if let Some(interval) = pts.opt_saturating_sub(state.last_cc_pts) {
                if interval > timeout {
                    gst::info!(CAT, imp = self, "Reached timeout, clearing overlay");
                    state.composition.take();
                    state.last_cc_pts.take();
                }
            }
        }

        if let Some(composition) = &state.composition {
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
                self.srcpad.check_reconfigure();
                match self.negotiate(&mut state) {
                    Ok(_) => true,
                    Err(_) => {
                        self.srcpad.mark_reconfigure();
                        true
                    }
                }
            }
            EventView::FlushStop(..) => {
                let settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();
                state.renderer = Cea608Renderer::new();
                state
                    .renderer
                    .set_black_background(settings.black_background);
                state.composition = None;
                if let Some((width, height)) = state
                    .video_info
                    .as_ref()
                    .map(|vinfo| (vinfo.width(), vinfo.height()))
                {
                    state.renderer.set_video_size(width, height);
                }
                drop(state);
                drop(settings);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cea608Overlay {
    const NAME: &'static str = "GstCea608Overlay";
    type Type = super::Cea608Overlay;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cea608Overlay::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |overlay| overlay.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cea608Overlay::catch_panic_pad_function(
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

impl ObjectImpl for Cea608Overlay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecInt::builder("field")
                    .nick("Field")
                    .blurb("The field to render the caption for when available, (-1=automatic)")
                    .minimum(-1)
                    .maximum(1)
                    .default_value(DEFAULT_FIELD)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("black-background")
                    .nick("Black background")
                    .blurb("Whether a black background should be drawn behind text")
                    .default_value(DEFAULT_BLACK_BACKGROUND)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("timeout")
                    .nick("Timeout")
                    .blurb("Duration after which to erase overlay when no cc data has arrived for the selected field")
                    .minimum(16.seconds().nseconds())
                    .default_value(u64::MAX)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "field" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();

                settings.field = value.get().expect("type checked upstream");

                let old_field = state.selected_field;
                state.selected_field = match settings.field {
                    -1 => None,
                    val => Some(val as u8),
                };

                if state.selected_field != old_field {
                    state.renderer.clear();
                    state.composition = None;
                }
            }
            "black-background" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();

                settings.black_background = value.get().expect("type checked upstream");
                state
                    .renderer
                    .set_black_background(settings.black_background);
                state.composition.take();
            }
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();

                let timeout = value.get().expect("type checked upstream");

                settings.timeout = match timeout {
                    u64::MAX => gst::ClockTime::NONE,
                    _ => Some(timeout.nseconds()),
                };
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "field" => {
                let settings = self.settings.lock().unwrap();
                settings.field.to_value()
            }
            "black-background" => {
                let settings = self.settings.lock().unwrap();
                settings.black_background.to_value()
            }
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                if let Some(timeout) = settings.timeout {
                    timeout.nseconds().to_value()
                } else {
                    u64::MAX.to_value()
                }
            }
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

impl GstObjectImpl for Cea608Overlay {}

impl ElementImpl for Cea608Overlay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Cea 608 overlay",
                "Video/Overlay/Subtitle",
                "Renders CEA 608 closed caption meta over raw video frames",
                "Mathieu Duponchelle <mathieu@centricular.com>",
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
                let settings = self.settings.lock().unwrap();
                state.selected_field = match settings.field {
                    -1 => None,
                    val => Some(val as u8),
                };
                state
                    .renderer
                    .set_black_background(settings.black_background);
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
