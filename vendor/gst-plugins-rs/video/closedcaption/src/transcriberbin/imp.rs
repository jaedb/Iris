// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::cea608utils::Cea608Mode;
use crate::cea708utils::Cea708Mode;
use anyhow::{anyhow, Error};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use std::sync::LazyLock;
use std::sync::MutexGuard;

use super::{CaptionSource, MuxMethod};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "transcriberbin",
        gst::DebugColorFlags::empty(),
        Some("Transcribe and inject closed captions"),
    )
});

const DEFAULT_PASSTHROUGH: bool = false;
const DEFAULT_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(4);
const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::from_seconds(0);
const DEFAULT_TRANSLATE_LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(500);
const DEFAULT_ACCUMULATE: gst::ClockTime = gst::ClockTime::ZERO;
const DEFAULT_MODE: Cea608Mode = Cea608Mode::RollUp2;
const DEFAULT_CAPTION_SOURCE: CaptionSource = CaptionSource::Both;
const DEFAULT_INPUT_LANG_CODE: &str = "en-US";
const DEFAULT_MUX_METHOD: MuxMethod = MuxMethod::Cea608;

const CEAX08MUX_LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(100);
const CCCOMBINER_LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(100);

#[derive(Debug)]
enum TargetPassthroughState {
    None,
    Enabled,
    Disabled,
}

#[derive(Debug)]
enum PassthroughState {
    Enabled,
    Disabled,
}

#[derive(Debug)]
enum CaptionChannelUpdate {
    Upsert(HashSet<String>),
    Remove,
}

#[derive(Debug)]
enum CustomChannelUpdate {
    Upsert(String),
    Remove,
}

fn parse_language_pair(
    mux_method: MuxMethod,
    key: &str,
    value: &gst::glib::Value,
) -> Result<(String, HashSet<String>), Error> {
    let lowercased_key = key.to_lowercase();
    Ok(match mux_method {
        MuxMethod::Cea608 => {
            if ["cc1", "cc3"].contains(&lowercased_key.as_str()) {
                (
                    value.get::<String>()?,
                    HashSet::from([lowercased_key.to_string()]),
                )
            } else if let Ok(caption_stream) = value.get::<String>() {
                if !["cc1", "cc3"].contains(&caption_stream.as_str()) {
                    anyhow::bail!(
                        "Unknown 608 channel {}, valid values are cc1, cc3",
                        caption_stream
                    );
                }
                (key.to_string(), HashSet::from([caption_stream]))
            } else {
                anyhow::bail!("Unknown 608 channel/language {}", key);
            }
        }
        MuxMethod::Cea708 => {
            if let Ok(caption_stream) = value.get::<String>() {
                (key.to_string(), HashSet::from([caption_stream]))
            } else if let Ok(caption_streams) = value.get::<gst::List>() {
                let mut streams = HashSet::new();
                for s in caption_streams.iter() {
                    let service = s.get::<String>()?;
                    if ["cc1", "cc3"].contains(&service.as_str()) || service.starts_with("708_") {
                        streams.insert(service);
                    } else {
                        anyhow::bail!(
                            "Unknown 708 service {}, valid values are cc1, cc3 or 708_*",
                            key
                        );
                    }
                }
                (key.to_string(), streams)
            } else {
                anyhow::bail!("Unknown 708 translation language field {}", key);
            }
        }
    })
}

/* One per language, including original */
#[derive(Clone)]
struct CaptionChannel {
    bin: gst::Bin,
    textwrap: gst::Element,
    tttoceax08: gst::Element,
    language: String,
    ccmux_pad_name: String,
    cccapsfilter: gst::Element,
    caption_streams: HashSet<String>,
}

#[derive(Clone, Copy)]
enum CustomOutputChannelType {
    Synthesis,
    Subtitle,
}

impl CustomOutputChannelType {
    fn name(&self) -> &str {
        match self {
            Self::Synthesis => "synthesis",
            Self::Subtitle => "subtitle",
        }
    }
}

#[derive(Clone)]
struct CustomOutputChannel {
    bin_description: String,
    bin: gst::Bin,
    language: String,
    latency: gst::ClockTime,
    type_: CustomOutputChannelType,
}

/* Locking order: State, Settings, PadState, PadSettings */

struct State {
    mux_method: MuxMethod,
    framerate: Option<gst::Fraction>,
    internal_bin: gst::Bin,
    video_queue: gst::Element,
    ccmux: gst::Element,
    ccmux_filter: gst::Element,
    cccombiner: gst::Element,
    transcription_bin: gst::Bin,
    cccapsfilter: gst::Element,
    transcription_valve: gst::Element,
    audio_serial: u32,
    audio_sink_pads: HashMap<String, super::TranscriberSinkPad>,
}

struct Settings {
    cc_caps: gst::Caps,
    latency: gst::ClockTime,
    lateness: gst::ClockTime,
    translate_latency: gst::ClockTime,
    accumulate_time: gst::ClockTime,
    caption_source: CaptionSource,
    mux_method: MuxMethod,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cc_caps: gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .build(),
            latency: DEFAULT_LATENCY,
            lateness: DEFAULT_LATENESS,
            translate_latency: DEFAULT_TRANSLATE_LATENCY,
            accumulate_time: DEFAULT_ACCUMULATE,
            caption_source: DEFAULT_CAPTION_SOURCE,
            mux_method: DEFAULT_MUX_METHOD,
        }
    }
}

// Struct containing all the element data
pub struct TranscriberBin {
    audio_srcpad: gst::GhostPad,
    video_srcpad: gst::GhostPad,
    audio_sinkpad: gst::GhostPad,
    video_sinkpad: gst::GhostPad,

    state: Mutex<Option<State>>,
    settings: Mutex<Settings>,
}

fn query_latency(element: &gst::Element) -> Result<gst::ClockTime, Error> {
    let fakesrc = gst::ElementFactory::make("fakesrc")
        .property("is-live", true)
        .build()?;

    let Some(srcpad) = element.src_pads().first().cloned() else {
        return Err(anyhow!(
            "querying latency on element with no source pad unsupported"
        ));
    };

    fakesrc.link(element)?;

    let mut q = gst::query::Latency::new();

    let handled = srcpad.query(&mut q);

    fakesrc.unlink(element);

    if !handled {
        return Err(anyhow!("impossible to query latency"));
    }

    let (_live, min, _max) = q.result();

    Ok(min)
}

impl TranscriberBin {
    fn configure_transcriber(&self, transcriber: &gst::Element) {
        let settings = self.settings.lock().unwrap();
        let latency_ms = settings.latency.mseconds() as u32;

        if transcriber.has_property_with_type("transcribe-latency", u32::static_type()) {
            transcriber.set_property("transcribe-latency", latency_ms);
        } else if transcriber.has_property_with_type("latency", u32::static_type()) {
            transcriber.set_property("latency", latency_ms);
        }

        if transcriber.has_property_with_type("translate-latency", u32::static_type()) {
            let translate_latency_ms = settings.translate_latency.mseconds() as u32;
            transcriber.set_property("translate-latency", translate_latency_ms);
        }
        if transcriber.has_property_with_type("lateness", u32::static_type()) {
            let lateness_ms = settings.lateness.mseconds() as u32;
            transcriber.set_property("lateness", lateness_ms);
        }
    }

    fn link_transcriber_to_channel(
        &self,
        topbin: &super::TranscriberBin,
        state: &State,
        pad_state: &TranscriberSinkPadState,
        language: &str,
        channel_bin: &gst::Bin,
    ) -> Result<(), Error> {
        let Some(ref transcriber) = pad_state.transcriber else {
            return Err(anyhow!("no transcriber to link"));
        };

        let language_tee = pad_state
            .language_tees
            .get(language)
            .expect("language tee created");

        let sinkpad = if let Some(filter) = pad_state.language_filters.get(language) {
            filter.static_pad("sink").unwrap()
        } else {
            language_tee.static_pad("sink").unwrap()
        };

        if !sinkpad.is_linked() {
            let transcriber_src_pad = match language {
                "transcript" => transcriber
                    .static_pad("src")
                    .ok_or(anyhow!("Failed to retrieve transcription source pad"))?,
                language => {
                    let pad = transcriber
                        .request_pad_simple("translate_src_%u")
                        .ok_or(anyhow!("Failed to request translation source pad"))?;
                    pad.set_property("language-code", language);
                    pad
                }
            };

            gst::debug!(
                CAT,
                obj = transcriber_src_pad,
                "Linking transcriber source pad to language tee"
            );

            transcriber_src_pad.link(&sinkpad)?;

            pad_state.expose_unsynced_pads(topbin, state, transcriber_src_pad.name().as_str())?;
        }

        let tee_srcpad = language_tee.request_pad_simple("src_%u").unwrap();

        gst::debug!(CAT, obj = tee_srcpad, "Linking language tee to channel");

        tee_srcpad.link(&channel_bin.static_pad("sink").unwrap())?;

        Ok(())
    }

    fn link_transcriber_to_channels(
        &self,
        state: &State,
        pad_state: &TranscriberSinkPadState,
    ) -> Result<(), Error> {
        for (language, bin) in itertools::chain!(
            pad_state
                .caption_channels
                .values()
                .map(|c| (c.language.as_str(), c.bin.clone())),
            pad_state
                .synthesis_channels
                .values()
                .map(|c| (c.language.as_str(), c.bin.clone())),
            pad_state
                .subtitle_channels
                .values()
                .map(|c| (c.language.as_str(), c.bin.clone())),
        ) {
            self.link_transcriber_to_channel(&self.obj(), state, pad_state, language, &bin)?;
        }

        Ok(())
    }

    fn expose_channel_outputs(
        &self,
        state: &State,
        pad_state: &TranscriberSinkPadState,
        pad_settings: &TranscriberSinkPadSettings,
    ) -> Result<(), Error> {
        for channel in pad_state.caption_channels.values() {
            pad_state.link_transcription_channel(channel, state, pad_settings.passthrough)?;
        }

        for channel in itertools::chain!(
            pad_state.synthesis_channels.values(),
            pad_state.subtitle_channels.values()
        ) {
            self.expose_custom_output_pads(
                &pad_state.transcription_bin,
                &state.transcription_bin,
                &state.internal_bin,
                pad_state.serial,
                channel,
            )?;
        }

        Ok(())
    }

    fn construct_custom_output_channel(
        &self,
        lang: &str,
        bin_description: &str,
        type_: CustomOutputChannelType,
    ) -> Result<CustomOutputChannel, Error> {
        let bin = gst::Bin::new();
        let queue = gst::ElementFactory::make("queue").build()?;
        let synthesizer = gst::parse::bin_from_description_full(
            bin_description,
            true,
            None,
            gst::ParseFlags::NO_SINGLE_ELEMENT_BINS,
        )?;

        let latency = query_latency(&synthesizer)?;

        bin.add_many([&queue, &synthesizer])?;
        gst::Element::link_many([&queue, &synthesizer])?;

        queue.set_property("max-size-buffers", 0u32);
        queue.set_property("max-size-time", 0u64);

        let sinkpad = gst::GhostPad::with_target(&queue.static_pad("sink").unwrap()).unwrap();
        bin.add_pad(&sinkpad)?;

        let srcpad = gst::GhostPad::with_target(&synthesizer.static_pad("src").unwrap()).unwrap();
        bin.add_pad(&srcpad)?;

        Ok(CustomOutputChannel {
            bin_description: bin_description.to_string(),
            bin,
            language: String::from(lang),
            latency,
            type_,
        })
    }

    fn construct_transcription_channel(
        &self,
        lang: &str,
        mux_method: MuxMethod,
        caption_streams: HashSet<String>,
    ) -> Result<CaptionChannel, Error> {
        let bin = gst::Bin::new();
        let queue = gst::ElementFactory::make("queue").build()?;
        let textwrap = gst::ElementFactory::make("textwrap").build()?;
        let (tttoceax08, ccmux_pad_name) = match mux_method {
            MuxMethod::Cea608 => {
                if caption_streams.len() != 1 {
                    anyhow::bail!("Muxing zero/multiple cea608 streams for the same language is not supported");
                }
                (
                    gst::ElementFactory::make("tttocea608").build()?,
                    caption_streams.iter().next().unwrap().clone(),
                )
            }
            MuxMethod::Cea708 => {
                if !(1..=2).contains(&caption_streams.len()) {
                    anyhow::bail!(
                        "Incorrect number of caption stream names {} for muxing 608/708",
                        caption_streams.len()
                    );
                }
                let mut service_no = None;
                let mut cea608_channel = None;
                for cc in caption_streams.iter() {
                    if let Some(cea608) = cc.to_lowercase().strip_prefix("cc") {
                        if cea608_channel.is_some() {
                            anyhow::bail!(
                                "Multiple CEA-608 streams for a language are not supported"
                            );
                        }
                        let channel = cea608.parse::<u32>()?;
                        if (1..=4).contains(&channel) {
                            cea608_channel = Some(channel);
                        } else {
                            anyhow::bail!(
                                "CEA-608 channels only support values between 1 and 4 inclusive"
                            );
                        }
                    } else if let Some(cea708_service) = cc.strip_prefix("708_") {
                        if service_no.is_some() {
                            anyhow::bail!(
                                "Multiple CEA-708 streams for a language are not supported"
                            );
                        }
                        service_no = Some(cea708_service.parse::<u32>()?);
                    } else {
                        anyhow::bail!(
                            "caption service name does not match \'708_%u\', or cc1, or cc3"
                        );
                    }
                }
                let service_no = service_no.ok_or(anyhow!("No 708 caption service provided"))?;
                let mut builder =
                    gst::ElementFactory::make("tttocea708").property("service-number", service_no);
                if let Some(channel) = cea608_channel {
                    builder = builder.property("cea608-channel", channel);
                }
                (builder.build()?, format!("sink_{service_no}"))
            }
        };
        let cccapsfilter = gst::ElementFactory::make("capsfilter").build()?;
        let converter = gst::ElementFactory::make("ccconverter").build()?;

        bin.add_many([&queue, &textwrap, &tttoceax08, &cccapsfilter, &converter])?;
        gst::Element::link_many([&queue, &textwrap, &tttoceax08, &cccapsfilter, &converter])?;

        queue.set_property("max-size-buffers", 0u32);
        queue.set_property("max-size-time", 0u64);

        textwrap.set_property("lines", 2u32);

        let caps = match mux_method {
            MuxMethod::Cea608 => gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .field("framerate", gst::Fraction::new(30, 1))
                .build(),
            MuxMethod::Cea708 => gst::Caps::builder("closedcaption/x-cea-708")
                .field("format", "cc_data")
                .field("framerate", gst::Fraction::new(30, 1))
                .build(),
        };

        cccapsfilter.set_property("caps", caps);

        let sinkpad = gst::GhostPad::with_target(&queue.static_pad("sink").unwrap()).unwrap();
        let srcpad = gst::GhostPad::with_target(&converter.static_pad("src").unwrap()).unwrap();
        bin.add_pad(&sinkpad)?;
        bin.add_pad(&srcpad)?;

        Ok(CaptionChannel {
            bin,
            textwrap,
            tttoceax08,
            language: String::from(lang),
            ccmux_pad_name,
            cccapsfilter,
            caption_streams,
        })
    }

