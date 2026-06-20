// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::anyhow;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use anyhow::Error;
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "translationbin",
        gst::DebugColorFlags::empty(),
        Some("Transcribes and translates text"),
    )
});

const DEFAULT_TRANSCRIBE_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(1);
const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::ZERO;
const DEFAULT_TRANSLATE_LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(500);
const DEFAULT_INPUT_LANG_CODE: &str = "en-US";
const DEFAULT_OUTPUT_LANG_CODE: &str = "fr-FR";

struct State {
    transcriber: Option<gst::Element>,
    tee: Option<gst::Element>,
    queue: Option<gst::Element>,
    srcpads: HashSet<super::TranslationSrcPad>,
    pad_serial: u32,
}

#[derive(Clone)]
struct Settings {
    language_code: String,
    transcribe_latency: gst::ClockTime,
    lateness: gst::ClockTime,
    translate_latency: gst::ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language_code: DEFAULT_INPUT_LANG_CODE.to_string(),
            transcribe_latency: DEFAULT_TRANSCRIBE_LATENCY,
            lateness: DEFAULT_LATENESS,
            translate_latency: DEFAULT_TRANSLATE_LATENCY,
        }
    }
}

pub struct TranslationBin {
    state: Mutex<State>,
    settings: Mutex<Settings>,
    audio_sinkpad: gst::GhostPad,
    transcript_srcpad: gst::GhostPad,
}

impl TranslationBin {
    fn prepare_translation_srcpad(
        &self,
        elem: &super::TranslationBin,
        srcpad: &super::TranslationSrcPad,
        input_language_code: &str,
        translate_latency_ms: u32,
        tee: &gst::Element,
    ) -> Result<(), Error> {
        let output_language_code = srcpad.imp().settings.lock().unwrap().language_code.clone();

        let queue = gst::ElementFactory::make("queue").build()?;
        let translator = gst::ElementFactory::make("awstranslate")
            .property("input-language-code", input_language_code)
            .property("output-language-code", output_language_code)
            .build()?;

        if translator.has_property_with_type("latency", u32::static_type()) {
            translator.set_property("latency", translate_latency_ms);
        }

        elem.add_many([&queue, &translator])?;
        queue.sync_state_with_parent()?;
        translator.sync_state_with_parent()?;

        gst::Element::link_many([tee, &queue, &translator])?;

        srcpad.set_target(Some(
            &translator
                .static_pad("src")
                .ok_or(anyhow!("No pad named src on translator"))?,
        ))?;

        let mut pad_state = srcpad.imp().state.lock().unwrap();

        pad_state.queue = Some(queue);
        pad_state.translator = Some(translator);

        drop(pad_state);

        srcpad.notify("translator");

        Ok(())
    }

    fn unprepare_translation_srcpad(
        &self,
        elem: &super::TranslationBin,
        tee: &gst::Element,
        srcpad: &super::TranslationSrcPad,
    ) -> Result<(), Error> {
        let (queue, translator) = {
            let mut pad_state = srcpad.imp().state.lock().unwrap();

            (
                pad_state.queue.take().unwrap(),
                pad_state.translator.take().unwrap(),
            )
        };

        tee.unlink(&queue);

        elem.remove_many([&queue, &translator])?;

        srcpad.set_target(None::<&gst::Pad>)?;

        let _ = queue.set_state(gst::State::Null);
        let _ = translator.set_state(gst::State::Null);

        Ok(())
    }

