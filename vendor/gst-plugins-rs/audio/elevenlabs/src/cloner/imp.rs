// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! ElevenLabs voice cloning element.
//!
//! This element sends audio samples to ElevenLabs for instant voice cloning.

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};
use tokio::sync::mpsc;

use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};

use crate::RUNTIME;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "elevenlabsvoicecloner",
        gst::DebugColorFlags::empty(),
        Some("ElevenLabs voice cloning element"),
    )
});

#[derive(serde::Deserialize, Debug)]
struct CreateVoiceResponse {
    voice_id: String,
}

const DEFAULT_API_KEY: Option<&str> = None;
const DEFAULT_SPEAKER: Option<&str> = None;
const DEFAULT_SEGMENT_DURATION: gst::ClockTime = gst::ClockTime::from_seconds(10);
const DEFAULT_REMOVE_BACKGROUND_NOISE: bool = true;

#[derive(Debug, Clone)]
pub(super) struct Settings {
    api_key: Option<String>,
    speaker: Option<String>,
    segment_duration: gst::ClockTime,
    remove_background_noise: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: DEFAULT_API_KEY.map(String::from),
            speaker: DEFAULT_SPEAKER.map(String::from),
            segment_duration: DEFAULT_SEGMENT_DURATION,
            remove_background_noise: DEFAULT_REMOVE_BACKGROUND_NOISE,
        }
    }
}

#[derive(Clone)]
struct Cursor(Arc<Mutex<std::io::Cursor<Vec<u8>>>>);

struct SpeakerTask {
    join_handle: tokio::task::JoinHandle<()>,
    tx: mpsc::UnboundedSender<(Vec<u8>, gst::ClockTime)>,
    cursor: Cursor,
    wav_writer: Option<hound::WavWriter<Cursor>>,
}

#[derive(Default)]
struct State {
    segment: Option<gst::FormattedSegment<gst::ClockTime>>,
    caps: Option<gst::Caps>,
    client: Option<Client>,
    samples: VecDeque<gst::Sample>,
    current_speaker: Option<String>,
    speaker_tasks: HashMap<String, SpeakerTask>,
}

impl std::io::Write for Cursor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl std::io::Seek for Cursor {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.lock().unwrap().seek(pos)
    }
}

// Locking order: state then settings
pub struct Cloner {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl Cloner {
    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::trace!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        match event.view() {
            FlushStart(_) => {
                gst::info!(CAT, imp = self, "Received flush start, disconnecting");
                let ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                self.disconnect();
                ret
            }
            Segment(e) => {
                let segment = match e.segment().clone().downcast::<gst::ClockTime>() {
                    Err(segment) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["Only Time segments supported, got {:?}", segment.format(),]
                        );
                        return false;
                    }
                    Ok(segment) => segment,
                };

                {
                    let mut state = self.state.lock().unwrap();

                    if let Some(mut old_segment) = state.segment.as_ref().cloned() {
                        old_segment.set_position(segment.position());
                        if old_segment != segment {
                            if let Some(current_speaker) = state.current_speaker.as_ref().cloned() {
                                if let Err(err) = self.drain_writer(&mut state, &current_speaker) {
                                    self.post_error_message(err);
                                    return false;
                                }
                            }
                        }
                    }

                    state.segment = Some(segment);
                }