    fn link_input_audio_stream(
        &self,
        pad_name: &str,
        pad_state: &TranscriberSinkPadState,
        state: &mut State,
        pad_settings: &TranscriberSinkPadSettings,
    ) -> Result<(), Error> {
        gst::debug!(CAT, imp = self, "Linking input audio stream {pad_name}");

        pad_state
            .transcription_bin
            .set_property("name", format!("transcription-bin-{pad_name}"));

        state
            .internal_bin
            .add_many([
                &pad_state.clocksync,
                &pad_state.identity,
                &pad_state.audio_tee,
                &pad_state.queue_passthrough,
            ])
            .unwrap();
        gst::Element::link_many([
            &pad_state.clocksync,
            &pad_state.identity,
            &pad_state.audio_tee,
        ])?;
        pad_state.audio_tee.link_pads(
            Some("src_%u"),
            &pad_state.queue_passthrough,
            Some("sink"),
        )?;
        state.transcription_bin.add(&pad_state.transcription_bin)?;
        let aqueue_transcription = gst::ElementFactory::make("queue")
            .name("transqueue")
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .property("max-size-time", 5_000_000_000u64)
            .property_from_str("leaky", "downstream")
            .build()?;

        pad_state.transcription_bin.add_many([
            &aqueue_transcription,
            &pad_state.transcriber_resample,
            &pad_state.transcriber_aconv,
        ])?;

        if let Some(ref transcriber) = pad_state.transcriber {
            pad_state.transcription_bin.add(transcriber)?;
        }

        gst::Element::link_many([
            &aqueue_transcription,
            &pad_state.transcriber_resample,
            &pad_state.transcriber_aconv,
        ])?;

        if let Some(ref transcriber) = pad_state.transcriber {
            pad_state.transcriber_aconv.link(transcriber)?;
        }

        let transcription_audio_sinkpad =
            gst::GhostPad::builder_with_target(&aqueue_transcription.static_pad("sink").unwrap())
                .unwrap()
                .name(pad_name)
                .build();

        pad_state
            .transcription_bin
            .add_pad(&transcription_audio_sinkpad)?;

        let transcription_audio_sinkpad =
            gst::GhostPad::with_target(&transcription_audio_sinkpad).unwrap();

        state
            .transcription_bin
            .add_pad(&transcription_audio_sinkpad)?;

        self.link_transcriber_to_channels(state, pad_state)?;

        self.expose_channel_outputs(state, pad_state, pad_settings)?;

        Ok(())
    }

    fn construct_transcription_bin(&self, state: &mut State) -> Result<(), Error> {
        gst::debug!(CAT, imp = self, "Building transcription bin");

        let ccconverter = gst::ElementFactory::make("ccconverter").build()?;

        state.transcription_bin.add_many([
            &state.ccmux,
            &state.ccmux_filter,
            &ccconverter,
            &state.cccapsfilter,
            &state.transcription_valve,
        ])?;

        gst::Element::link_many([
            &state.ccmux,
            &state.ccmux_filter,
            &ccconverter,
            &state.cccapsfilter,
            &state.transcription_valve,
        ])?;

        state.ccmux.set_property("latency", CEAX08MUX_LATENCY);

        let transcription_audio_srcpad =
            gst::GhostPad::with_target(&state.transcription_valve.static_pad("src").unwrap())
                .unwrap();

        state
            .transcription_bin
            .add_pad(&transcription_audio_srcpad)?;

        state.internal_bin.add(&state.transcription_bin)?;

        state.transcription_bin.set_locked_state(true);

        Ok(())
    }

    fn construct_internal_bin(&self, state: &mut State) -> Result<(), Error> {
        let vclocksync = gst::ElementFactory::make("clocksync")
            .name("vclocksync")
            .build()?;

        state
            .internal_bin
            .add_many([&vclocksync, &state.video_queue, &state.cccombiner])?;

        vclocksync.link(&state.video_queue)?;
        state
            .video_queue
            .link_pads(Some("src"), &state.cccombiner, Some("sink"))?;

        let internal_video_sinkpad =
            gst::GhostPad::builder_with_target(&vclocksync.static_pad("sink").unwrap())
                .unwrap()
                .name("video_sink")
                .build();
        let internal_video_srcpad =
            gst::GhostPad::builder_with_target(&state.cccombiner.static_pad("src").unwrap())
                .unwrap()
                .name("video_src")
                .build();

        state.internal_bin.add_pad(&internal_video_sinkpad)?;
        state.internal_bin.add_pad(&internal_video_srcpad)?;

        let imp_weak = self.downgrade();
        let comp_sinkpad = &state.cccombiner.static_pad("sink").unwrap();
        // Drop caption meta from video buffer if user preference is transcription
        comp_sinkpad.add_probe(gst::PadProbeType::BUFFER, move |_, probe_info| {
            let Some(imp) = imp_weak.upgrade() else {
                return gst::PadProbeReturn::Remove;
            };

            let settings = imp.settings.lock().unwrap();
            if settings.caption_source != CaptionSource::Transcription {
                return gst::PadProbeReturn::Pass;
            }

            if let Some(buffer) = probe_info.buffer_mut() {
                let buffer = buffer.make_mut();
                while let Some(meta) = buffer.meta_mut::<gst_video::VideoCaptionMeta>() {
                    meta.remove().unwrap();
                }
            }

            gst::PadProbeReturn::Ok
        });

        self.obj().add(&state.internal_bin)?;

        state.cccombiner.set_property("latency", CCCOMBINER_LATENCY);

        self.video_sinkpad
            .set_target(Some(&state.internal_bin.static_pad("video_sink").unwrap()))?;
        self.video_srcpad
            .set_target(Some(&state.internal_bin.static_pad("video_src").unwrap()))?;

        self.construct_transcription_bin(state)?;

        let pad = self
            .audio_sinkpad
            .downcast_ref::<super::TranscriberSinkPad>()
            .unwrap();
        // FIXME: replace this pattern with https://doc.rust-lang.org/nightly/std/sync/struct.MappedMutexGuard.html
        let ps = pad.imp().state.lock().unwrap();
        let pad_state = ps.as_ref().unwrap();
        let pad_settings = pad.imp().settings.lock().unwrap();

        self.link_input_audio_stream("sink_audio", pad_state, state, &pad_settings)?;

        let internal_audio_sinkpad =
            gst::GhostPad::builder_with_target(&pad_state.clocksync.static_pad("sink").unwrap())
                .unwrap()
                .name("audio_sink")
                .build();
        let internal_audio_srcpad = gst::GhostPad::builder_with_target(
            &pad_state.queue_passthrough.static_pad("src").unwrap(),
        )
        .unwrap()
        .name("audio_src")
        .build();

        state.internal_bin.add_pad(&internal_audio_sinkpad)?;
        state.internal_bin.add_pad(&internal_audio_srcpad)?;

        self.audio_sinkpad
            .set_target(Some(&state.internal_bin.static_pad("audio_sink").unwrap()))?;
        self.audio_srcpad
            .set_target(Some(&state.internal_bin.static_pad("audio_src").unwrap()))?;
        Ok(())
    }

    fn setup_transcription(&self, state: &State) {
        let settings = self.settings.lock().unwrap();
        let mut cc_caps = settings.cc_caps.clone();

        let cc_caps_mut = cc_caps.make_mut();
        let s = cc_caps_mut.structure_mut(0).unwrap();

        s.set("framerate", state.framerate.unwrap());

        state.cccapsfilter.set_property("caps", &cc_caps);

        let ccmux_caps = match state.mux_method {
            MuxMethod::Cea608 => gst::Caps::builder("closedcaption/x-cea-608")
                .field("framerate", state.framerate.unwrap())
                .build(),
            MuxMethod::Cea708 => gst::Caps::builder("closedcaption/x-cea-708")
                .field("format", "cc_data")
                .field("framerate", state.framerate.unwrap())
                .build(),
        };

        state.ccmux_filter.set_property("caps", ccmux_caps);

        let caps = match state.mux_method {
            MuxMethod::Cea608 => gst::Caps::builder("closedcaption/x-cea-608")
                .field("format", "raw")
                .field("framerate", state.framerate.unwrap())
                .build(),
            MuxMethod::Cea708 => gst::Caps::builder("closedcaption/x-cea-708")
                .field("format", "cc_data")
                .field("framerate", state.framerate.unwrap())
                .build(),
        };

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();
            if let Ok(pad_state) = ps.as_ref() {
                for channel in pad_state.caption_channels.values() {
                    channel.cccapsfilter.set_property("caps", &caps);
                }
            }
        }

        let max_size_time = self.our_latency(state, &settings);

        gst::debug!(
            CAT,
            "Calculated max size time for passthrough branches: {max_size_time}"
        );

        state.video_queue.set_property("max-size-bytes", 0u32);
        state.video_queue.set_property("max-size-buffers", 0u32);
        state
            .video_queue
            .set_property("max-size-time", max_size_time);

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();
            let pad_state = ps.as_ref().unwrap();