    fn prepare(&self) -> Result<(), Error> {
        let (transcriber, srcpads) = {
            let state = self.state.lock().unwrap();

            let transcriber = match state.transcriber {
                Some(ref transcriber) => transcriber.clone(),
                None => gst::ElementFactory::make("awstranscriber2").build()?,
            };

            (transcriber, state.srcpads.clone())
        };

        let Settings {
            transcribe_latency,
            lateness,
            translate_latency,
            language_code,
        } = self.settings.lock().unwrap().clone();

        let transcribe_latency_ms = transcribe_latency.mseconds() as u32;
        let lateness_ms = lateness.mseconds() as u32;
        let translate_latency_ms = translate_latency.mseconds() as u32;

        if transcriber.has_property_with_type("transcribe-latency", u32::static_type()) {
            transcriber.set_property("transcribe-latency", transcribe_latency_ms);
        } else if transcriber.has_property_with_type("latency", u32::static_type()) {
            transcriber.set_property("latency", transcribe_latency_ms);
        }

        if transcriber.has_property_with_type("lateness", u32::static_type()) {
            transcriber.set_property("lateness", lateness_ms);
        }

        transcriber.set_property("language-code", &language_code);

        let accumulate = gst::ElementFactory::make("textaccumulate").build()?;

        let tee = gst::ElementFactory::make("tee")
            .property("allow-not-linked", true)
            .build()?;
        let queue = gst::ElementFactory::make("queue").build()?;

        let obj = self.obj();

        obj.add_many([&transcriber, &accumulate, &tee, &queue])?;

        transcriber.sync_state_with_parent()?;
        accumulate.sync_state_with_parent()?;
        tee.sync_state_with_parent()?;

        gst::Element::link_many([&transcriber, &accumulate, &tee, &queue])?;

        self.audio_sinkpad.set_target(Some(
            &transcriber
                .static_pad("sink")
                .ok_or(anyhow!("No pad named sink on transcriber"))?,
        ))?;

        self.transcript_srcpad
            .set_target(Some(&queue.static_pad("src").unwrap()))?;

        for srcpad in srcpads {
            self.prepare_translation_srcpad(
                &obj,
                &srcpad,
                &language_code,
                translate_latency_ms,
                &tee,
            )?;
        }

        let mut state = self.state.lock().unwrap();

        state.transcriber = Some(transcriber);
        state.tee = Some(tee);
        state.queue = Some(queue);

        Ok(())
    }

    fn unprepare(&self) -> Result<(), Error> {
        let (transcriber, tee, queue) = {
            let mut state = self.state.lock().unwrap();

            (
                state.transcriber.as_ref().unwrap().clone(),
                state.tee.take().unwrap(),
                state.queue.take().unwrap(),
            )
        };
        let obj = self.obj();

        let srcpads = self.state.lock().unwrap().srcpads.clone();

        for srcpad in srcpads {
            self.unprepare_translation_srcpad(&obj, &tee, &srcpad)?;
        }

        transcriber.unlink(&tee);

        obj.remove_many([&transcriber, &tee, &queue])?;

        self.audio_sinkpad.set_target(None::<&gst::Pad>)?;
        self.transcript_srcpad.set_target(None::<&gst::Pad>)?;

        let _ = transcriber.set_state(gst::State::Null);
        let _ = tee.set_state(gst::State::Null);
        let _ = queue.set_state(gst::State::Null);

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TranslationBin {
    const NAME: &'static str = "GstTranslationBin";
    type Type = super::TranslationBin;
    type ParentType = gst::Bin;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let audio_sinkpad = gst::PadBuilder::<gst::GhostPad>::from_template(&templ).build();

        let templ = klass.pad_template("src").unwrap();
        let transcript_srcpad = gst::PadBuilder::<gst::GhostPad>::from_template(&templ).build();

        Self {
            state: Mutex::new(State {
                transcriber: None,
                tee: None,
                queue: None,
                srcpads: HashSet::new(),
                pad_serial: 0,
            }),
            settings: Mutex::new(Settings::default()),
            audio_sinkpad,
            transcript_srcpad,
        }
    }
}

impl ObjectImpl for TranslationBin {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("transcribe-latency")
                    .nick("Transcription Latency")
                    .blurb("Amount of milliseconds to allow for transcription")
                    .default_value(DEFAULT_TRANSCRIBE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Transcription Lateness")
                    .blurb("Amount of milliseconds to offset transcription by")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                glib::ParamSpecUInt::builder("translate-latency")
                    .nick("Translation Latency")
                    .blurb("Amount of milliseconds to allow for translation")
                    .default_value(DEFAULT_TRANSLATE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("The language of the input stream")
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecObject::builder::<gst::Element>("transcriber")
                    .nick("Transcriber")
                    .blurb("The transcriber element to use")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "transcribe-latency" => {
                let mut settings = self.settings.lock().unwrap();
                settings.transcribe_latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "translate-latency" => {
                let mut settings = self.settings.lock().unwrap();
                settings.translate_latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "language-code" => {
                let language_code: String = value.get().expect("type checked upstream");
                let mut settings = self.settings.lock().unwrap();
                settings.language_code = language_code;
            }
            "transcriber" => {
                self.state.lock().unwrap().transcriber =
                    value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "transcribe-latency" => {
                let settings = self.settings.lock().unwrap();
                (settings.transcribe_latency.mseconds() as u32).to_value()
            }
            "lateness" => {
                let settings = self.settings.lock().unwrap();
                (settings.lateness.mseconds() as u32).to_value()
            }
            "translate-latency" => {
                let settings = self.settings.lock().unwrap();
                (settings.translate_latency.mseconds() as u32).to_value()
            }
            "language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.language_code.to_value()
            }
            "transcriber" => self.state.lock().unwrap().transcriber.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.audio_sinkpad).unwrap();
        obj.add_pad(&self.transcript_srcpad).unwrap();
    }
}

impl GstObjectImpl for TranslationBin {}

impl ElementImpl for TranslationBin {
    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let serial = {
            let mut state = self.state.lock().unwrap();
            let serial = state.pad_serial;
            state.pad_serial += 1;
            serial
        };

        let pad = gst::PadBuilder::<super::TranslationSrcPad>::from_template(templ)
            .name(format!("translate_src_{serial}"))
            .build();

        self.obj().add_pad(pad.upcast_ref::<gst::Pad>()).unwrap();

        self.state.lock().unwrap().srcpads.insert(pad.clone());

        if let Some(tee) = self.state.lock().unwrap().tee.clone() {
            let Settings {
                translate_latency,
                language_code,
                ..
            } = self.settings.lock().unwrap().clone();

            let translate_latency_ms = translate_latency.mseconds() as u32;

            if let Err(err) = self.prepare_translation_srcpad(
                &self.obj(),
                &pad,
                &language_code,
                translate_latency_ms,
                &tee,
            ) {
                gst::error!(CAT, "Failed to prepare translation source pad: {err:?}");
                return None;
            }
        }

        Some(pad.upcast())
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let srcpad = self.state.lock().unwrap().srcpads.take(pad);

        if let Some(srcpad) = srcpad {
            if let Some(tee) = self.state.lock().unwrap().tee.clone() {
                if let Err(err) = self.unprepare_translation_srcpad(&self.obj(), &tee, &srcpad) {
                    gst::warning!(CAT, "Failed to unprepare translation source pad: {err:?}");
                }
            }
        }

        let _ = self.obj().remove_pad(pad);
    }

    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "TranslationBin",
                "Audio / Text",
                "Transcribes audio and translates it",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("audio/x-raw").build();
            let audio_sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
            let transcript_src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let translate_src_pad_template = gst::PadTemplate::with_gtype(
                "translate_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Request,
                &caps,
                super::TranslationSrcPad::static_type(),
            )
            .unwrap();

            vec![
                audio_sink_pad_template,
                transcript_src_pad_template,
                translate_src_pad_template,
            ]
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
                self.prepare().map_err(|err| {
                    gst::error!(CAT, "Failed to prepare: {:?}", err);
                    gst::StateChangeError
                })?;
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition);