                self.srcpad.push_event(event)
            }
            Caps(c) => {
                let caps = c.caps();

                {
                    let mut state = self.state.lock().unwrap();

                    if let Some(old_caps) = state.caps.as_ref() {
                        if old_caps != caps {
                            if let Some(current_speaker) = state.current_speaker.as_ref().cloned() {
                                if let Err(err) = self.drain_writer(&mut state, &current_speaker) {
                                    self.post_error_message(err);
                                    return false;
                                }
                            }
                        }
                    }

                    state.caps = Some(caps.to_owned());

                    gst::debug!(CAT, "stored caps {}", caps);
                }

                self.srcpad.push_event(event)
            }
            Eos(_) => {
                {
                    let mut state = self.state.lock().unwrap();

                    let speakers: Vec<_> =
                        state.speaker_tasks.keys().map(|s| s.to_owned()).collect();

                    for speaker in speakers {
                        if let Err(err) = self.drain_writer(&mut state, &speaker) {
                            self.post_error_message(err);
                            return false;
                        }
                    }

                    let tasks: Vec<_> = state.speaker_tasks.drain().map(|(_, task)| task).collect();

                    drop(state);

                    for task in tasks {
                        drop(task.tx);
                        let _ = RUNTIME.block_on(task.join_handle);
                    }
                }

                self.srcpad.push_event(event)
            }
            _ => self.srcpad.push_event(event),
        }
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::trace!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        if let CustomUpstream(c) = event.view() {
            let Some(s) = c.structure() else {
                return gst::Pad::event_default(pad, Some(&*self.obj()), event);
            };

            if s.name().as_str() == "rstranscribe/new-item" {
                if self.settings.lock().unwrap().speaker.is_some() {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Ignoring rstranscribe/new-item event in single speaker mode"
                    );
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                }

                let Ok(speaker) = s.get::<String>("speaker") else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Missing speaker field in rstranscribe/new-item event"
                    );
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                let Ok(item_rtime) = s.get::<gst::ClockTime>("running-time") else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Missing running-time field in rstranscribe/new-item event"
                    );
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                let Ok(item_duration) = s.get::<gst::ClockTime>("duration") else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Missing duration field in rstranscribe/new-item event"
                    );
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                let Ok(content) = s.get::<String>("content") else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Missing content field in rstranscribe/new-item event"
                    );
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                gst::debug!(CAT, imp = self, "received new item {content} for speaker {speaker} with running time {item_rtime} and duration {item_duration}");

                let mut state = self.state.lock().unwrap();

                let trim = state.current_speaker.is_none();

                if let Some(old_speaker) = state.current_speaker.as_ref().cloned() {
                    if speaker != *old_speaker {
                        if let Err(err) = self.maybe_drain_writer(&mut state, &old_speaker) {
                            self.post_error_message(err);
                            return false;
                        }
                    }
                }

                state.current_speaker = Some(speaker.clone());

                while let Some(sample) = state.samples.pop_front() {
                    // Safe unwraps, we always create full samples
                    let sample_buffer = sample.buffer().unwrap();
                    let segment = sample
                        .segment()
                        .unwrap()
                        .downcast_ref::<gst::ClockTime>()
                        .unwrap();
                    let sample_rtime = segment
                        .to_running_time(sample_buffer.pts().unwrap())
                        .unwrap();
                    let sample_duration = sample_buffer.duration().unwrap();

                    let write_sample = if sample_rtime + sample_duration < item_rtime {
                        !trim
                    } else if sample_rtime > item_rtime + item_duration {
                        state.samples.push_front(sample);
                        gst::log!(CAT, imp = self, "keeping sample with running time {sample_rtime} and duration {sample_duration}");
                        break;
                    } else {
                        true
                    };

                    if write_sample {
                        gst::log!(
                            CAT,
                            imp = self,
                            "writing sample with pts {:?} and duration {:?}",
                            sample_buffer.pts(),
                            sample_buffer.duration()
                        );

                        self.write_sample(&mut state, &sample, &speaker);
                    } else {
                        gst::log!(CAT, imp = self, "discarding sample with running time {sample_rtime} and duration {sample_duration}");
                    }
                }

                if let Err(err) = self.maybe_drain_writer(&mut state, &speaker) {
                    self.post_error_message(err);
                    return false;
                }
            };
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    fn drain_writer(&self, state: &mut State, speaker: &str) -> Result<(), gst::ErrorMessage> {
        let speaker_task = state
            .speaker_tasks
            .get_mut(speaker)
            .expect("speaker task was created");

        let Some(wav_writer) = speaker_task.wav_writer.take() else {
            return Ok(());
        };

        let wav_duration = gst::ClockTime::from_nseconds(
            (wav_writer.duration() as u64)
                .mul_div_round(
                    gst::ClockTime::SECOND.nseconds(),
                    wav_writer.spec().sample_rate as u64,
                )
                .unwrap(),
        );

        gst::debug!(
            CAT,
            imp = self,
            "flushing writer with duration {wav_duration} for speaker {speaker}"
        );

        if let Err(err) = wav_writer.finalize() {
            gst::error!(CAT, imp = self, "failed to finalize wav writer: {err}");
            return Err(gst::error_msg!(
                gst::StreamError::Failed,
                ["failed to finalize wav writer: {err}"]
            ));
        }

        let inner_cursor = speaker_task.cursor.0.lock().unwrap();
        let wav = inner_cursor.get_ref();

        gst::debug!(
            CAT,
            imp = self,
            "flushed writer for speaker {speaker}, cursor size: {}",
            wav.len()
        );

        let _ = speaker_task.tx.send((wav.to_owned(), wav_duration));

        drop(inner_cursor);

        speaker_task.cursor = Cursor(Arc::new(Mutex::new(std::io::Cursor::new(vec![]))));

        Ok(())
    }

    fn maybe_drain_writer(
        &self,
        state: &mut State,
        speaker: &str,
    ) -> Result<(), gst::ErrorMessage> {
        let segment_duration = self.settings.lock().unwrap().segment_duration;

        let speaker_task = state
            .speaker_tasks
            .get(speaker)
            .expect("speaker task was created");

        let Some(ref wav_writer) = speaker_task.wav_writer else {
            return Ok(());
        };

        let do_drain = {
            let wav_duration = gst::ClockTime::from_nseconds(
                (wav_writer.duration() as u64)
                    .mul_div_round(
                        gst::ClockTime::SECOND.nseconds(),
                        wav_writer.spec().sample_rate as u64,
                    )
                    .unwrap(),
            );
            let wav_length = wav_writer.len() * 2;

            // Maximum file size is 11 MB, let's leave some room for the WAV header too,
            // unfortunately the hound API doesn't seem to have a way to precalculate that
            // https://github.com/ruuda/hound/issues/97
            wav_duration > segment_duration || wav_length > 10_000_000
        };

        if do_drain {
            self.drain_writer(state, speaker)
        } else {
            Ok(())
        }
    }

    fn write_sample(&self, state: &mut State, sample: &gst::Sample, speaker: &str) {
        let sample_buffer = sample.buffer().unwrap();
        let caps = sample.caps().unwrap();
        let audio_info = gst_audio::AudioInfo::from_caps(caps).unwrap();
        let remove_background_noise = self.settings.lock().unwrap().remove_background_noise;

        let speaker_task = state
            .speaker_tasks
            .entry(speaker.to_string())
            .or_insert_with(|| {
                let this_weak = self.downgrade();
                let speaker = speaker.to_string();
                let (tx, mut rx) = mpsc::unbounded_channel();

                let join_handle = RUNTIME.spawn(async move {
                    let mut voice_id: Option<String> = None;

                    while let Some((wav_data, wav_duration)) = rx.recv().await {
                        let Some(this) = this_weak.upgrade() else {
                            break;
                        };

                        let Some(client) = this.state.lock().unwrap().client.as_ref().cloned()
                        else {
                            break;
                        };

                        let remove_background_noise =
                            remove_background_noise && wav_duration > 5 * gst::ClockTime::SECOND;

                        if let Some(ref voice_id) = voice_id {
                            gst::debug!(
                                CAT,
                                imp = this,
                                "Updating voice {voice_id} for speaker {}",
                                speaker
                            );
                            let url =
                                format!("https://api.elevenlabs.io/v1/voices/{voice_id}/edit");

                            let job = {
                                let mut form = reqwest::multipart::Form::new()
                                    .text("name", speaker.clone())
                                    .text(
                                        "remove_background_noise",
                                        remove_background_noise.to_string(),
                                    );
                                let part = reqwest::multipart::Part::bytes(wav_data)
                                    .file_name("foo.wav")
                                    .mime_str("audio/wav")
                                    .unwrap();
                                form = form.part("files", part);
                                client.post(url).multipart(form).send()
                            };

                            let resp = match job.await {
                                Ok(resp) => resp,
                                Err(err) => {
                                    gst::error!(CAT, imp = this, "Failed updating voice: {err:?}",);
                                    this.post_error_message(gst::error_msg!(
                                        gst::StreamError::Failed,
                                        ["Failed updating voice: {err}"]
                                    ));
                                    break;
                                }
                            };

                            let text = match resp.text().await {
                                Ok(text) => text,
                                Err(err) => {
                                    gst::error!(CAT, imp = this, "Failed updating voice: {err:?}",);
                                    this.post_error_message(gst::error_msg!(
                                        gst::StreamError::Failed,
                                        ["Failed updating voice: {err}"]
                                    ));
                                    break;
                                }
                            };

                            gst::debug!(
                                CAT,
                                imp = this,
                                "update response for speaker {speaker}: {:?}",
                                text
                            );
                        } else {
                            gst::debug!(CAT, imp = this, "Creating voice for speaker {}", speaker);

                            let url = "https://api.elevenlabs.io/v1/voices/add";

                            let job = {
                                let mut form = reqwest::multipart::Form::new()
                                    .text("name", speaker.clone())
                                    .text(
                                        "remove_background_noise",
                                        remove_background_noise.to_string(),
                                    );
                                let part = reqwest::multipart::Part::bytes(wav_data)
                                    .file_name("foo.wav")
                                    .mime_str("audio/wav")
                                    .unwrap();
                                form = form.part("files", part);
                                client.post(url).multipart(form).send()
                            };

                            let resp = match job.await {
                                Ok(resp) => resp,
                                Err(err) => {
                                    gst::error!(CAT, imp = this, "Failed creating voice: {err:?}",);
                                    this.post_error_message(gst::error_msg!(
                                        gst::StreamError::Failed,
                                        ["Failed creating voice: {err}"]
                                    ));
                                    break;
                                }
                            };

                            let text = match resp.text().await {
                                Ok(text) => text,
                                Err(err) => {
                                    gst::error!(CAT, imp = this, "Failed creating voice: {err:?}",);
                                    this.post_error_message(gst::error_msg!(
                                        gst::StreamError::Failed,
                                        ["Failed creating voice: {err}"]
                                    ));
                                    break;
                                }
                            };

                            gst::debug!(
                                CAT,
                                imp = this,
                                "create response for speaker {speaker}: {:?}",
                                text
                            );

                            let response: CreateVoiceResponse =
                                serde_json::from_str(&text).unwrap();

                            voice_id = Some(response.voice_id.clone());

                            let s = gst::Structure::builder("elevenlabs/speaker-voice")
                                .field("speaker", &speaker)
                                .field("voice-id", response.voice_id)
                                .build();

                            let message = gst::message::Application::builder(s.clone())
                                .src(&*this.obj())
                                .build();

                            this.post_message(message);

                            let event = gst::event::CustomDownstreamOob::builder(s).build();

                            let _ = this.srcpad.push_event(event);
                        }
                    }
                });

                let cursor = Cursor(Arc::new(Mutex::new(std::io::Cursor::new(vec![]))));

                SpeakerTask {
                    join_handle,
                    tx,
                    cursor,
                    wav_writer: None,
                }
            });

        let data = sample_buffer.map_readable().unwrap();

        let cursor = speaker_task.cursor.clone();

        let wav_writer = speaker_task.wav_writer.get_or_insert_with(move || {
            let spec = hound::WavSpec {
                channels: audio_info.channels() as u16,
                sample_rate: audio_info.rate(),
                bits_per_sample: audio_info.bps() as u16 * 8,
                sample_format: hound::SampleFormat::Int,
            };

            gst::log!(CAT, imp = self, "creating wav writer with spec {spec:?}");

            hound::WavWriter::new(cursor, spec).unwrap()
        });

        for sample in data.as_slice().chunks_exact(2) {
            wav_writer
                .write_sample(i16::from_le_bytes([sample[0], sample[1]]))
                .unwrap();
        }
    }

    fn store_sample(&self, sample: gst::Sample) -> Result<(), gst::ErrorMessage> {
        let single_speaker = self.settings.lock().unwrap().speaker.clone();

        let mut state = self.state.lock().unwrap();

        if let Some(single_speaker) = single_speaker {
            self.write_sample(&mut state, &sample, &single_speaker);
            self.maybe_drain_writer(&mut state, &single_speaker)
        } else {
            state.samples.push_back(sample);
            Ok(())
        }
    }

    fn create_sample(
        &self,
        buffer: &gst::Buffer,
    ) -> Result<Option<gst::Sample>, gst::ErrorMessage> {
        let state = self.state.lock().unwrap();

        let Some(caps) = state.caps.as_ref() else {
            gst::warning!(CAT, imp = self, "Dropping buffer before caps");
            return Ok(None);
        };

        let Some(segment) = state.segment.as_ref() else {
            gst::warning!(CAT, imp = self, "Dropping buffer before segment");
            return Ok(None);
        };

        let Some(pts) = buffer.pts() else {
            gst::error!(CAT, imp = self, "buffers with PTS required");
            return Err(gst::error_msg!(
                gst::StreamError::Failed,
                ["buffers with PTS required"]
            ));
        };

        if buffer.duration().is_none() {
            gst::error!(CAT, imp = self, "buffers with duration required");
            return Err(gst::error_msg!(
                gst::StreamError::Failed,
                ["buffers with duration required"]
            ));
        }

        if segment.to_running_time(pts).is_none() {
            gst::warning!(
                CAT,
                imp = self,
                "Dropping buffer with pts {pts} outside segment {segment:?}"
            );
            return Ok(None);
        }

        Ok(Some(
            gst::Sample::builder()
                .segment(segment)
                .caps(caps)
                .buffer(buffer)
                .build(),
        ))
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling {buffer:?}");

        let Some(sample) = (match self.create_sample(&buffer) {
            Ok(sample) => sample,
            Err(err) => {
                self.post_error_message(err);
                return Err(gst::FlowError::Error);
            }
        }) else {
            return Ok(gst::FlowSuccess::Ok);
        };

        if let Err(err) = self.store_sample(sample) {
            self.post_error_message(err);
            Err(gst::FlowError::Error)
        } else {
            self.srcpad.push(buffer)
        }
    }

    fn ensure_connection(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        if state.client.is_none() {
            let Some(api_key) = settings.api_key.clone() else {
                return Err(gst::error_msg!(
                    gst::CoreError::Failed,
                    ["An API key is required"]
                ));
            };

            let mut headers = HeaderMap::new();

            let api_key_header = match HeaderValue::from_str(&api_key) {
                Ok(header) => header,
                Err(err) => {
                    return Err(gst::error_msg!(
                        gst::CoreError::Failed,
                        ["A valid string is required for the API key: {err}"]
                    ));
                }
            };

            headers.insert("xi-api-key", api_key_header);
            headers.insert(
                "Content-Type",
                HeaderValue::from_static("multipart/form-data"),
            );

            state.client = Some(Client::builder().default_headers(headers).build().map_err(
                |err| gst::error_msg!(gst::CoreError::Failed, ["Failed to create client: {err}"]),
            )?);
        }
        Ok(())
    }

    fn disconnect(&self) {
        gst::info!(CAT, imp = self, "Disconnecting");
        let mut state = self.state.lock().unwrap();

        state.client = None;

        for (_, task) in state.speaker_tasks.drain() {
            task.join_handle.abort();
        }

        *state = State::default();

        gst::info!(CAT, imp = self, "Disconnected");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cloner {
    const NAME: &'static str = "GstElevenLabsCloner";
    type Type = super::Cloner;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Cloner::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cloner::catch_panic_pad_function(parent, || false, |imp| imp.sink_event(pad, event))
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .flags(gst::PadFlags::FIXED_CAPS)
            .flags(gst::PadFlags::PROXY_CAPS)
            .event_function(|pad, parent, event| {
                Cloner::catch_panic_pad_function(parent, || false, |imp| imp.src_event(pad, event))
            })
            .build();

        Self {
            srcpad,
            sinkpad,
            settings: Default::default(),
            state: Default::default(),
        }
    }
}