            if let Some(ref transcriber) = pad_state.transcriber {
                let latency_ms = settings.latency.mseconds() as u32;

                if transcriber.has_property_with_type("transcribe-latency", u32::static_type()) {
                    transcriber.set_property("transcribe-latency", latency_ms);
                } else if transcriber.has_property_with_type("latency", u32::static_type()) {
                    transcriber.set_property("latency", latency_ms);
                }

                if transcriber.has_property_with_type("translate-latency", u32::static_type()) {
                    let translate_latency_ms = settings.translate_latency.mseconds() as u32;
                    transcriber.set_property("translate-latency", translate_latency_ms);
                }
                if transcriber.has_property_with_type("lateness", u32::static_type()) {
                    let lateness_ms = settings.lateness.mseconds() as u32;
                    transcriber.set_property("lateness", lateness_ms);
                }
            }
            pad_state
                .queue_passthrough
                .set_property("max-size-bytes", 0u32);
            pad_state
                .queue_passthrough
                .set_property("max-size-buffers", 0u32);
            pad_state
                .queue_passthrough
                .set_property("max-size-time", max_size_time);
        }

        gst::debug!(
            CAT,
            imp = self,
            "Linking transcription bins and synchronizing state"
        );
        state
            .transcription_bin
            .link_pads(Some("src"), &state.cccombiner, Some("caption"))
            .unwrap();

        // We don't need to sync below playing as the state is now unlocked
        // and the bin will transition when necessary.
        //
        // Trying to sync in PAUSED was causing issues with base time distribution,
        // with cea608mux selecting an incorrect start time.
        let do_sync = self.obj().current_state() == gst::State::Playing;

        state.transcription_bin.set_locked_state(false);
        if do_sync {
            state.transcription_bin.sync_state_with_parent().unwrap();
        }

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();
            let pad_state = ps.as_ref().unwrap();
            let pad_settings = pad.imp().settings.lock().unwrap();

            if !pad_settings.passthrough {
                pad_state.transcription_bin.set_locked_state(false);

                if do_sync {
                    pad_state
                        .transcription_bin
                        .sync_state_with_parent()
                        .unwrap();
                }

                let transcription_sink_pad =
                    state.transcription_bin.static_pad(&pad.name()).unwrap();
                // Might be linked already if "translation-languages" is set
                if transcription_sink_pad.peer().is_none() {
                    let audio_tee_pad = pad_state.audio_tee.request_pad_simple("src_%u").unwrap();
                    audio_tee_pad.link(&transcription_sink_pad).unwrap();
                }
            }
        }

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();
            let pad_state = ps.as_ref().unwrap();
            let pad_settings = pad.imp().settings.lock().unwrap();
            self.setup_cc_mode(
                pad,
                pad_state,
                state.mux_method,
                pad_settings.mode,
                settings.accumulate_time,
            );
        }
    }

    fn disable_transcription_bin(
        &self,
        pad: &super::TranscriberSinkPad,
        state: &mut State,
        pad_state: &mut TranscriberSinkPadState,
    ) {
        if matches!(pad_state.passthrough_state, PassthroughState::Enabled) {
            gst::log!(CAT, obj = pad, "passthrough was already enabled");
            return;
        }

        gst::debug!(CAT, imp = self, "disabling transcription bin");

        let bin_sink_pad = state.transcription_bin.static_pad(&pad.name()).unwrap();
        if let Some(audio_tee_pad) = bin_sink_pad.peer() {
            audio_tee_pad.unlink(&bin_sink_pad).unwrap();

            gst::debug!(CAT, obj = pad, "releasing audio tee pad");

            pad_state.audio_tee.release_request_pad(&audio_tee_pad);
        }

        for channel in pad_state.caption_channels.values() {
            let srcpad = pad_state
                .transcription_bin
                .static_pad(&format!("src_{}", channel.language))
                .unwrap();

            if let Some(sinkpad) = state.ccmux.static_pad(&channel.ccmux_pad_name) {
                srcpad.unlink(&sinkpad).unwrap();
                state.ccmux.release_request_pad(&sinkpad);
            }
        }

        pad_state.transcription_bin.set_locked_state(true);
        pad_state
            .transcription_bin
            .set_state(gst::State::Null)
            .unwrap();

        pad_state.target_passthrough_state = TargetPassthroughState::None;
        pad_state.passthrough_state = PassthroughState::Enabled;
    }

    fn enable_transcription_bin(
        &self,
        sinkpad: &TranscriberSinkPad,
        state: &mut State,
        pad_state: &mut TranscriberSinkPadState,
    ) {
        if matches!(pad_state.passthrough_state, PassthroughState::Disabled) {
            gst::log!(CAT, imp = sinkpad, "passthrough was already disabled");
            return;
        }

        gst::debug!(CAT, imp = sinkpad, "enabling transcription bin");

        for channel in pad_state.caption_channels.values() {
            let srcpad = pad_state
                .transcription_bin
                .static_pad(&format!("src_{}", channel.language))
                .unwrap();

            gst::log!(
                CAT,
                obj = srcpad,
                "linking transcription channel for language {}",
                channel.language
            );

            let sinkpad = state
                .ccmux
                .static_pad(&channel.ccmux_pad_name)
                .unwrap_or_else(|| {
                    state
                        .ccmux
                        .request_pad_simple(&channel.ccmux_pad_name)
                        .unwrap()
                });
            srcpad.link(&sinkpad).unwrap();
        }
        pad_state.transcription_bin.set_locked_state(false);
        pad_state
            .transcription_bin
            .sync_state_with_parent()
            .unwrap();
        let audio_tee_pad = pad_state.audio_tee.request_pad_simple("src_%u").unwrap();
        let transcription_sink_pad = state
            .transcription_bin
            .static_pad(&sinkpad.obj().name())
            .unwrap();
        audio_tee_pad.link(&transcription_sink_pad).unwrap();
        pad_state.target_passthrough_state = TargetPassthroughState::None;
        pad_state.passthrough_state = PassthroughState::Disabled;
    }

    fn block_and_update(
        &self,
        sinkpad: &TranscriberSinkPad,
        mut s: std::sync::MutexGuard<Option<State>>,
        mut ps: std::sync::MutexGuard<Result<TranscriberSinkPadState, Error>>,
    ) {
        let Some(ref mut state) = s.as_mut() else {
            return;
        };

        let Ok(ref mut pad_state) = ps.as_mut() else {
            return;
        };

        match pad_state.target_passthrough_state {
            TargetPassthroughState::Enabled => {
                drop(s);
                drop(ps);
                let imp_weak = self.downgrade();
                let _ = sinkpad.obj().add_probe(
                    gst::PadProbeType::IDLE
                        | gst::PadProbeType::BUFFER
                        | gst::PadProbeType::EVENT_DOWNSTREAM,
                    move |pad, _info| {
                        let Some(imp) = imp_weak.upgrade() else {
                            return gst::PadProbeReturn::Remove;
                        };

                        let mut s = imp.state.lock().unwrap();

                        let Some(ref mut state) = s.as_mut() else {
                            return gst::PadProbeReturn::Remove;
                        };

                        gst::debug!(CAT, imp = imp, "pad probed");

                        let pad_imp = pad
                            .downcast_ref::<super::TranscriberSinkPad>()
                            .unwrap()
                            .imp();
                        let mut ps = pad_imp.state.lock().unwrap();
                        let Ok(ref mut pad_state) = ps.as_mut() else {
                            return gst::PadProbeReturn::Remove;
                        };

                        match pad_state.target_passthrough_state {
                            TargetPassthroughState::Enabled => {
                                imp.disable_transcription_bin(pad, state, pad_state);
                                // Now that we are done, make sure that this is reflected in our settings
                                let notify = {
                                    let mut pad_settings = pad_imp.settings.lock().unwrap();
                                    let old_passthrough = pad_settings.passthrough;
                                    pad_settings.passthrough = true;
                                    !old_passthrough
                                };

                                if notify {
                                    // We cannot notify from this thread, as this could cause
                                    // the user to reset the passthrough property, which would
                                    // in turn hang at link time (the probe would fire again
                                    // and deadlock)
                                    let pad_weak = pad.downgrade();
                                    imp.obj().call_async(move |_| {
                                        let Some(pad) = pad_weak.upgrade() else {
                                            return;
                                        };
                                        pad.notify("passthrough");
                                    });
                                }
                            }
                            TargetPassthroughState::Disabled => {
                                // Now at this point, even though we initially blocked in
                                // order to enable passthrough, the user may instead have
                                // requested disabling it in the meantime.
                                imp.enable_transcription_bin(pad_imp, state, pad_state);
                            }
                            _ => (),
                        }

                        gst::PadProbeReturn::Remove
                    },
                );
            }
            TargetPassthroughState::Disabled => {
                self.enable_transcription_bin(sinkpad, state, pad_state);
            }
            _ => (),
        }
    }

    fn setup_cc_mode(
        &self,
        pad: &super::TranscriberSinkPad,
        pad_state: &TranscriberSinkPadState,
        mux_method: MuxMethod,
        mode: Cea608Mode,
        accumulate_time: gst::ClockTime,
    ) {
        gst::debug!(
            CAT,
            imp = self,
            "setting CC mode {:?} for pad {:?}",
            mode,
            pad
        );

        for channel in pad_state.caption_channels.values() {
            match mux_method {
                MuxMethod::Cea608 => channel.tttoceax08.set_property("mode", mode),
                MuxMethod::Cea708 => match mode {
                    Cea608Mode::PopOn => channel.tttoceax08.set_property("mode", Cea708Mode::PopOn),
                    Cea608Mode::PaintOn => {
                        channel.tttoceax08.set_property("mode", Cea708Mode::PaintOn)
                    }
                    Cea608Mode::RollUp2 | Cea608Mode::RollUp3 | Cea608Mode::RollUp4 => {
                        channel.tttoceax08.set_property("mode", Cea708Mode::RollUp)
                    }
                },
            }

            if mode.is_rollup() {
                channel.textwrap.set_property("accumulate-time", 0u64);
            } else {
                channel
                    .textwrap
                    .set_property("accumulate-time", accumulate_time);
            }
        }
    }

    /* We make no ceremonies here because the function can only
     * be called in READY */
    fn relink_transcriber(
        &self,
        state: &State,
        pad_state: &TranscriberSinkPadState,
        old_transcriber: Option<&gst::Element>,
    ) -> Result<(), Error> {
        gst::debug!(
            CAT,
            imp = self,
            "Relinking transcriber, old: {:?}, new: {:?}",
            old_transcriber,
            pad_state.transcriber
        );

        if let Some(old_transcriber) = old_transcriber {
            gst::debug!(CAT, obj = old_transcriber, "Unlinking old transcriber");
            pad_state.transcriber_aconv.unlink(old_transcriber);
            pad_state.unlink_language_tees(self.obj().as_ref(), state, pad_state);
            let _ = pad_state.transcription_bin.remove(old_transcriber);
            old_transcriber.set_state(gst::State::Null).unwrap();
        }

        if let Some(ref transcriber) = pad_state.transcriber {
            gst::debug!(CAT, obj = transcriber, "Linking new transcriber");
            pad_state.transcription_bin.add(transcriber)?;
            transcriber.sync_state_with_parent().unwrap();
            pad_state.transcriber_aconv.link(transcriber)?;

            self.link_transcriber_to_channels(state, pad_state)?;
        }

        Ok(())
    }

    fn construct_channels(
        &self,
        mux_method: MuxMethod,
        pad_state: &mut TranscriberSinkPadState,
        pad_settings: &TranscriberSinkPadSettings,
    ) -> Result<(), Error> {
        self.construct_caption_channels(pad_settings, mux_method, pad_state)
            .unwrap();

        self.construct_synthesis_channels(pad_settings, pad_state)
            .unwrap();

        self.construct_subtitle_channels(pad_settings, pad_state)
            .unwrap();

        for k in pad_state
            .caption_channels
            .keys()
            .chain(pad_state.synthesis_channels.keys())
            .chain(pad_state.subtitle_channels.keys())
        {
            use std::collections::hash_map::Entry::*;
            if let Vacant(e) = pad_state.language_tees.entry(k.clone()) {
                let tee = gst::ElementFactory::make("tee")
                    .name(format!("tee-{k}"))
                    .property("allow-not-linked", true)
                    .build()?;

                pad_state.transcription_bin.add(&tee)?;

                if let Some(val) = pad_settings
                    .language_filters
                    .as_ref()
                    .and_then(|f| f.value(k).ok())
                {
                    let filter = if val.is::<String>() {
                        let bin_description = val.get::<String>().unwrap();
                        gst::parse::bin_from_description_full(
                            &bin_description,
                            true,
                            None,
                            gst::ParseFlags::NO_SINGLE_ELEMENT_BINS,
                        )?
                    } else if val.is::<gst::Element>() {
                        val.get::<gst::Element>().unwrap()
                    } else {
                        return Err(anyhow!(
                            "Value for language filter map must be string or element"
                        ));
                    };

                    pad_state.transcription_bin.add(&filter)?;
                    filter.link(&tee)?;
                    pad_state.language_filters.insert(k.clone(), filter);
                }

                e.insert(tee);
            };
        }

        Ok(())
    }

    fn tear_down_channels(
        &self,
        state: &State,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        pad_state.unlink_language_tees(self.obj().as_ref(), state, pad_state);

        self.tear_down_caption_channels(state, pad_state)?;

        self.tear_down_synthesis_channels(state, pad_state)?;

        self.tear_down_subtitle_channels(state, pad_state)?;

        for (language, tee) in pad_state.language_tees.drain() {
            if let Some(filter) = pad_state.language_filters.remove(&language) {
                let _ = pad_state.transcription_bin.remove(&filter);
                let _ = filter.set_state(gst::State::Null);
                // At this point the filter is unlinked, removed from the bin and
                // its state is reset. If it was directly provided by the user then
                // it remains reffed in the settings structure, otherwise it goes
                // out of scope here and will get recreated.
            }
            let _ = pad_state.transcription_bin.remove(&tee);
        }

        Ok(())
    }

    fn tear_down_custom_output_channel(
        &self,
        channel: CustomOutputChannel,
        pad_transcription_bin: &gst::Bin,
        transcription_bin: &gst::Bin,
        internal_bin: &gst::Bin,
        serial: Option<u32>,
    ) -> Result<(), Error> {
        let mut pad_name = format!("src_{}_{}", channel.type_.name(), channel.language);
        let srcpad = pad_transcription_bin.static_pad(&pad_name).unwrap();

        let _ = pad_transcription_bin.remove_pad(&srcpad);

        let channel_sinkpad = channel.bin.static_pad("sink").unwrap();

        if let Some(tee_srcpad) = channel_sinkpad.peer() {
            let _ = tee_srcpad.unlink(&channel_sinkpad);
            if let Some(tee) = tee_srcpad
                .parent()
                .and_then(|p| p.downcast::<gst::Element>().ok())
            {
                let _ = tee.remove_pad(&tee_srcpad);
            }
        }

        pad_transcription_bin.remove(&channel.bin)?;

        let _ = channel.bin.set_state(gst::State::Null);

        if let Some(serial) = serial {
            pad_name = format!("{pad_name}_{serial}");
        }
        let srcpad = transcription_bin.static_pad(&pad_name).unwrap();
        let _ = transcription_bin.remove_pad(&srcpad);

        let srcpad = internal_bin.static_pad(&pad_name).unwrap();
        let _ = internal_bin.remove_pad(&srcpad);
        let srcpad = self.obj().static_pad(&pad_name).unwrap();
        let _ = self.obj().remove_pad(&srcpad);

        Ok(())
    }

    fn tear_down_caption_channel(
        &self,
        channel: CaptionChannel,
        pad_transcription_bin: &gst::Bin,
        ccmux: &gst::Element,
    ) -> Result<(), Error> {
        let channel_sinkpad = channel.bin.static_pad("sink").unwrap();

        if let Some(tee_srcpad) = channel_sinkpad.peer() {
            let _ = tee_srcpad.unlink(&channel_sinkpad);
            if let Some(tee) = tee_srcpad
                .parent()
                .and_then(|p| p.downcast::<gst::Element>().ok())
            {
                let _ = tee.remove_pad(&tee_srcpad);
            }
        }

        let srcpad = pad_transcription_bin
            .static_pad(&format!("src_{}", channel.language))
            .unwrap();

        if let Some(peer) = srcpad.peer() {
            // The source pad might not have been linked to the muxer initially, for
            // instance in case of a collision with another source pad's
            // translation-languages mapping
            if peer.parent().and_downcast_ref::<gst::Element>() == Some(ccmux) {
                srcpad.unlink(&peer)?;
                ccmux.release_request_pad(&peer);
            }
        }

        let _ = pad_transcription_bin.remove_pad(&srcpad);

        pad_transcription_bin.remove(&channel.bin)?;

        let _ = channel.bin.set_state(gst::State::Null);

        Ok(())
    }

    fn tear_down_synthesis_channels(
        &self,
        state: &State,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        let channels: Vec<_> = pad_state.synthesis_channels.drain().collect();
        for (_, channel) in channels {
            self.tear_down_custom_output_channel(
                channel,
                &pad_state.transcription_bin,
                &state.transcription_bin,
                &state.internal_bin,
                pad_state.serial,
            )?;
        }

        Ok(())
    }

    fn tear_down_subtitle_channels(
        &self,
        state: &State,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        let channels: Vec<_> = pad_state.subtitle_channels.drain().collect();
        for (_, channel) in channels {
            self.tear_down_custom_output_channel(
                channel,
                &pad_state.transcription_bin,
                &state.transcription_bin,
                &state.internal_bin,
                pad_state.serial,
            )?;
        }

        Ok(())
    }

    fn tear_down_caption_channels(
        &self,
        state: &State,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        let channels: Vec<_> = pad_state.caption_channels.drain().collect();

        for (_, channel) in channels {
            self.tear_down_caption_channel(channel, &pad_state.transcription_bin, &state.ccmux)?;
        }

        Ok(())
    }

    fn construct_synthesis_channels(
        &self,
        pad_settings: &TranscriberSinkPadSettings,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        if let Some(ref map) = pad_settings.synthesis_languages {
            for (language, value) in map.iter() {
                let bin_description = value.get::<String>()?;
                pad_state.synthesis_channels.insert(
                    language.to_string(),
                    self.construct_custom_output_channel(
                        language,
                        &bin_description,
                        CustomOutputChannelType::Synthesis,
                    )?,
                );
            }

            for channel in pad_state.synthesis_channels.values() {
                pad_state.transcription_bin.add(&channel.bin)?;
            }
        }

        Ok(())
    }

    fn construct_subtitle_channels(
        &self,
        pad_settings: &TranscriberSinkPadSettings,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        if let Some(ref map) = pad_settings.subtitle_languages {
            for (language, value) in map.iter() {
                let bin_description = value.get::<String>()?;
                pad_state.subtitle_channels.insert(
                    language.to_string(),
                    self.construct_custom_output_channel(
                        language,
                        &bin_description,
                        CustomOutputChannelType::Subtitle,
                    )?,
                );
            }

            for channel in pad_state.subtitle_channels.values() {
                pad_state.transcription_bin.add(&channel.bin)?;
            }
        }

        Ok(())
    }

    fn construct_caption_channels(
        &self,
        settings: &TranscriberSinkPadSettings,
        mux_method: MuxMethod,
        pad_state: &mut TranscriberSinkPadState,
    ) -> Result<(), Error> {
        if let Some(ref map) = settings.translation_languages {
            for (key, value) in map.iter() {
                let (language_code, caption_streams) = parse_language_pair(mux_method, key, value)?;

                pad_state.caption_channels.insert(
                    language_code.to_owned(),
                    self.construct_transcription_channel(
                        &language_code,
                        mux_method,
                        caption_streams,
                    )?,
                );
            }
        } else {
            let caption_streams = match mux_method {
                MuxMethod::Cea608 => HashSet::from(["cc1".to_string()]),
                MuxMethod::Cea708 => HashSet::from(["cc1".to_string(), "708_1".to_string()]),
            };
            pad_state.caption_channels.insert(
                "transcript".to_string(),
                self.construct_transcription_channel("transcript", mux_method, caption_streams)?,
            );
        }

        for channel in pad_state.caption_channels.values() {
            pad_state.transcription_bin.add(&channel.bin)?;
        }

        Ok(())
    }

    fn prepare_caption_channel_updates(
        &self,
        state: &State,
        pad_state: &mut TranscriberSinkPadState,
        settings: &TranscriberSinkPadSettings,
    ) -> Result<(Vec<CaptionChannel>, Vec<CaptionChannel>), Error> {
        let mut updates = HashMap::new();
        let mut old_languages: HashSet<String> =
            pad_state.caption_channels.keys().cloned().collect();

        let translation_languages =
            settings
                .translation_languages
                .clone()
                .unwrap_or(match state.mux_method {
                    MuxMethod::Cea608 => gst::Structure::builder("languages")
                        .field("transcript", "cc1")
                        .build(),
                    MuxMethod::Cea708 => gst::Structure::builder("languages")
                        .field(
                            "transcript",
                            gst::List::from_values([
                                "cc1".to_string().to_send_value(),
                                "708_1".to_string().to_send_value(),
                            ]),
                        )
                        .build(),
                });

        for (key, value) in translation_languages.iter() {
            let (language_code, caption_streams) =
                parse_language_pair(state.mux_method, key, value)?;

            old_languages.remove(&language_code);

            if let Some(channel) = pad_state.caption_channels.get(&language_code) {
                if channel.caption_streams != caption_streams {
                    updates.insert(language_code, CaptionChannelUpdate::Upsert(caption_streams));
                }
            } else {
                updates.insert(language_code, CaptionChannelUpdate::Upsert(caption_streams));
            }
        }

        for language_code in old_languages.drain() {
            updates.insert(language_code, CaptionChannelUpdate::Remove);
        }

        let mut channels_to_remove = vec![];
        let mut channels_to_add = vec![];

        for (language_code, update) in updates {
            match update {
                CaptionChannelUpdate::Remove => {
                    let transcription_channel =
                        pad_state.caption_channels.remove(&language_code).unwrap();

                    channels_to_remove.push(transcription_channel);

                    gst::debug!(
                        CAT,
                        imp = self,
                        "caption channel {language_code} will be removed"
                    );
                }
                CaptionChannelUpdate::Upsert(caption_streams) => {
                    if let Some(transcription_channel) =
                        pad_state.caption_channels.remove(&language_code)
                    {
                        channels_to_remove.push(transcription_channel);

                        gst::debug!(
                            CAT,
                            imp = self,
                            "caption channel {language_code} will be removed"
                        );
                    }

                    let transcription_channel = self.construct_transcription_channel(
                        &language_code,
                        state.mux_method,
                        caption_streams,
                    )?;
                    channels_to_add.push(transcription_channel.clone());
                    pad_state
                        .caption_channels
                        .insert(language_code.clone(), transcription_channel);

                    gst::debug!(
                        CAT,
                        imp = self,
                        "caption channel {language_code} will be added"
                    );
                }
            }
        }

        Ok((channels_to_remove, channels_to_add))
    }

    fn prepare_custom_output_channel_updates(
        &self,
        languages: &Option<gst::Structure>,
        channels_map: &mut HashMap<String, CustomOutputChannel>,
        channel_type: CustomOutputChannelType,
    ) -> Result<(Vec<CustomOutputChannel>, Vec<CustomOutputChannel>), Error> {
        let mut updates = HashMap::new();
        let mut old_languages: HashSet<String> = channels_map.keys().cloned().collect();

        if let Some(ref map) = languages {
            for (key, value) in map.iter() {
                let language_code = key.to_string();

                old_languages.remove(&language_code);

                let bin_description = value.get::<String>()?;
                if let Some(channel) = channels_map.get(&language_code) {
                    if channel.bin_description != bin_description {
                        updates.insert(language_code, CustomChannelUpdate::Upsert(bin_description));
                    }
                } else {
                    updates.insert(language_code, CustomChannelUpdate::Upsert(bin_description));
                }
            }
        }

        for language_code in old_languages.drain() {
            updates.insert(language_code, CustomChannelUpdate::Remove);
        }

        let mut channels_to_remove = vec![];
        let mut channels_to_add = vec![];

        for (language_code, update) in updates {
            match update {
                CustomChannelUpdate::Remove => {
                    let channel = channels_map.remove(&language_code).unwrap();

                    channels_to_remove.push(channel);

                    gst::debug!(
                        CAT,
                        imp = self,
                        "{} channel {language_code} will be removed",
                        channel_type.name()
                    );
                }
                CustomChannelUpdate::Upsert(bin_description) => {
                    if let Some(channel) = channels_map.remove(&language_code) {
                        channels_to_remove.push(channel);

                        gst::debug!(
                            CAT,
                            imp = self,
                            "{} channel {language_code} will be removed",
                            channel_type.name()
                        );
                    }

                    let channel = self.construct_custom_output_channel(
                        &language_code,
                        &bin_description,
                        channel_type,
                    )?;
                    channels_to_add.push(channel.clone());
                    channels_map.insert(language_code.clone(), channel);

                    gst::debug!(
                        CAT,
                        imp = self,
                        "{} channel {language_code} will be added",
                        channel_type.name()
                    );
                }
            }
        }

        Ok((channels_to_remove, channels_to_add))
    }

    #[allow(clippy::type_complexity)]
    fn prepare_language_updates(
        &self,
        pad_state: &mut TranscriberSinkPadState,
        pad_settings: &TranscriberSinkPadSettings,
        mut old_languages: HashSet<String>,
    ) -> Result<
        (
            Vec<(gst::Element, Option<gst::Element>)>,
            HashMap<String, (gst::Element, Option<gst::Element>)>,
        ),
        Error,
    > {
        let mut languages_to_add: HashMap<String, (gst::Element, Option<gst::Element>)> =
            HashMap::new();

        for k in pad_state
            .caption_channels
            .keys()
            .chain(pad_state.synthesis_channels.keys())
            .chain(pad_state.subtitle_channels.keys())
        {
            use std::collections::hash_map::Entry::*;
            if let Vacant(e) = pad_state.language_tees.entry(k.clone()) {
                let tee = gst::ElementFactory::make("tee")
                    .name(format!("tee-{k}"))
                    .property("allow-not-linked", true)
                    .build()?;

                if let Some(val) = pad_settings
                    .language_filters
                    .as_ref()
                    .and_then(|f| f.value(k).ok())
                {
                    let filter = if val.is::<String>() {
                        let bin_description = val.get::<String>().unwrap();
                        gst::parse::bin_from_description_full(
                            &bin_description,
                            true,
                            None,
                            gst::ParseFlags::NO_SINGLE_ELEMENT_BINS,
                        )?
                    } else if val.is::<gst::Element>() {
                        val.get::<gst::Element>().unwrap()
                    } else {
                        return Err(anyhow!(
                            "Value for language filter map must be string or element"
                        ));
                    };

                    languages_to_add.insert(k.clone(), (tee.clone(), Some(filter.clone())));
                    pad_state.language_filters.insert(k.clone(), filter);

                    gst::debug!(CAT, imp = self, "language {k} will be added");
                } else {
                    languages_to_add.insert(k.clone(), (tee.clone(), None));

                    gst::debug!(CAT, imp = self, "language {k} will be added");
                }

                e.insert(tee);
            };

            old_languages.remove(k);
        }

        let mut languages_to_remove: Vec<(gst::Element, Option<gst::Element>)> = vec![];

        for language in old_languages.drain() {
            if let Some(tee) = pad_state.language_tees.remove(&language) {
                let filter = pad_state.language_filters.remove(&language);
                languages_to_remove.push((tee, filter));

                gst::debug!(CAT, imp = self, "language {language} will be removed");
            }
        }

        Ok((languages_to_remove, languages_to_add))
    }

    fn remove_caption_channels(
        &self,
        pad_transcription_bin: &gst::Bin,
        ccmux: &gst::Element,
        mut channels_to_remove: Vec<CaptionChannel>,
    ) -> Result<(), Error> {
        for channel in channels_to_remove.drain(..) {
            self.tear_down_caption_channel(channel, pad_transcription_bin, ccmux)?;
        }

        Ok(())
    }

    fn remove_custom_output_channels(
        &self,
        pad_transcription_bin: &gst::Bin,
        transcription_bin: &gst::Bin,
        internal_bin: &gst::Bin,
        serial: Option<u32>,
        mut channels_to_remove: Vec<CustomOutputChannel>,
    ) -> Result<(), Error> {
        for channel in channels_to_remove.drain(..) {
            self.tear_down_custom_output_channel(
                channel,
                pad_transcription_bin,
                transcription_bin,
                internal_bin,
                serial,
            )?;
        }

        Ok(())
    }

    fn remove_languages(
        &self,
        pad_transcription_bin: &gst::Bin,
        mut languages_to_remove: Vec<(gst::Element, Option<gst::Element>)>,
    ) {
        for (tee, filter) in languages_to_remove.drain(..) {
            let sinkpad = if let Some(ref filter) = filter {
                filter.static_pad("sink").unwrap()
            } else {
                tee.static_pad("sink").unwrap()
            };

            if let Some(srcpad) = sinkpad.peer() {
                let _ = srcpad.unlink(&sinkpad);

                if srcpad.name() != "src" {
                    srcpad
                        .parent()
                        .unwrap()
                        .downcast::<gst::Element>()
                        .unwrap()
                        .release_request_pad(&srcpad);
                }
            }

            let _ = pad_transcription_bin.remove(&tee);
            let _ = tee.set_state(gst::State::Null);
            if let Some(filter) = filter {
                let _ = pad_transcription_bin.remove(&filter);
                let _ = filter.set_state(gst::State::Null);
            }
        }
    }

    fn add_languages(
        &self,
        transcriber: &gst::Element,
        pad_transcription_bin: &gst::Bin,
        mut languages_to_add: HashMap<String, (gst::Element, Option<gst::Element>)>,
    ) -> Result<(), Error> {
        for (language, (tee, filter)) in languages_to_add.drain() {
            let transcriber_srcpad = match language.as_str() {
                "transcript" => transcriber
                    .static_pad("src")
                    .ok_or(anyhow!("Failed to retrieve transcription source pad"))?,
                language => {
                    let pad = transcriber
                        .request_pad_simple("translate_src_%u")
                        .ok_or(anyhow!("Failed to request translation source pad"))?;
                    pad.set_property("language-code", language);
                    pad
                }
            };

            pad_transcription_bin.add(&tee)?;
            tee.sync_state_with_parent()?;

            if let Some(filter) = filter {
                pad_transcription_bin.add(&filter)?;
                filter.link(&tee)?;
                filter.sync_state_with_parent()?;
                transcriber_srcpad.link(&filter.static_pad("sink").unwrap())?;
            } else {
                transcriber_srcpad.link(&tee.static_pad("sink").unwrap())?;
            }
        }

        Ok(())
    }

    fn add_caption_channels(
        &self,
        pad_transcription_bin: &gst::Bin,
        ccmux: &gst::Element,
        language_tees: &HashMap<String, gst::Element>,
        passthrough: bool,
        mut channels_to_add: Vec<CaptionChannel>,
    ) -> Result<(), Error> {
        for channel in channels_to_add.drain(..) {
            pad_transcription_bin.add(&channel.bin)?;

            let srcpad =
                gst::GhostPad::builder_with_target(&channel.bin.static_pad("src").unwrap())
                    .unwrap()
                    .name(format!("src_{}", channel.language))
                    .build();

            pad_transcription_bin.add_pad(&srcpad)?;

            if !passthrough {
                let sinkpad = ccmux
                    .static_pad(&channel.ccmux_pad_name)
                    .unwrap_or_else(|| ccmux.request_pad_simple(&channel.ccmux_pad_name).unwrap());

                srcpad.link(&sinkpad)?;
            }

            channel.bin.sync_state_with_parent()?;

            let tee = language_tees.get(&channel.language).unwrap();

            let tee_srcpad = tee.request_pad_simple("src_%u").unwrap();

            gst::debug!(CAT, obj = tee_srcpad, "Linking language tee to channel");

            tee_srcpad.link(&channel.bin.static_pad("sink").unwrap())?;
        }

        Ok(())
    }

    fn expose_custom_output_pads(
        &self,
        pad_transcription_bin: &gst::Bin,
        transcription_bin: &gst::Bin,
        internal_bin: &gst::Bin,
        serial: Option<u32>,
        channel: &CustomOutputChannel,
    ) -> Result<(), Error> {
        let mut pad_name = format!("src_{}_{}", channel.type_.name(), channel.language);
        let srcpad = gst::GhostPad::builder_with_target(&channel.bin.static_pad("src").unwrap())
            .unwrap()
            .name(&pad_name)
            .build();

        pad_transcription_bin.add_pad(&srcpad)?;

        if let Some(serial) = serial {
            pad_name = format!("{pad_name}_{serial}");
        }

        let srcpad = gst::GhostPad::builder_with_target(&srcpad)
            .unwrap()
            .name(pad_name.clone())
            .build();
        transcription_bin.add_pad(&srcpad)?;

        let srcpad = gst::GhostPad::with_target(&srcpad).unwrap();
        internal_bin.add_pad(&srcpad)?;

        let srcpad = gst::GhostPad::with_target(&srcpad).unwrap();
        self.obj().add_pad(&srcpad)?;

        Ok(())
    }

    fn add_custom_output_channels(
        &self,
        pad_transcription_bin: &gst::Bin,
        transcription_bin: &gst::Bin,
        internal_bin: &gst::Bin,
        serial: Option<u32>,
        language_tees: &HashMap<String, gst::Element>,
        mut channels_to_add: Vec<CustomOutputChannel>,
    ) -> Result<(), Error> {
        for channel in channels_to_add.drain(..) {
            pad_transcription_bin.add(&channel.bin)?;

            self.expose_custom_output_pads(
                pad_transcription_bin,
                transcription_bin,
                internal_bin,
                serial,
                &channel,
            )?;

            channel.bin.sync_state_with_parent()?;

            let tee = language_tees.get(&channel.language).unwrap();

            let tee_srcpad = tee.request_pad_simple("src_%u").unwrap();

            gst::debug!(CAT, obj = tee_srcpad, "Linking language tee to channel");

            tee_srcpad.link(&channel.bin.static_pad("sink").unwrap())?;
        }

        Ok(())
    }

    fn reconfigure_transcription_bin_dynamic(&self, pad: &TranscriberSinkPad) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();

        gst::info!(CAT, imp = self, "Reconfiguring transcription bin");

        if let Some(ref mut state) = s.as_mut() {
            let mut ps = pad.state.lock().unwrap();
            let pad_state = ps.as_mut().unwrap();
            let pad_settings = pad.settings.lock().unwrap();

            let old_languages: HashSet<String> = pad_state
                .caption_channels
                .keys()
                .chain(pad_state.synthesis_channels.keys())
                .chain(pad_state.subtitle_channels.keys())
                .cloned()
                .collect();

            gst::debug!(CAT, imp = self, "old languages: {old_languages:?}");

            let (caption_channels_to_remove, caption_channels_to_add) =
                self.prepare_caption_channel_updates(state, pad_state, &pad_settings)?;

            let (synthesis_channels_to_remove, synthesis_channels_to_add) = self
                .prepare_custom_output_channel_updates(
                    &pad_settings.synthesis_languages,
                    &mut pad_state.synthesis_channels,
                    CustomOutputChannelType::Synthesis,
                )?;

            let (subtitle_channels_to_remove, subtitle_channels_to_add) = self
                .prepare_custom_output_channel_updates(
                    &pad_settings.subtitle_languages,
                    &mut pad_state.subtitle_channels,
                    CustomOutputChannelType::Subtitle,
                )?;

            let (languages_to_remove, languages_to_add) =
                self.prepare_language_updates(pad_state, &pad_settings, old_languages)?;

            let ccmux = state.ccmux.clone();
            let pad_transcription_bin = pad_state.transcription_bin.clone();
            let transcriber = pad_state.transcriber.clone().unwrap();
            let language_tees = pad_state.language_tees.clone();
            let passthrough = pad_settings.passthrough;
            let serial = pad_state.serial;
            let transcription_bin = state.transcription_bin.clone();
            let internal_bin = state.internal_bin.clone();

            drop(s);
            drop(pad_settings);
            drop(ps);

            gst::debug!(
                CAT,
                imp = self,
                "update prepared, releasing locks and applying"
            );

            self.remove_caption_channels(
                &pad_transcription_bin,
                &ccmux,
                caption_channels_to_remove,
            )?;

            self.remove_custom_output_channels(
                &pad_transcription_bin,
                &transcription_bin,
                &internal_bin,
                serial,
                [synthesis_channels_to_remove, subtitle_channels_to_remove].concat(),
            )?;

            self.remove_languages(&pad_transcription_bin, languages_to_remove);

            self.add_languages(&transcriber, &pad_transcription_bin, languages_to_add)?;

            self.add_caption_channels(
                &pad_transcription_bin,
                &ccmux,
                &language_tees,
                passthrough,
                caption_channels_to_add,
            )?;

            self.add_custom_output_channels(
                &pad_transcription_bin,
                &transcription_bin,
                &internal_bin,
                serial,
                &language_tees,
                [synthesis_channels_to_add, subtitle_channels_to_add].concat(),
            )?;
        }

        Ok(())
    }

    fn reconfigure_transcription_bin(
        &self,
        pad: &TranscriberSinkPad,
        lang_code_only: bool,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();

        if let Some(ref mut state) = state.as_mut() {
            let settings = self.settings.lock().unwrap();
            let mut ps = pad.state.lock().unwrap();
            let pad_state = ps.as_mut().unwrap();
            let pad_settings = pad.settings.lock().unwrap();

            gst::debug!(
                CAT,
                imp = self,
                "Updating transcription/translation language"
            );

            // Unlink sinkpad temporarily
            let sinkpad = state
                .transcription_bin
                .static_pad(&pad.obj().name())
                .unwrap();
            let peer = sinkpad.peer();
            let mut relink_tee = false;
            if let Some(peer) = &peer {
                gst::debug!(CAT, imp = self, "Unlinking {:?}", peer);
                peer.unlink(&sinkpad)?;
                pad_state.audio_tee.release_request_pad(peer);
                relink_tee = true;
            }

            pad_state.transcription_bin.set_locked_state(true);
            pad_state
                .transcription_bin
                .set_state(gst::State::Null)
                .unwrap();

            if let Some(ref transcriber) = pad_state.transcriber {
                transcriber.set_property("language-code", &pad_settings.language_code);
            }

            if lang_code_only {
                if !pad_settings.passthrough {
                    gst::debug!(CAT, imp = self, "Syncing state with parent");

                    drop(settings);

                    // While we haven't locked the state here, the state of the
                    // top level transcription bin might be locked, for instance
                    // at start up. Unlock and sync both the inner and top level
                    // bin states to ensure data flows in the correct state
                    state.transcription_bin.set_locked_state(false);
                    state.transcription_bin.sync_state_with_parent().unwrap();

                    pad_state.transcription_bin.set_locked_state(false);
                    pad_state.transcription_bin.sync_state_with_parent()?;

                    let audio_tee_pad = pad_state.audio_tee.request_pad_simple("src_%u").unwrap();
                    audio_tee_pad.link(&sinkpad)?;
                }

                return Ok(());
            }

            self.tear_down_channels(state, pad_state)?;

            self.construct_channels(state.mux_method, pad_state, &pad_settings)?;

            self.link_transcriber_to_channels(state, pad_state)?;

            self.expose_channel_outputs(state, pad_state, &pad_settings)?;

            self.setup_cc_mode(
                &pad.obj(),
                pad_state,
                state.mux_method,
                pad_settings.mode,
                settings.accumulate_time,
            );

            if !pad_settings.passthrough {
                gst::debug!(CAT, imp = self, "Syncing state with parent");

                drop(pad_settings);
                drop(settings);

                pad_state.transcription_bin.set_locked_state(false);
                pad_state.transcription_bin.sync_state_with_parent()?;
                if relink_tee {
                    let audio_tee_pad = pad_state.audio_tee.request_pad_simple("src_%u").unwrap();

                    audio_tee_pad.link(&sinkpad)?;
                }
            }
        }

        Ok(())
    }

    fn update_languages(&self, pad: &super::TranscriberSinkPad, lang_code_only: bool) {
        gst::debug!(
            CAT,
            imp = self,
            "Schedule transcription/translation language update for pad {pad:?}"
        );

        let Some(sinkpad) = pad
            .imp()
            .state
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .transcription_bin
            .static_pad(pad.name().as_str())
        else {
            gst::debug!(CAT, imp = pad.imp(), "transcription bin not set up yet");
            return;
        };

        let imp_weak = self.downgrade();
        let pad_weak = pad.downgrade();

        let _ = sinkpad.add_probe(
            gst::PadProbeType::IDLE
                | gst::PadProbeType::BUFFER
                | gst::PadProbeType::EVENT_DOWNSTREAM,
            move |_pad, _info| {
                let Some(imp) = imp_weak.upgrade() else {
                    return gst::PadProbeReturn::Remove;
                };

                let Some(pad) = pad_weak.upgrade() else {
                    return gst::PadProbeReturn::Remove;
                };

                if let Err(e) = imp.reconfigure_transcription_bin(pad.imp(), lang_code_only) {
                    gst::error!(CAT, "Couldn't reconfigure channels: {e}");
                    gst::element_imp_error!(
                        imp,
                        gst::StreamError::Failed,
                        ["Couldn't reconfigure channels: {}", e]
                    );
                    *imp.state.lock().unwrap() = None;
                }

                gst::PadProbeReturn::Remove
            },
        );
    }

    fn query_upstream_latency(&self, state_guard: MutexGuard<Option<State>>) -> gst::ClockTime {
        let state = state_guard.as_ref().unwrap();
        let mut min = gst::ClockTime::from_seconds(0);

        let pads = Iterator::chain(
            state
                .audio_sink_pads
                .values()
                .map(|p| p.clone().upcast::<gst::Pad>()),
            std::iter::once(&self.video_sinkpad).map(|p| p.clone().upcast::<gst::Pad>()),
        )
        .collect::<Vec<_>>();
        drop(state_guard);

        for pad in pads {
            let mut upstream_query = gst::query::Latency::new();

            if pad.peer_query(&mut upstream_query) {
                let (_, upstream_min, _) = upstream_query.result();

                if min < upstream_min {
                    min = upstream_min;
                }
            }
        }

        min
    }

    fn synthesis_latency(&self, state: &State) -> gst::ClockTime {
        let mut ret = gst::ClockTime::ZERO;

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();

            if let Ok(pad_state) = ps.as_ref() {
                for channel in pad_state.synthesis_channels.values() {
                    if channel.latency > ret {
                        ret = channel.latency;
                    }
                }
            }
        }

        ret
    }

    fn subtitle_latency(&self, state: &State) -> gst::ClockTime {
        let mut ret = gst::ClockTime::ZERO;

        for pad in state.audio_sink_pads.values() {
            let ps = pad.imp().state.lock().unwrap();

            if let Ok(pad_state) = ps.as_ref() {
                for channel in pad_state.subtitle_channels.values() {
                    if channel.latency > ret {
                        ret = channel.latency;
                    }
                }
            }
        }

        ret
    }

    fn our_latency(&self, state: &State, settings: &Settings) -> gst::ClockTime {
        [
            settings.latency + self.subtitle_latency(state),
            settings.latency + self.synthesis_latency(state),
            settings.latency
                + settings.accumulate_time
                + CEAX08MUX_LATENCY
                + settings.translate_latency
                + CCCOMBINER_LATENCY
                + state
                    .framerate
                    .map(|f| {
                        2 * gst::ClockTime::SECOND
                            .mul_div_floor(f.denom() as u64, f.numer() as u64)
                            .unwrap()
                    })
                    .unwrap_or(gst::ClockTime::from_seconds(0)),
        ]
        .iter()
        .max()
        .unwrap()
        .to_owned()
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::trace!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let state_guard = self.state.lock().unwrap();
                if let Some(ref state) = &*state_guard {
                    let our_latency = self.our_latency(state, &self.settings.lock().unwrap());
                    let upstream_min = self.query_upstream_latency(state_guard);

                    let min = upstream_min + our_latency;

                    gst::debug!(CAT, imp = self, "calculated latency: {}", min);

                    q.set(true, min, gst::ClockTime::NONE);
                }

                true
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }

    fn build_state(&self) -> Result<State, Error> {
        let internal_bin = gst::Bin::with_name("internal");
        let transcription_bin = gst::Bin::with_name("transcription-bin");
        let cccombiner = gst::ElementFactory::make("cccombiner")
            .name("cccombiner")
            .build()?;
        let video_queue = gst::ElementFactory::make("queue").build()?;
        let cccapsfilter = gst::ElementFactory::make("capsfilter").build()?;
        let transcription_valve = gst::ElementFactory::make("valve")
            .property_from_str("drop-mode", "transform-to-gap")
            .build()?;

        let settings = self.settings.lock().unwrap();
        let mux_method = settings.mux_method;

        let has_force_live = |factory_name: &str| {
            gst::ElementFactory::find(factory_name)
                .and_then(|f| f.load().ok())
                .map(|f| f.element_type())
                .and_then(glib::Class::<gst::Element>::from_type)
                .map(|k| k.has_property_with_type("force-live", bool::static_type()))
                .unwrap_or(false)
        };

        let ccmux = match mux_method {
            MuxMethod::Cea608 => gst::ElementFactory::make("cea608mux")
                .property_from_str("start-time-selection", "first")
                .property_if("force-live", true, has_force_live("cea608mux"))
                .build()?,
            MuxMethod::Cea708 => gst::ElementFactory::make("cea708mux")
                .property_from_str("start-time-selection", "first")
                .property_if("force-live", true, has_force_live("cea708mux"))
                .build()?,
        };
        let ccmux_filter = gst::ElementFactory::make("capsfilter").build()?;

        let pad = self
            .audio_sinkpad
            .clone()
            .downcast::<super::TranscriberSinkPad>()
            .unwrap();
        let mut audio_sink_pads = HashMap::new();
        audio_sink_pads.insert(self.audio_sinkpad.name().to_string(), pad.clone());
        let mut ps = pad.imp().state.lock().unwrap();
        let pad_state = ps
            .as_mut()
            .map_err(|err| anyhow!("Sink pad state creation failed: {err}"))?;
        let pad_settings = pad.imp().settings.lock().unwrap();
        self.construct_channels(settings.mux_method, pad_state, &pad_settings)?;

        Ok(State {
            mux_method,
            framerate: None,
            internal_bin,
            video_queue,
            ccmux,
            ccmux_filter,
            cccombiner,
            transcription_bin,
            cccapsfilter,
            transcription_valve,
            audio_serial: 0,
            audio_sink_pads,
        })
    }

    #[allow(clippy::single_match)]
    fn video_sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::trace!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(e) => {
                let mut state = self.state.lock().unwrap();

                if let Some(ref mut state) = state.as_mut() {
                    let caps = e.caps();
                    let s = caps.structure(0).unwrap();

                    let had_framerate = state.framerate.is_some();

                    if let Ok(framerate) = s.get::<gst::Fraction>("framerate") {
                        state.framerate = Some(framerate);
                    } else {
                        state.framerate = Some(gst::Fraction::new(30, 1));
                    }

                    if !had_framerate {
                        gst::info!(
                            CAT,
                            imp = self,
                            "Received video caps, setting up transcription"
                        );
                        self.setup_transcription(state);
                    }
                }

                drop(state);

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

impl ChildProxyImpl for TranscriberBin {
    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let parent_children_count = self.parent_children_count();

        if index < parent_children_count {
            self.parent_child_by_index(index)
        } else {
            self.obj()
                .pads()
                .into_iter()
                .nth((index - parent_children_count) as usize)
                .map(|p| p.upcast())
        }
    }

    fn children_count(&self) -> u32 {
        let object = self.obj();
        self.parent_children_count() + object.num_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        if let Some(child) = self.parent_child_by_name(name) {
            Some(child)
        } else {
            self.obj().static_pad(name).map(|pad| pad.upcast())
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TranscriberBin {
    const NAME: &'static str = "GstTranscriberBin";
    type Type = super::TranscriberBin;
    type ParentType = gst::Bin;
    type Interfaces = (gst::ChildProxy,);

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink_audio").unwrap();
        let audio_sinkpad =
            gst::PadBuilder::<super::TranscriberSinkPad>::from_template(&templ).build();
        let templ = klass.pad_template("src_audio").unwrap();
        let audio_srcpad = gst::GhostPad::builder_from_template(&templ)
            .query_function(|pad, parent, query| {
                TranscriberBin::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.src_query(pad.upcast_ref(), query),
                )
            })
            .build();

        let templ = klass.pad_template("sink_video").unwrap();
        let video_sinkpad = gst::GhostPad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                TranscriberBin::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.video_sink_event(pad.upcast_ref(), event),
                )
            })
            .build();
        let templ = klass.pad_template("src_video").unwrap();
        let video_srcpad = gst::GhostPad::builder_from_template(&templ)
            .query_function(|pad, parent, query| {
                TranscriberBin::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.src_query(pad.upcast_ref(), query),
                )
            })
            .build();

        Self {
            audio_srcpad,
            video_srcpad,
            audio_sinkpad: audio_sinkpad.into(),
            video_sinkpad,
            state: Mutex::new(None),
            settings: Mutex::new(Settings::default()),
        }
    }
}