        match transition {
            gst::StateChange::PausedToReady => {
                self.unprepare().map_err(|err| {
                    gst::error!(CAT, "Failed to unprepare: {:?}", err);
                    gst::StateChangeError
                })?;
            }
            _ => (),
        }

        ret
    }
}

impl BinImpl for TranslationBin {}

#[derive(Debug)]
struct TranslationSrcPadState {
    queue: Option<gst::Element>,
    translator: Option<gst::Element>,
}

#[derive(Debug)]
struct TranslationSrcPadSettings {
    language_code: String,
}

#[derive(Debug)]
pub struct TranslationSrcPad {
    state: Mutex<TranslationSrcPadState>,
    settings: Mutex<TranslationSrcPadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for TranslationSrcPad {
    const NAME: &'static str = "GstTranslationBinTranslationSrcPad";
    type Type = super::TranslationSrcPad;
    type ParentType = gst::GhostPad;

    fn new() -> Self {
        Self {
            state: Mutex::new(TranslationSrcPadState {
                queue: None,
                translator: None,
            }),
            settings: Mutex::new(TranslationSrcPadSettings {
                language_code: DEFAULT_OUTPUT_LANG_CODE.to_string(),
            }),
        }
    }
}

impl ObjectImpl for TranslationSrcPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("The language of the output stream")
                    .default_value(Some(DEFAULT_OUTPUT_LANG_CODE))
                    .build(),
                glib::ParamSpecObject::builder::<gst::Element>("translator")
                    .nick("Translator")
                    .blurb("The translator element in use")
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "language-code" => {
                let language_code: String = value.get().expect("type checked upstream");
                let mut settings = self.settings.lock().unwrap();
                settings.language_code = language_code.clone();

                if let Some(translator) = self.state.lock().unwrap().translator.as_ref() {
                    translator.set_property("output-language-code", language_code);
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.language_code.to_value()
            }
            "translator" => self.state.lock().unwrap().translator.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TranslationSrcPad {}

impl PadImpl for TranslationSrcPad {}

impl ProxyPadImpl for TranslationSrcPad {}

impl GhostPadImpl for TranslationSrcPad {}