impl ObjectImpl for Cloner {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("api-key")
                    .nick("API Key")
                    .blurb("ElevenLabs API Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("speaker")
                    .nick("Speaker")
                    .blurb("Optional speaker name. When set the element will treat all audio as uttered by a single speaker \
                        and will not use rstranscribe/new-item events for diarization.")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("segment-duration")
                    .nick("Segment Duration")
                    .blurb("Amount of milliseconds to store before creating / updating voice")
                    .default_value(DEFAULT_SEGMENT_DURATION.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("remove-background-noise")
                    .nick("Remove Background Noise")
                    .blurb("Whether to ask elevenlabs to remove background noise")
                    .default_value(DEFAULT_REMOVE_BACKGROUND_NOISE)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "api-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.api_key = value.get().expect("type checked upstream");
            }
            "speaker" => {
                let mut settings = self.settings.lock().unwrap();
                settings.speaker = value.get().expect("type checked upstream");
            }
            "segment-duration" => {
                let mut settings = self.settings.lock().unwrap();
                settings.segment_duration = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "remove-background-noise" => {
                let mut settings = self.settings.lock().unwrap();
                settings.remove_background_noise = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "api-key" => {
                let settings = self.settings.lock().unwrap();
                settings.api_key.to_value()
            }
            "speaker" => {
                let settings = self.settings.lock().unwrap();
                settings.speaker.to_value()
            }
            "segment-duration" => {
                let settings = self.settings.lock().unwrap();
                (settings.segment_duration.mseconds() as u32).to_value()
            }
            "remove-background-noise" => {
                let settings = self.settings.lock().unwrap();
                settings.remove_background_noise.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Cloner {}

impl ElementImpl for Cloner {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Voice Cloner",
                "Audio",
                "ElevenLabs Voice Cloner",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst_audio::AudioCapsBuilder::new()
                .format(gst_audio::AudioFormat::S16le)
                .layout(gst_audio::AudioLayout::Interleaved)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_audio::AudioCapsBuilder::new()
                .format(gst_audio::AudioFormat::S16le)
                .layout(gst_audio::AudioLayout::Interleaved)
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
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
        gst::info!(CAT, imp = self, "Changing state {transition:?}");

        match transition {
            gst::StateChange::ReadyToPaused => {
                self.ensure_connection().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.disconnect();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