impl ObjectImpl for TranscriberBin {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow the transcriber")
                    .default_value(DEFAULT_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Lateness")
                    .blurb("Amount of milliseconds to pass as lateness to the transcriber")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("accumulate-time")
                    .nick("accumulate-time")
                    .blurb("Cut-off time for textwrap accumulation, in milliseconds (0=do not accumulate). \
                    Set this to a non-default value if you plan to switch to pop-on mode")
                    .default_value(DEFAULT_ACCUMULATE.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("cc-caps")
                    .nick("Closed Caption caps")
                    .blurb("The expected format of the closed captions")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("caption-source", DEFAULT_CAPTION_SOURCE)
                    .nick("Caption source")
                    .blurb("Caption source to use. \
                    If \"Transcription\" or \"Inband\" is selected, the caption meta \
                    of the other source will be dropped by transcriberbin")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("translate-latency")
                    .nick("Translation Latency")
                    .blurb("Amount of extra milliseconds to allow for translating")
                    .default_value(DEFAULT_TRANSLATE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder("mux-method")
                    .nick("Mux Method")
                    .blurb("The method for muxing multiple transcription streams")
                    .default_value(DEFAULT_MUX_METHOD)
                    .construct()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "latency" => {
                let state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );

                if let Some(state) = state.as_ref() {
                    for pad in state.audio_sink_pads.values() {
                        let ps = pad.imp().state.lock().unwrap();
                        let pad_state = ps.as_ref().unwrap();

                        if let Some(ref transcriber) = pad_state.transcriber {
                            let latency_ms = settings.latency.mseconds() as u32;

                            if transcriber
                                .has_property_with_type("transcribe-latency", u32::static_type())
                            {
                                transcriber.set_property("transcribe-latency", latency_ms);
                            } else if transcriber
                                .has_property_with_type("latency", u32::static_type())
                            {
                                transcriber.set_property("latency", latency_ms);
                            }
                        }
                    }
                }
            }
            "lateness" => {
                let state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );

                if let Some(state) = state.as_ref() {
                    for pad in state.audio_sink_pads.values() {
                        let ps = pad.imp().state.lock().unwrap();
                        let pad_state = ps.as_ref().unwrap();

                        if let Some(ref transcriber) = pad_state.transcriber {
                            if transcriber.has_property_with_type("lateness", u32::static_type()) {
                                let lateness_ms = settings.lateness.mseconds() as u32;
                                transcriber.set_property("lateness", lateness_ms);
                            }
                        }
                    }
                }
            }
            "accumulate-time" => {
                let mut settings = self.settings.lock().unwrap();
                settings.accumulate_time = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "mode" => {
                self.audio_sinkpad.set_property(
                    "mode",
                    value.get::<Cea608Mode>().expect("type checked upstream"),
                );
            }
            "cc-caps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cc_caps = value.get().expect("type checked upstream");
            }
            "caption-source" => {
                let s = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.caption_source = value.get().expect("type checked upstream");

                if let Some(state) = s.as_ref() {
                    if state.cccombiner.has_property("input-meta-processing") {
                        match settings.caption_source {
                            CaptionSource::Inband => {
                                state
                                    .cccombiner
                                    .set_property_from_str("input-meta-processing", "force");
                            }
                            CaptionSource::Both => {
                                state
                                    .cccombiner
                                    .set_property_from_str("input-meta-processing", "append");
                            }
                            CaptionSource::Transcription => {
                                state
                                    .cccombiner
                                    .set_property_from_str("input-meta-processing", "drop");
                            }
                        }
                    } else if settings.caption_source == CaptionSource::Inband {
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Use inband caption, dropping transcription"
                        );
                        state.transcription_valve.set_property("drop", true);
                    } else {
                        gst::debug!(CAT, imp = self, "Stop dropping transcription");
                        state.transcription_valve.set_property("drop", false);
                    }
                }
            }
            "translate-latency" => {
                let state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.translate_latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
                if let Some(state) = state.as_ref() {
                    for pad in state.audio_sink_pads.values() {
                        let ps = pad.imp().state.lock().unwrap();
                        let pad_state = ps.as_ref().unwrap();

                        if let Some(ref transcriber) = pad_state.transcriber {
                            if transcriber
                                .has_property_with_type("translate-latency", u32::static_type())
                            {
                                let translate_latency_ms =
                                    settings.translate_latency.mseconds() as u32;
                                transcriber.set_property("translate-latency", translate_latency_ms);
                            }
                        }
                    }
                }
            }
            "mux-method" => {
                let mut settings = self.settings.lock().unwrap();
                settings.mux_method = value.get().expect("type checked upstream")
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "latency" => {
                let settings = self.settings.lock().unwrap();
                (settings.latency.mseconds() as u32).to_value()
            }
            "lateness" => {
                let settings = self.settings.lock().unwrap();
                (settings.lateness.mseconds() as u32).to_value()
            }
            "accumulate-time" => {
                let settings = self.settings.lock().unwrap();
                (settings.accumulate_time.mseconds() as u32).to_value()
            }
            "cc-caps" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_caps.to_value()
            }
            "caption-source" => {
                let settings = self.settings.lock().unwrap();
                settings.caption_source.to_value()
            }
            "translate-latency" => {
                let settings = self.settings.lock().unwrap();
                (settings.translate_latency.mseconds() as u32).to_value()
            }
            "mux-method" => {
                let settings = self.settings.lock().unwrap();
                settings.mux_method.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.audio_srcpad).unwrap();
        obj.add_pad(&self.audio_sinkpad).unwrap();
        obj.add_pad(&self.video_srcpad).unwrap();
        obj.add_pad(&self.video_sinkpad).unwrap();

        *self.state.lock().unwrap() = match self.build_state() {
            Ok(mut state) => match self.construct_internal_bin(&mut state) {
                Ok(()) => Some(state),
                Err(err) => {
                    gst::error!(CAT, "Failed to build internal bin: {}", err);
                    None
                }
            },
            Err(err) => {
                gst::error!(CAT, "Failed to build state: {}", err);
                None
            }
        }
    }
}

impl GstObjectImpl for TranscriberBin {}

impl ElementImpl for TranscriberBin {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "TranscriberBin",
                "Audio / Video / Text",
                "Transcribes audio and adds it as closed captions",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("video/x-raw").any_features().build();
            let video_src_pad_template = gst::PadTemplate::new(
                "src_video",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            let video_sink_pad_template = gst::PadTemplate::new(
                "sink_video",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("audio/x-raw").build();
            let audio_src_pad_template = gst::PadTemplate::new(
                "src_audio",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            let audio_sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_audio",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
                super::TranscriberSinkPad::static_type(),
            )
            .unwrap();
            let secondary_audio_sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_audio_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
                super::TranscriberSinkPad::static_type(),
            )
            .unwrap();
            let secondary_audio_src_pad_template = gst::PadTemplate::with_gtype(
                "src_audio_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &caps,
                super::TranscriberSrcPad::static_type(),
            )
            .unwrap();

            let src_caps = gst::Caps::builder("application/x-json").build();
            let unsynced_src_pad_template = gst::PadTemplate::new(
                "unsynced_src",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();
            let unsynced_translate_src_pad_template = gst::PadTemplate::new(
                "unsynced_translate_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();
            let unsynced_secondary_src_pad_template = gst::PadTemplate::new(
                "unsynced_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();
            let unsynced_secondary_translate_src_pad_template = gst::PadTemplate::new(
                "unsynced_translate_src_%u_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("text/x-raw").build();
            let subtitle_src_pad_template = gst::PadTemplate::new(
                "src_subtitle_%s",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();
            let secondary_subtitle_src_pad_template = gst::PadTemplate::new(
                "src_subtitle_%s_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("audio/x-raw").build();
            let synthesis_src_pad_template = gst::PadTemplate::new(
                "src_synthesis_%s",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();
            let secondary_synthesis_src_pad_template = gst::PadTemplate::new(
                "src_synthesis_%s_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();

            vec![
                video_src_pad_template,
                video_sink_pad_template,
                audio_src_pad_template,
                audio_sink_pad_template,
                secondary_audio_sink_pad_template,
                secondary_audio_src_pad_template,
                unsynced_src_pad_template,
                unsynced_translate_src_pad_template,
                unsynced_secondary_src_pad_template,
                unsynced_secondary_translate_src_pad_template,
                subtitle_src_pad_template,
                secondary_subtitle_src_pad_template,
                synthesis_src_pad_template,
                secondary_synthesis_src_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn release_pad(&self, pad: &gst::Pad) {
        if self.obj().current_state() > gst::State::Null {
            gst::fixme!(
                CAT,
                obj = pad,
                "releasing secondary audio stream while PLAYING is untested"
            );
        }

        // In practice we will probably at least need some flushing here,
        // and latency recalculating, but at least the basic skeleton for
        // releasing is in place

        let Some(pad) = pad.downcast_ref::<super::TranscriberSinkPad>() else {
            gst::error!(CAT, imp = self, "not a transcriber sink pad: {pad:?}");
            return;
        };

        let mut s = self.state.lock().unwrap();
        let ps = pad.imp().state.lock().unwrap();
        let pad_state = ps.as_ref().unwrap();

        pad_state.transcription_bin.set_locked_state(true);
        let _ = pad_state.transcription_bin.set_state(gst::State::Null);

        if let Some(ref mut state) = s.as_mut() {
            for channel in pad_state.caption_channels.values() {
                if let Some(srcpad) = pad_state
                    .transcription_bin
                    .static_pad(&format!("src_{}", channel.language))
                {
                    if let Some(peer) = srcpad.peer() {
                        let _ = state.ccmux.remove_pad(&peer);
                    }
                }
            }

            let _ = state.transcription_bin.remove(&pad_state.transcription_bin);
            for srcpad in pad_state.audio_tee.iterate_src_pads() {
                let srcpad = srcpad.unwrap();
                if let Some(peer) = srcpad.peer() {
                    if let Some(parent) = peer.parent() {
                        if parent == state.transcription_bin {
                            let _ = parent.downcast::<gst::Element>().unwrap().remove_pad(&peer);
                        }
                    }
                }
            }
            let _ = state.internal_bin.remove_many([
                &pad_state.clocksync,
                &pad_state.identity,
                &pad_state.audio_tee,
            ]);
            state.audio_sink_pads.remove(pad.name().as_str());
        }

        let srcpad = pad_state
            .srcpad_name
            .as_ref()
            .and_then(|name| self.obj().static_pad(name));

        drop(ps);
        drop(s);

        let _ = pad.set_active(false);
        let _ = self.obj().remove_pad(pad);

        if let Some(srcpad) = srcpad {
            let _ = srcpad.set_active(false);
            let _ = self.obj().remove_pad(&srcpad);
        }

        self.obj()
            .child_removed(pad.upcast_ref::<gst::Object>(), &pad.name());
    }

    fn request_new_pad(
        &self,
        _templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let element = self.obj();
        if element.current_state() > gst::State::Null {
            gst::error!(CAT, "element pads can only be requested before starting");
            return None;
        }

        let mut s = self.state.lock().unwrap();
        if let Some(ref mut state) = s.as_mut() {
            let name = format!("sink_audio_{}", state.audio_serial);

            let templ = self.obj().pad_template("sink_audio_%u").unwrap();
            let sink_pad = gst::PadBuilder::<super::TranscriberSinkPad>::from_template(&templ)
                .name(&name)
                .build();

            let src_pad = {
                let settings = self.settings.lock().unwrap();
                let mut s = sink_pad.imp().state.lock().unwrap();
                let pad_state = match s.as_mut() {
                    Ok(s) => s,
                    Err(e) => {
                        gst::error!(CAT, "Failed to construct sink pad: {e}");
                        return None;
                    }
                };
                let pad_settings = sink_pad.imp().settings.lock().unwrap();
                self.construct_channels(settings.mux_method, pad_state, &pad_settings)
                    .unwrap();

                pad_state.serial = Some(state.audio_serial);

                if let Err(e) = self.link_input_audio_stream(&name, pad_state, state, &pad_settings)
                {
                    gst::error!(CAT, "Failed to link secondary audio stream: {e}");
                    return None;
                }

                let internal_sink_pad = gst::GhostPad::builder_with_target(
                    &pad_state.clocksync.static_pad("sink").unwrap(),
                )
                .unwrap()
                .name(name.clone())
                .build();
                internal_sink_pad.set_active(true).unwrap();
                state.internal_bin.add_pad(&internal_sink_pad).unwrap();
                sink_pad.set_target(Some(&internal_sink_pad)).unwrap();
                sink_pad.set_active(true).unwrap();
                state
                    .audio_sink_pads
                    .insert(name.to_string(), sink_pad.clone());

                let templ = self.obj().pad_template("src_audio_%u").unwrap();
                let name = format!("src_audio_{}", state.audio_serial);
                let src_pad = gst::PadBuilder::<super::TranscriberSrcPad>::from_template(&templ)
                    .name(name.as_str())
                    .query_function(|pad, parent, query| {
                        TranscriberBin::catch_panic_pad_function(
                            parent,
                            || false,
                            |transcriber| transcriber.src_query(pad.upcast_ref(), query),
                        )
                    })
                    .build();
                let internal_src_pad = gst::GhostPad::builder_with_target(
                    &pad_state.queue_passthrough.static_pad("src").unwrap(),
                )
                .unwrap()
                .name(&name)
                .build();
                internal_src_pad.set_active(true).unwrap();
                state.internal_bin.add_pad(&internal_src_pad).unwrap();
                src_pad.set_target(Some(&internal_src_pad)).unwrap();
                src_pad.set_active(true).unwrap();

                pad_state.srcpad_name = Some(name.clone());

                src_pad
            };

            state.audio_serial += 1;

            drop(s);

            self.obj().add_pad(&sink_pad).unwrap();
            self.obj()
                .child_added(sink_pad.upcast_ref::<gst::Object>(), &sink_pad.name());
            self.obj().add_pad(&src_pad).unwrap();

            Some(sink_pad.upcast())
        } else {
            None
        }
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.lock().unwrap();

                if let Some(ref mut state) = state.as_mut() {
                    if state.framerate.is_some() {
                        gst::info!(
                            CAT,
                            imp = self,
                            "Received video caps, setting up transcription"
                        );
                        self.setup_transcription(state);
                    }
                } else {
                    drop(state);
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Failed,
                        ["Can't change state with no state"]
                    );
                    return Err(gst::StateChangeError);
                }
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}

impl BinImpl for TranscriberBin {
    fn handle_message(&self, msg: gst::Message) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Error(m) => {
                let mut s = self.state.lock().unwrap();
                let Some(state) = s.as_mut() else {
                    drop(s);
                    self.parent_handle_message(msg);
                    return;
                };
                let mut handled = false;
                for pad in state.audio_sink_pads.values() {
                    let mut ps = pad.imp().state.lock().unwrap();
                    let Ok(pad_state) = ps.as_mut() else {
                        continue;
                    };

                    if msg.src() == pad_state.transcriber.as_ref().map(|t| t.upcast_ref())
                        || msg
                            .src()
                            .zip(pad_state.transcriber.clone())
                            .map(|(src, transcriber)| src.has_ancestor(&transcriber))
                            .unwrap_or(false)
                    {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Transcriber has posted an error ({m:?}), going back to passthrough",
                        );
                        pad_state.target_passthrough_state = TargetPassthroughState::Enabled;
                        let pad_weak = pad.downgrade();
                        self.obj().call_async(move |bin| {
                            let Some(pad) = pad_weak.upgrade() else {
                                return;
                            };
                            let thiz = bin.imp();
                            let s = thiz.state.lock().unwrap();
                            let ps = pad.imp().state.lock().unwrap();
                            thiz.block_and_update(pad.imp(), s, ps);
                        });
                        handled = true;
                        break;
                    }
                }

                if !handled {
                    drop(s);
                    self.parent_handle_message(msg);
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}

#[derive(Debug, Clone)]
struct TranscriberSinkPadSettings {
    translation_languages: Option<gst::Structure>,
    synthesis_languages: Option<gst::Structure>,
    subtitle_languages: Option<gst::Structure>,
    language_filters: Option<gst::Structure>,
    language_code: String,
    mode: Cea608Mode,
    passthrough: bool,
}

impl Default for TranscriberSinkPadSettings {
    fn default() -> Self {
        Self {
            translation_languages: None,
            synthesis_languages: None,
            subtitle_languages: None,
            language_filters: None,
            language_code: String::from(DEFAULT_INPUT_LANG_CODE),
            mode: DEFAULT_MODE,
            passthrough: DEFAULT_PASSTHROUGH,
        }
    }
}

struct TranscriberSinkPadState {
    clocksync: gst::Element,
    identity: gst::Element,
    audio_tee: gst::Element,
    transcription_bin: gst::Bin,
    transcriber_aconv: gst::Element,
    transcriber_resample: gst::Element,
    transcriber: Option<gst::Element>,
    queue_passthrough: gst::Element,
    caption_channels: HashMap<String, CaptionChannel>,
    synthesis_channels: HashMap<String, CustomOutputChannel>,
    subtitle_channels: HashMap<String, CustomOutputChannel>,
    srcpad_name: Option<String>,
    passthrough_state: PassthroughState,
    target_passthrough_state: TargetPassthroughState,
    serial: Option<u32>,
    language_tees: HashMap<String, gst::Element>,
    language_filters: HashMap<String, gst::Element>,
}

impl TranscriberSinkPad {
    fn uses_translation_bin(&self) -> bool {
        let s = self.state.lock().unwrap();
        let state = s.as_ref().unwrap();

        state
            .transcriber
            .as_ref()
            .and_then(|t| t.factory())
            .map(|f| f.name() == "translationbin")
            .unwrap_or(false)
    }
}

impl TranscriberSinkPadState {
    fn try_new() -> Result<Self, Error> {
        Ok(Self {
            clocksync: gst::ElementFactory::make("clocksync").build()?,
            identity: gst::ElementFactory::make("identity")
                // We need to do that otherwise downstream may block for up to
                // latency long until the allocation makes it through all branches.
                // Audio buffer pools are fortunately not the most critical :)
                .property("drop-allocation", true)
                .build()?,
            audio_tee: gst::ElementFactory::make("tee")
                // Protect passthrough enable (and resulting dynamic reconfigure)
                // from non-streaming thread
                .property("allow-not-linked", true)
                .build()?,
            transcription_bin: gst::Bin::new(),
            transcriber_resample: gst::ElementFactory::make("audioresample").build()?,
            transcriber_aconv: gst::ElementFactory::make("audioconvert").build()?,
            transcriber: gst::ElementFactory::make("awstranscriber")
                .name("transcriber")
                .build()
                .ok(),
            queue_passthrough: gst::ElementFactory::make("queue").build()?,
            caption_channels: HashMap::new(),
            synthesis_channels: HashMap::new(),
            subtitle_channels: HashMap::new(),
            srcpad_name: None,
            passthrough_state: PassthroughState::Disabled,
            target_passthrough_state: TargetPassthroughState::None,
            serial: None,
            language_tees: HashMap::new(),
            language_filters: HashMap::new(),
        })
    }

    fn remove_unsynced_pads(
        &self,
        topbin: &super::TranscriberBin,
        state: &State,
        srcpad_name: &str,
    ) {
        let Some(ref transcriber) = self.transcriber else {
            return;
        };

        let mut unsynced_pad_name = format!("unsynced_{srcpad_name}");

        if transcriber.static_pad(&unsynced_pad_name).is_some() {
            let srcpad = self
                .transcription_bin
                .static_pad(&unsynced_pad_name)
                .unwrap();
            let _ = self.transcription_bin.remove_pad(&srcpad);
            if let Some(serial) = self.serial {
                unsynced_pad_name = format!("{unsynced_pad_name}_{serial}");
            }
            let srcpad = state
                .transcription_bin
                .static_pad(&unsynced_pad_name)
                .unwrap();
            let _ = state.transcription_bin.remove_pad(&srcpad);

            let srcpad = state.internal_bin.static_pad(&unsynced_pad_name).unwrap();
            let _ = state.internal_bin.remove_pad(&srcpad);
            let srcpad = topbin.static_pad(&unsynced_pad_name).unwrap();
            let _ = topbin.remove_pad(&srcpad);
        }
    }

    fn unlink_language_tees(
        &self,
        topbin: &super::TranscriberBin,
        state: &State,
        pad_state: &TranscriberSinkPadState,
    ) {
        for (language, tee) in pad_state.language_tees.iter() {
            let sinkpad = if let Some(filter) = pad_state.language_filters.get(language) {
                filter.static_pad("sink").unwrap()
            } else {
                tee.static_pad("sink").unwrap()
            };

            if let Some(srcpad) = sinkpad.peer() {
                let _ = srcpad.unlink(&sinkpad);
                self.remove_unsynced_pads(topbin, state, srcpad.name().as_str());
                if srcpad.name() != "src" {
                    srcpad
                        .parent()
                        .unwrap()
                        .downcast::<gst::Element>()
                        .unwrap()
                        .release_request_pad(&srcpad);
                }
            }

            for srcpad in tee.iterate_src_pads() {
                let Ok(srcpad) = srcpad else {
                    continue;
                };

                if let Some(sinkpad) = srcpad.peer() {
                    let _ = srcpad.unlink(&sinkpad);
                }
            }
        }
    }

    fn expose_unsynced_pads(
        &self,
        topbin: &super::TranscriberBin,
        state: &State,
        srcpad_name: &str,
    ) -> Result<(), Error> {
        let Some(ref transcriber) = self.transcriber else {
            return Ok(());
        };

        let mut unsynced_pad_name = format!("unsynced_{srcpad_name}");

        if let Some(unsynced_pad) = transcriber.static_pad(&unsynced_pad_name) {
            let srcpad = gst::GhostPad::builder_with_target(&unsynced_pad)
                .unwrap()
                .name(unsynced_pad_name.clone())
                .build();
            self.transcription_bin.add_pad(&srcpad)?;

            if let Some(serial) = self.serial {
                unsynced_pad_name = format!("{unsynced_pad_name}_{serial}");
            }

            let srcpad = gst::GhostPad::builder_with_target(&srcpad)
                .unwrap()
                .name(unsynced_pad_name.clone())
                .build();
            state.transcription_bin.add_pad(&srcpad)?;

            let srcpad = gst::GhostPad::with_target(&srcpad).unwrap();
            state.internal_bin.add_pad(&srcpad)?;

            let srcpad = gst::GhostPad::with_target(&srcpad).unwrap();

            srcpad.set_active(true).unwrap();

            unsynced_pad.sticky_events_foreach(|event| {
                if event.type_() == gst::EventType::Tag
                    || event.type_() == gst::EventType::StreamStart
                {
                    gst::debug!(
                        CAT,
                        obj = srcpad,
                        "Storing {event:?} on unsynced source pad"
                    );
                    let _ = srcpad.store_sticky_event(event);
                }
                std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
            });

            topbin.add_pad(&srcpad)?;
        }

        Ok(())
    }

    fn link_transcription_channel(
        &self,
        channel: &CaptionChannel,
        state: &State,
        passthrough: bool,
    ) -> Result<(), Error> {
        let srcpad = gst::GhostPad::builder_with_target(&channel.bin.static_pad("src").unwrap())
            .unwrap()
            .name(format!("src_{}", channel.language))
            .build();

        self.transcription_bin.add_pad(&srcpad)?;

        if state.ccmux.static_pad(&channel.ccmux_pad_name).is_none() && !passthrough {
            let ccmux_pad = state
                .ccmux
                .request_pad_simple(&channel.ccmux_pad_name)
                .ok_or(anyhow!("Failed to request ccmux sink pad"))?;
            srcpad.link(&ccmux_pad)?;
        }

        Ok(())
    }
}

pub struct TranscriberSinkPad {
    state: Mutex<Result<TranscriberSinkPadState, Error>>,
    settings: Mutex<TranscriberSinkPadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for TranscriberSinkPad {
    const NAME: &'static str = "GstTranscriberSinkPad";
    type Type = super::TranscriberSinkPad;
    type ParentType = gst::GhostPad;

    fn new() -> Self {
        Self {
            state: Mutex::new(TranscriberSinkPadState::try_new()),
            settings: Mutex::new(TranscriberSinkPadSettings::default()),
        }
    }
}

impl ObjectImpl for TranscriberSinkPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("passthrough")
                    .nick("Passthrough")
                    .blurb("Whether transcription should occur")
                    .default_value(DEFAULT_PASSTHROUGH)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("translation-languages")
                    .nick("Translation languages")
                    .blurb("A map of language codes to caption channels, e.g. translation-languages=\"languages, transcript={CC1, 708_1}, fr={708_2, CC3}\" will map the French translation to CC1/service 1 and the original transcript to CC3/service 2")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("The language of the input stream")
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_playing()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("mode", DEFAULT_MODE)
                    .nick("Mode")
                    .blurb("Which closed caption mode to operate in")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecObject::builder::<gst::Element>("transcriber")
                    .nick("Transcriber")
                    .blurb("The transcriber element to use")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("synthesis-languages")
                    .nick("Synthesis languages")
                    .blurb("A map of language codes to bin descriptions, e.g. synthesis-languages=\"languages, fr=awspolly\" will use the awspolly element to synthesize speech from French translations")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("subtitle-languages")
                    .nick("Subtitle languages")
                    .blurb("A map of language codes to bin descriptions, e.g. subtitle-languages=\"languages, fr=textwrap lines=2 accumulate-time=5000000000\" will use the textwrap element before outputting the subtitles")
                    .mutable_playing()
                    .build(),
                gst::ParamSpecArray::builder("transcription-mix-matrix")
                    .nick("Transcription mix matrix")
                    .blurb("Initial transformation matrix for the transcriber audioconvert")
                    .element_spec(
                        &gst::ParamSpecArray::builder("rows")
                            .nick("Rows")
                            .blurb("A row in the matrix")
                            .element_spec(
                                &glib::ParamSpecFloat::builder("columns")
                                .nick("Columns")
                                .blurb("A column in the matrix")
                                .minimum(-1.)
                                .maximum(1.)
                                .default_value(0.)
                                .build()
                            )
                            .build(),
                    )
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("language-filters")
                    .nick("Language filters")
                    .blurb("A map of language codes to bin descriptions, e.g. text-filters=\"languages, fr=regex\" will filter words out of the transcriber through the regex element")
                    .mutable_playing()
                    .build(),
        ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "passthrough" => {
                let Some(parent) = self.obj().parent().and_downcast::<super::TranscriberBin>()
                else {
                    return;
                };
                let s = parent.imp().state.lock().unwrap();
                let mut ps = self.state.lock().unwrap();
                let mut pad_settings = self.settings.lock().unwrap();
                pad_settings.passthrough = value.get().expect("type checked upstream");

                let Ok(ref mut pad_state) = ps.as_mut() else {
                    return;
                };

                if pad_settings.passthrough {
                    pad_state.target_passthrough_state = TargetPassthroughState::Enabled;
                } else {
                    pad_state.target_passthrough_state = TargetPassthroughState::Disabled;
                }
                drop(pad_settings);

                gst::debug!(
                    CAT,
                    imp = self,
                    "target passthrough state: {:?}",
                    pad_state.target_passthrough_state
                );

                parent.imp().block_and_update(self, s, ps);
            }
            "translation-languages" => {
                let mut settings = self.settings.lock().unwrap();
                settings.translation_languages = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Updated translation-languages {:?}",
                    settings.translation_languages
                );

                drop(settings);

                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    if self.uses_translation_bin() {
                        if let Err(e) = this.imp().reconfigure_transcription_bin_dynamic(self) {
                            gst::error!(CAT, "Couldn't reconfigure caption channels: {e}");
                            gst::element_imp_error!(
                                this.imp(),
                                gst::StreamError::Failed,
                                ["Couldn't reconfigure caption channels: {}", e]
                            );
                            *this.imp().state.lock().unwrap() = None;
                        }
                    } else {
                        this.imp().update_languages(&self.obj(), false)
                    }
                }
            }
            "synthesis-languages" => {
                let mut settings = self.settings.lock().unwrap();
                settings.synthesis_languages = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Updated synthesis-languages {:?}",
                    settings.synthesis_languages
                );

                drop(settings);

                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    if self.uses_translation_bin() {
                        if let Err(e) = this.imp().reconfigure_transcription_bin_dynamic(self) {
                            gst::error!(CAT, "Couldn't reconfigure synthesis channels: {e}");
                            gst::element_imp_error!(
                                this.imp(),
                                gst::StreamError::Failed,
                                ["Couldn't reconfigure synthesis channels: {}", e]
                            );
                            *this.imp().state.lock().unwrap() = None;
                        }
                    } else {
                        this.imp().update_languages(&self.obj(), false)
                    }
                }
            }
            "subtitle-languages" => {
                let mut settings = self.settings.lock().unwrap();
                settings.subtitle_languages = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Updated subtitle-languages {:?}",
                    settings.subtitle_languages
                );

                drop(settings);

                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    if self.uses_translation_bin() {
                        if let Err(e) = this.imp().reconfigure_transcription_bin_dynamic(self) {
                            gst::error!(CAT, "Couldn't reconfigure subtitle channels: {e}");
                            gst::element_imp_error!(
                                this.imp(),
                                gst::StreamError::Failed,
                                ["Couldn't reconfigure subtitle channels: {}", e]
                            );
                            *this.imp().state.lock().unwrap() = None;
                        }
                    } else {
                        this.imp().update_languages(&self.obj(), false)
                    }
                }
            }
            "language-filters" => {
                let mut settings = self.settings.lock().unwrap();
                settings.language_filters = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream");
                gst::debug!(
                    CAT,
                    imp = self,
                    "Updated language-filters {:?}",
                    settings.language_filters
                );

                drop(settings);

                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    this.imp().update_languages(&self.obj(), false);
                }
            }
            "mode" => {
                let Some(parent) = self.obj().parent().and_downcast::<super::TranscriberBin>()
                else {
                    return;
                };
                let accumulate_time = parent.imp().settings.lock().unwrap().accumulate_time;
                let mut settings = self.settings.lock().unwrap();

                let old_mode = settings.mode;
                let new_mode = value.get().expect("type checked upstream");
                settings.mode = new_mode;

                if old_mode != new_mode {
                    drop(settings);
                    if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>()
                    {
                        if let Some(state) = this.imp().state.lock().unwrap().as_ref() {
                            let ps = self.state.lock().unwrap();
                            let pad_state = ps.as_ref().unwrap();
                            this.imp().setup_cc_mode(
                                &self.obj(),
                                pad_state,
                                state.mux_method,
                                new_mode,
                                accumulate_time,
                            );
                        }
                    }
                }
            }
            "language-code" => {
                let mut settings = self.settings.lock().unwrap();

                let old_code = settings.language_code.clone();
                let new_code = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| String::from(DEFAULT_INPUT_LANG_CODE));
                settings.language_code.clone_from(&new_code);
                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    drop(settings);
                    if new_code != old_code {
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Updating language code {old_code} -> {new_code}",
                        );

                        this.imp().update_languages(&self.obj(), true)
                    }
                }
            }
            "transcriber" => {
                if let Some(this) = self.obj().parent().and_downcast::<super::TranscriberBin>() {
                    let new_transcriber: Option<gst::Element> =
                        value.get().expect("type checked upstream");

                    if let Some(ref transcriber) = new_transcriber {
                        this.imp().configure_transcriber(transcriber);
                    }

                    let mut s = this.imp().state.lock().unwrap();
                    let mut ps = self.state.lock().unwrap();
                    let Ok(pad_state) = ps.as_mut() else {
                        return;
                    };
                    let old_transcriber = pad_state.transcriber.clone();
                    pad_state.transcriber.clone_from(&new_transcriber);

                    if old_transcriber != new_transcriber {
                        if let Some(ref mut state) = s.as_mut() {
                            match this.imp().relink_transcriber(
                                state,
                                pad_state,
                                old_transcriber.as_ref(),
                            ) {
                                Ok(()) => (),
                                Err(err) => {
                                    gst::error!(CAT, "invalid transcriber: {err}");
                                    drop(s);
                                    *this.imp().state.lock().unwrap() = None;
                                }
                            }
                        }
                    }
                }
            }
            "transcription-mix-matrix" => {
                let mut ps = self.state.lock().unwrap();
                let Ok(pad_state) = ps.as_mut() else {
                    return;
                };
                pad_state
                    .transcriber_aconv
                    .set_property("mix-matrix", value);
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "passthrough" => {
                let settings = self.settings.lock().unwrap();
                settings.passthrough.to_value()
            }
            "translation-languages" => {
                let settings = self.settings.lock().unwrap();
                settings.translation_languages.to_value()
            }
            "synthesis-languages" => {
                let settings = self.settings.lock().unwrap();
                settings.synthesis_languages.to_value()
            }
            "subtitle-languages" => {
                let settings = self.settings.lock().unwrap();
                settings.subtitle_languages.to_value()
            }
            "language-filters" => {
                let settings = self.settings.lock().unwrap();
                settings.language_filters.to_value()
            }
            "language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.language_code.to_value()
            }
            "mode" => {
                let settings = self.settings.lock().unwrap();
                settings.mode.to_value()
            }
            "transcriber" => {
                let ps = self.state.lock().unwrap();
                match ps.as_ref() {
                    Ok(ps) => ps.transcriber.to_value(),
                    Err(_) => None::<gst::Element>.to_value(),
                }
            }
            "transcription-mix-matrix" => {
                let ps = self.state.lock().unwrap();
                match ps.as_ref() {
                    Ok(ps) => ps.transcriber_aconv.property_value("mix-matrix"),
                    Err(_) => {
                        let v: Vec<gst::Array> = vec![];
                        gst::Array::new(&v).to_value()
                    }
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TranscriberSinkPad {}

impl PadImpl for TranscriberSinkPad {}

impl ProxyPadImpl for TranscriberSinkPad {}

impl GhostPadImpl for TranscriberSinkPad {}

#[derive(Debug, Default)]
pub struct TranscriberSrcPad {}

#[glib::object_subclass]
impl ObjectSubclass for TranscriberSrcPad {
    const NAME: &'static str = "GstTranscriberSrcPad";
    type Type = super::TranscriberSrcPad;
    type ParentType = gst::GhostPad;

    fn new() -> Self {
        Default::default()
    }
}

impl ObjectImpl for TranscriberSrcPad {}

impl GstObjectImpl for TranscriberSrcPad {}

impl PadImpl for TranscriberSrcPad {}

impl ProxyPadImpl for TranscriberSrcPad {}

impl GhostPadImpl for TranscriberSrcPad {}
