// Copyright (C) 2024 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};

use std::default::Default;

use async_tungstenite::tungstenite::error::Error as WsError;
use async_tungstenite::{tokio::connect_async, tungstenite::Message};
use futures::future::{abortable, AbortHandle};
use futures::prelude::*;
use http::Request;
use tokio::runtime;
use tokio::sync::mpsc;
use url::Url;

use std::collections::{BTreeSet, VecDeque};
use std::pin::Pin;
use std::sync::{Condvar, Mutex, MutexGuard};

use atomic_refcell::AtomicRefCell;

use std::sync::LazyLock;

use super::SpeechmaticsTranscriberDiarization;

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct TranscriptMetadata {
    start_time: f32,
    end_time: f32,
    transcript: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[allow(dead_code)]
struct TranscriptDisplay {
    direction: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
enum Tag {
    Profanity,
    Disfluency,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[allow(dead_code)]
struct TranscriptAlternative {
    content: String,
    confidence: f32,
    display: Option<TranscriptDisplay>,
    language: Option<String>,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    tags: Vec<Tag>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[allow(dead_code)]
struct TranscriptResult {
    #[serde(rename = "type")]
    type_: String,
    start_time: f32,
    end_time: f32,
    #[serde(default)]
    is_eos: bool,
    #[serde(default)]
    alternatives: Vec<TranscriptAlternative>,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct Transcript {
    metadata: TranscriptMetadata,
    results: Vec<TranscriptResult>,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct TranslationResult {
    start_time: f32,
    end_time: f32,
    content: String,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct Translation {
    language: String,
    results: Vec<TranslationResult>,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct TranscriptError {
    code: Option<i32>,
    #[serde(rename = "type")]
    type_: String,
    reason: String,
}

#[derive(serde::Serialize, Debug)]
struct AudioType {
    #[serde(rename = "type")]
    type_: String,
    encoding: String,
    sample_rate: u32,
}

#[derive(serde::Serialize, Debug, Clone)]
struct Vocable {
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sounds_like: Vec<String>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "lowercase")]
enum Diarization {
    None,
    Speaker,
}

impl From<SpeechmaticsTranscriberDiarization> for Diarization {
    fn from(val: SpeechmaticsTranscriberDiarization) -> Self {
        use SpeechmaticsTranscriberDiarization::*;
        match val {
            None => Diarization::None,
            Speaker => Diarization::Speaker,
        }
    }
}

#[derive(serde::Serialize, Debug)]
struct SpeakerDiarizationConfig {
    max_speakers: u32,
}

#[derive(serde::Serialize, Debug)]
struct TranscriptFilteringConfig {
    remove_disfluencies: bool,
}

#[derive(serde::Serialize, Debug)]
struct TranscriptionConfig {
    language: String,
    enable_partials: bool,
    max_delay: f32,
    additional_vocab: Vec<Vocable>,
    diarization: Diarization,
    speaker_diarization_config: SpeakerDiarizationConfig,
    transcript_filtering_config: TranscriptFilteringConfig,
}

#[derive(serde::Serialize, Debug)]
struct TranslationConfig {
    target_languages: Vec<String>,
    enable_partials: bool,
}

#[derive(serde::Serialize, Debug)]
struct StartRecognition {
    message: String,
    audio_format: AudioType,
    transcription_config: TranscriptionConfig,
    translation_config: TranslationConfig,
}

#[derive(serde::Serialize, Debug)]
struct EndOfStream {
    message: String,
    last_seq_no: u64,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "speechmaticstranscribe",
        gst::DebugColorFlags::empty(),
        Some("Speechmatics Transcribe element"),
    )
});

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

const DEFAULT_LATENCY_MS: u32 = 8000;
const DEFAULT_MAX_DELAY_MS: u32 = 0;
const DEFAULT_LATENESS_MS: u32 = 0;
const DEFAULT_JOIN_PUNCTUATION: bool = true;
const DEFAULT_ENABLE_LATE_PUNCTUATION_HACK: bool = true;
const DEFAULT_MASK_PROFANITIES: bool = false;
const DEFAULT_DIARIZATION: SpeechmaticsTranscriberDiarization =
    SpeechmaticsTranscriberDiarization::None;
const DEFAULT_MAX_SPEAKERS: u32 = 50;
const DEFAULT_REMOVE_DISFLUENCIES: bool = false;

#[derive(Debug, Clone)]
struct Settings {
    latency_ms: u32,
    max_delay_ms: u32,
    lateness_ms: u32,
    language_code: Option<String>,
    url: Option<String>,
    api_key: Option<String>,
    join_punctuation: bool,
    enable_late_punctuation_hack: bool,
    diarization: SpeechmaticsTranscriberDiarization,
    max_speakers: u32,
    mask_profanities: bool,
    remove_disfluencies: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            latency_ms: DEFAULT_LATENCY_MS,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
            lateness_ms: DEFAULT_LATENESS_MS,
            language_code: Some("en".to_string()),
            url: Some("ws://0.0.0.0:9000".to_string()),
            api_key: None,
            join_punctuation: DEFAULT_JOIN_PUNCTUATION,
            enable_late_punctuation_hack: DEFAULT_ENABLE_LATE_PUNCTUATION_HACK,
            diarization: DEFAULT_DIARIZATION,
            max_speakers: DEFAULT_MAX_SPEAKERS,
            mask_profanities: DEFAULT_MASK_PROFANITIES,
            remove_disfluencies: DEFAULT_REMOVE_DISFLUENCIES,
        }
    }
}

#[derive(Debug)]
struct ItemAccumulator {
    text: String,
    start_time: gst::ClockTime,
    end_time: gst::ClockTime,
    speaker: Option<String>,
    items: Vec<String>,
}

impl From<ItemAccumulator> for gst::Buffer {
    fn from(acc: ItemAccumulator) -> Self {
        let mut buf = gst::Buffer::from_mut_slice(acc.text.into_bytes());

        {
            let buf = buf.get_mut().unwrap();
            buf.set_pts(acc.start_time);
            buf.set_duration(acc.end_time - acc.start_time);

            if let Ok(mut m) = gst::meta::CustomMeta::add(buf, "SpeechmaticsItemMeta") {
                m.mut_structure().set("items", acc.items);
            }
        }

        buf
    }
}

struct State {
    connected: bool,
    draining: bool,
    first_buffer_pts: Option<gst::ClockTime>,
    recv_abort_handle: Option<AbortHandle>,
    send_abort_handle: Option<AbortHandle>,
    in_segment: gst::FormattedSegment<gst::ClockTime>,
    seq_no: u64,
    additional_vocabulary: Vec<Vocable>,
    pad_serial: u32,
    srcpads: BTreeSet<super::TranscriberSrcPad>,
    observed_max_delay: gst::ClockTime,
    upstream_is_live: Option<bool>,
}

impl State {}

impl Default for State {
    fn default() -> Self {
        Self {
            connected: false,
            draining: false,
            first_buffer_pts: gst::ClockTime::NONE,
            recv_abort_handle: None,
            send_abort_handle: None,
            in_segment: gst::FormattedSegment::new(),
            seq_no: 0,
            additional_vocabulary: vec![],
            pad_serial: 0,
            srcpads: BTreeSet::new(),
            observed_max_delay: gst::ClockTime::ZERO,
            upstream_is_live: None,
        }
    }
}

type WsSink = Pin<Box<dyn Sink<Message, Error = WsError> + Send + Sync>>;

pub struct Transcriber {
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    state_cond: Condvar,
    ws_sink: AtomicRefCell<Option<WsSink>>,
}

impl TranscriberSrcPad {
    fn enqueue_translation(
        &self,
        translation: &Translation,
        now: Option<gst::ClockTime>,
        first_pts: gst::ClockTime,
    ) {
        let mut state = self.state.lock().unwrap();

        gst::log!(CAT, "Enqueuing {:?}", translation);

        for item in &translation.results {
            let start_time =
                gst::ClockTime::from_nseconds((item.start_time as f64 * 1_000_000_000.0) as u64)
                    + first_pts;
            let end_time =
                gst::ClockTime::from_nseconds((item.end_time as f64 * 1_000_000_000.0) as u64)
                    + first_pts;

            let mut buf = gst::Buffer::from_mut_slice(item.content.clone().into_bytes());

            {
                let buf = buf.get_mut().unwrap();
                buf.set_pts(start_time);
                buf.set_duration(end_time - start_time);
            }

            state.push_buffer(now, buf);
        }
    }

    fn drain_accumulator(
        &self,
        state: &mut TranscriberSrcPadState,
        now: Option<gst::ClockTime>,
        accumulator: ItemAccumulator,
    ) {
        gst::debug!(
            CAT,
            imp = self,
            "Item is ready: \"{}\", start_time: {}, end_time: {}, speaker: {:?}",
            accumulator.text,
            accumulator.start_time,
            accumulator.end_time,
            accumulator.speaker
        );

        let new_speaker = accumulator.speaker.clone();
        let text = accumulator.text.clone();
        let start_time = accumulator.start_time;
        let end_time = accumulator.end_time;

        if state.current_speaker != accumulator.speaker {
            state.push_speaker(new_speaker.clone());
        }

        let buffer = accumulator.into();

        state.push_event(
            gst::event::CustomUpstream::builder(
                gst::Structure::builder("rstranscribe/new-item")
                    .field("speaker", new_speaker.as_ref().map(|n| n.to_string()))
                    .field("content", text)
                    .field(
                        "running-time",
                        state.out_segment.to_running_time(start_time).unwrap(),
                    )
                    .field("duration", end_time - start_time)
                    .build(),
            )
            .build(),
        );

        state.push_buffer(now, buffer);
    }

    fn enqueue_transcript(
        &self,
        transcript: &Transcript,
        now: Option<gst::ClockTime>,
        first_pts: gst::ClockTime,
        join_punctuation: bool,
    ) {
        gst::log!(CAT, "Enqueuing {:?}", transcript);

        let mut state = self.state.lock().unwrap();

        let mut accumulator: Option<ItemAccumulator> = None;

        for item in &transcript.results {
            if let Some(alternative) = item.alternatives.first() {
                let start_time = gst::ClockTime::from_nseconds(
                    (item.start_time as f64 * 1_000_000_000.0) as u64,
                ) + first_pts;
                let end_time =
                    gst::ClockTime::from_nseconds((item.end_time as f64 * 1_000_000_000.0) as u64)
                        + first_pts;

                if let Some(ref mut accumulator_inner) = accumulator {
                    if item.type_ == "punctuation" {
                        accumulator_inner.text.push_str(&alternative.content);
                        accumulator_inner.end_time = end_time;
                        accumulator_inner
                            .items
                            .push(serde_json::to_string(&item).unwrap());
                    } else {
                        self.drain_accumulator(&mut state, now, accumulator.take().unwrap());

                        accumulator = Some(ItemAccumulator {
                            text: alternative.content.clone(),
                            start_time,
                            end_time,
                            speaker: alternative.speaker.clone(),
                            items: vec![serde_json::to_string(&item).unwrap()],
                        });
                    }
                } else if join_punctuation {
                    accumulator = Some(ItemAccumulator {
                        text: alternative.content.clone(),
                        start_time,
                        end_time,
                        speaker: alternative.speaker.clone(),
                        items: vec![serde_json::to_string(&item).unwrap()],
                    });
                } else {
                    let text = alternative.content.clone();

                    gst::debug!(
                        CAT,
                        imp = self,
                        "Item is ready: \"{}\", start_time: {}, end_time: {}, speaker: {:?}",
                        text,
                        start_time,
                        end_time,
                        alternative.speaker,
                    );

                    if state.current_speaker != alternative.speaker {
                        state.push_speaker(alternative.speaker.clone());
                    }

                    let running_time = state.out_segment.to_running_time(start_time).unwrap();

                    state.push_event(
                        gst::event::CustomUpstream::builder(
                            gst::Structure::builder("rstranscribe/new-item")
                                .field(
                                    "speaker",
                                    alternative.speaker.as_ref().map(|n| n.to_string()),
                                )
                                .field("content", &text)
                                .field("running-time", running_time)
                                .field("duration", end_time - start_time)
                                .build(),
                        )
                        .build(),
                    );

                    let mut buf = gst::Buffer::from_slice(text.into_bytes());

                    {
                        let buf = buf.get_mut().unwrap();
                        buf.set_pts(start_time);
                        buf.set_duration(end_time - start_time);
                        if let Ok(mut m) = gst::meta::CustomMeta::add(buf, "SpeechmaticsItemMeta") {
                            m.mut_structure()
                                .set("items", vec![serde_json::to_string(&item).unwrap()]);
                        }
                    }

                    state.push_buffer(now, buf);
                }
            }
        }

        if let Some(accumulator) = accumulator.take() {
            self.drain_accumulator(&mut state, now, accumulator);
        }

        let start_time = gst::ClockTime::from_nseconds(
            (transcript.metadata.start_time as f64 * 1_000_000_000.0) as u64,
        ) + first_pts;

        let end_time = gst::ClockTime::from_nseconds(
            (transcript.metadata.end_time as f64 * 1_000_000_000.0) as u64,
        ) + first_pts;

        state.advance_position(start_time, end_time);
    }

    fn enqueue_eos(&self) {
        self.state.lock().unwrap().push_eos();
    }

    fn loop_fn(
        &self,
        receiver: &mut mpsc::UnboundedReceiver<TranscriberOutput>,
    ) -> Result<(), gst::ErrorMessage> {
        let future = async move {
            let msg = match receiver.recv().await {
                Some(msg) => msg,
                /* Sender was closed */
                None => {
                    let _ = self.pause_task();
                    return Ok(());
                }
            };

            let Some(parent) = self.obj().parent() else {
                return Ok(());
            };

            let transcriber = parent
                .downcast::<super::Transcriber>()
                .expect("parent is transcriber");

            let (last_position, seqnum) = {
                let state = self.state.lock().unwrap();

                (state.out_segment.position(), state.seqnum)
            };

            match msg {
                TranscriberOutput::Buffer(buf) => {
                    let pts = buf.pts().unwrap();

                    if let Some(last_position) = last_position {
                        if pts > last_position {
                            let gap_event = gst::event::Gap::builder(last_position)
                                .duration(pts - last_position)
                                .seqnum(seqnum)
                                .build();
                            gst::log!(CAT, "Pushing gap:    {} -> {}", last_position, pts);
                            if !self.obj().push_event(gap_event) {
                                return Err(gst::error_msg!(
                                    gst::StreamError::Failed,
                                    ["failed to push gap"]
                                ));
                            }
                        }
                    }

                    let pts_end = if let Some(duration) = buf.duration() {
                        pts + duration
                    } else {
                        pts
                    };

                    gst::debug!(CAT, imp = self, "Pushing buffer: {} -> {}", pts, pts_end,);
                    if let Err(err) = self.obj().push(buf) {
                        return Err(gst::error_msg!(
                            gst::StreamError::Failed,
                            ["failed to push buffer: {err:?}"]
                        ));
                    }

                    Ok(())
                }
                TranscriberOutput::Position(position) => {
                    if let Some(last_position) = last_position {
                        if position > last_position {
                            let gap_event = gst::event::Gap::builder(last_position)
                                .duration(position - last_position)
                                .seqnum(seqnum)
                                .build();
                            gst::log!(CAT, "Pushing gap:    {} -> {}", last_position, position);
                            if !self.obj().push_event(gap_event) {
                                return Err(gst::error_msg!(
                                    gst::StreamError::Failed,
                                    ["failed to push gap"]
                                ));
                            }
                        }
                    }

                    Ok(())
                }
                TranscriberOutput::Event(event) => {
                    if event.is_downstream() {
                        self.obj().push_event(event);
                    } else {
                        transcriber.imp().sinkpad.push_event(event);
                    }

                    Ok(())
                }
                TranscriberOutput::Eos => {
                    let _ = self.pause_task();

                    let draining = transcriber.imp().state.lock().unwrap().draining;

                    if !draining
                        && !self
                            .obj()
                            .push_event(gst::event::Eos::builder().seqnum(seqnum).build())
                    {
                        return Err(gst::error_msg!(
                            gst::StreamError::Failed,
                            ["failed to push EOS"]
                        ));
                    }

                    Ok(())
                }
            }
        };

        RUNTIME.block_on(future)
    }

    fn start_task(&self) -> Result<(), gst::LoggableError> {
        let mut state = self.state.lock().unwrap();

        if state.sender.is_some() {
            gst::debug!(CAT, imp = self, "Have task already");
            return Ok(());
        }

        let this_weak = self.downgrade();
        let pad_weak = self.obj().downgrade();
        let (sender, mut receiver) = mpsc::unbounded_channel();

        state.sender = Some(sender);
        drop(state);

        let res = self.obj().start_task(move || {
            let Some(this) = this_weak.upgrade() else {
                if let Some(pad) = pad_weak.upgrade() {
                    let _ = pad.imp().pause_task();
                }
                return;
            };

            if let Err(err) = this.loop_fn(&mut receiver) {
                let parent = this
                    .obj()
                    .parent()
                    .and_downcast::<gst::Element>()
                    .expect("has parent");
                gst::error!(CAT, imp = this, "Streaming failed: {}", err);
                gst::element_error!(
                    parent,
                    gst::StreamError::Failed,
                    ["Streaming failed: {}", err]
                );
                let _ = this.pause_task();
            }
        });
        if res.is_err() {
            return Err(gst::loggable_error!(CAT, "Failed to start pad task"));
        }
        Ok(())
    }

    fn pause_task(&self) -> Result<(), glib::BoolError> {
        self.state.lock().unwrap().sender = None;

        let res = self.obj().pause_task();

        if let Some(parent) = self.obj().parent() {
            let transcriber = parent
                .downcast::<super::Transcriber>()
                .expect("parent is transcriber");

            transcriber.imp().state_cond.notify_one();
        };

        res
    }

    fn stop_task(&self) -> Result<(), glib::BoolError> {
        self.state.lock().unwrap().sender = None;

        self.obj().stop_task()
    }
}

impl Transcriber {
    fn upstream_is_live(&self) -> Option<bool> {
        if let Some(live) = self.state.lock().unwrap().upstream_is_live {
            return Some(live);
        }

        let mut peer_query = gst::query::Latency::new();

        let ret = self.sinkpad.peer_query(&mut peer_query);

        if ret {
            let (live, _, _) = peer_query.result();
            gst::info!(CAT, imp = self, "queried upstream liveness: {live}");

            self.state.lock().unwrap().upstream_is_live = Some(live);

            Some(live)
        } else {
            gst::trace!(CAT, imp = self, "could not query upstream liveness");

            None
        }
    }

    fn drain<'a>(
        &'a self,
        mut state: MutexGuard<'a, State>,
    ) -> Result<MutexGuard<'a, State>, gst::FlowError> {
        if !state.connected {
            return Ok(state);
        }

        let srcpads = state.srcpads.clone();

        state.draining = true;

        drop(state);

        self.handle_buffer(None)?;

        state = self.state.lock().unwrap();

        gst::info!(CAT, "draining");

        while !srcpads
            .iter()
            .all(|pad| pad.task_state() != gst::TaskState::Started)
            && state.draining
        {
            state = self.state_cond.wait(state).unwrap();
        }

        gst::info!(CAT, "all source pads joined");

        Ok(self.disconnect(state))
    }

    fn src_activatemode(
        &self,
        pad: &super::TranscriberSrcPad,
        _mode: gst::PadMode,
        active: bool,
    ) -> Result<(), gst::LoggableError> {
        if !active {
            pad.imp().stop_task()?;
        }

        Ok(())
    }

    fn src_query(&self, pad: &super::TranscriberSrcPad, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            gst::QueryViewMut::Latency(ref mut q) => {
                let mut peer_query = gst::query::Latency::new();

                let ret = self.sinkpad.peer_query(&mut peer_query);

                if ret {
                    let (live, min, _) = peer_query.result();
                    let our_latency = gst::ClockTime::from_mseconds(
                        self.settings.lock().unwrap().latency_ms as u64,
                    );
                    if live {
                        q.set(true, min + our_latency, gst::ClockTime::NONE);
                    } else {
                        q.set(live, min, gst::ClockTime::NONE);
                    }
                }
                ret
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::debug!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            gst::EventView::Eos(_) => match self.handle_buffer(None) {
                Err(err) => {
                    gst::error!(CAT, "Failed to send EOS: {}", err);
                    false
                }
                Ok(_) => true,
            },
            gst::EventView::FlushStart(_) => {
                gst::info!(CAT, imp = self, "Received flush start, disconnecting");

                let mut ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);

                drop(self.disconnect(self.state.lock().unwrap()));

                let state = self.state.lock().unwrap();
                for srcpad in &state.srcpads {
                    if let Err(err) = srcpad.imp().stop_task() {
                        gst::error!(CAT, imp = self, "Failed to stop srcpad task: {}", err);
                        ret = false;
                    }
                }

                self.state_cond.notify_one();

                ret
            }
            gst::EventView::FlushStop(_) => {
                gst::info!(CAT, imp = self, "Received flush stop, restarting task");

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::Segment(e) => {
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

                let mut state = self.state.lock().unwrap();

                // If segment changes in any other way than the position
                // we'll have to drain first.
                let mut compare_segment = segment.clone();
                compare_segment.set_position(state.in_segment.position());
                if state.in_segment != compare_segment {
                    // Do drain
                    state = match self.drain(state) {
                        Err(err) => {
                            gst::error!(CAT, imp = self, "Failed to drain: {err}");
                            gst::element_imp_error!(
                                self,
                                gst::StreamError::Failed,
                                ["Failed to drain: {err}"]
                            );
                            return false;
                        }
                        Ok(state) => state,
                    };
                }

                gst::info!(CAT, imp = self, "input segment is now {:?}", segment);

                state.in_segment = segment.clone();
                let srcpads = state.srcpads.clone();

                drop(state);

                let lateness =
                    gst::ClockTime::from_mseconds(self.settings.lock().unwrap().lateness_ms as u64);

                let mut segment_to_push = segment.clone();
                segment_to_push.set_base(segment.base().unwrap_or(gst::ClockTime::ZERO) + lateness);

                for srcpad in &srcpads {
                    let seqnum = {
                        let mut sstate = srcpad.imp().state.lock().unwrap();
                        sstate.out_segment = segment.clone();
                        sstate.seqnum = e.seqnum();
                        sstate.seqnum
                    };

                    let out_event = gst::event::Segment::builder(&segment_to_push)
                        .seqnum(seqnum)
                        .build();
                    srcpad.push_event(out_event.clone());
                }

                true
            }
            gst::EventView::Tag(_) => true,
            gst::EventView::StreamStart(e) => {
                let srcpads = self.state.lock().unwrap().srcpads.clone();
                for srcpad in &srcpads {
                    let sev = gst::event::StreamStart::builder("transcription")
                        .seqnum(e.seqnum())
                        .build();
                    if !srcpad.push_event(sev) {
                        gst::error!(CAT, obj = srcpad, "Failed to push stream start event");
                        return false;
                    }
                }
                true
            }
            gst::EventView::Caps(e) => {
                if let Some(old_caps) = self.sinkpad.current_caps() {
                    if old_caps != *e.caps() {
                        if let Err(err) = self.drain(self.state.lock().unwrap()) {
                            gst::error!(CAT, imp = self, "Failed to drain: {err}");
                            gst::element_imp_error!(
                                self,
                                gst::StreamError::Failed,
                                ["Failed to drain: {err}"]
                            );
                            return false;
                        };
                    }
                }

                let srcpads = self.state.lock().unwrap().srcpads.clone();

                for srcpad in &srcpads {
                    let seqnum = srcpad.imp().state.lock().unwrap().seqnum;

                    let caps = gst::Caps::builder("text/x-raw")
                        .field("format", "utf8")
                        .build();
                    let out_event = gst::event::Caps::builder(&caps).seqnum(seqnum).build();
                    srcpad.push_event(out_event);
                }

                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    async fn send(&self, buffer: Option<gst::Buffer>) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut n_chunks = 0;

        if let Some(ws_sink) = self.ws_sink.borrow_mut().as_mut() {
            if let Some(buffer) = buffer {
                let data = buffer.map_readable().unwrap();
                for chunk in data.chunks(8192) {
                    ws_sink
                        .send(Message::binary(chunk.to_vec()))
                        .await
                        .map_err(|err| {
                            gst::error!(CAT, imp = self, "Failed sending packet: {}", err);
                            gst::FlowError::Error
                        })?;
                    n_chunks += 1;
                }
            } else {
                let end_message = EndOfStream {
                    message: "EndOfStream".to_string(),
                    last_seq_no: self.state.lock().unwrap().seq_no,
                };
                let message = serde_json::to_string(&end_message).unwrap();

                ws_sink.send(Message::text(message)).await.map_err(|err| {
                    gst::error!(CAT, imp = self, "Failed sending packet: {}", err);
                    gst::FlowError::Error
                })?;
            }
        }

        self.state.lock().unwrap().seq_no += n_chunks;

        Ok(gst::FlowSuccess::Ok)
    }

    fn handle_buffer(
        &self,
        buffer: Option<gst::Buffer>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Handling {:?}", buffer);

        let now = self.obj().current_running_time();

        if let Some(ref buffer) = buffer {
            let mut state = self.state.lock().unwrap();

            if buffer.flags().contains(gst::BufferFlags::DISCONT) {
                state = match self.drain(state) {
                    Err(err) => {
                        gst::error!(CAT, imp = self, "Failed to drain: {err}");
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Failed,
                            ["Failed to drain: {err}"]
                        );
                        return Err(err);
                    }
                    Ok(state) => state,
                };
            }

            let pts = buffer.pts().expect("Checked in sink_chain()");

            if state.first_buffer_pts.is_none() {
                state.first_buffer_pts = Some(pts);
                for srcpad in &state.srcpads {
                    let mut sstate = srcpad.imp().state.lock().unwrap();
                    sstate.out_segment.set_position(pts);
                }
            }

            if let Some(now) = now {
                for srcpad in &state.srcpads {
                    srcpad
                        .imp()
                        .state
                        .lock()
                        .unwrap()
                        .input_times
                        .push_back((pts, now));
                }
            }
        }

        self.ensure_connection().map_err(|err| {
            // No need to worry too much here, we didn't have a session to
            // terminate in the first place
            if buffer.is_none() {
                return gst::FlowError::Eos;
            }
            gst::element_imp_error!(
                self,
                gst::StreamError::Failed,
                ["Streaming failed: {}", err]
            );
            gst::FlowError::Error
        })?;

        let (future, abort_handle) = abortable(self.send(buffer));

        self.state.lock().unwrap().send_abort_handle = Some(abort_handle);

        let res = RUNTIME.block_on(future);

        match res {
            Err(_) => Err(gst::FlowError::Flushing),
            Ok(res) => res,
        }
    }

    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if buffer.pts().is_none() {
            gst::error!(CAT, imp = self, "Only buffers with PTS supported");
            return Err(gst::FlowError::Error);
        }

        self.handle_buffer(Some(buffer))
    }

    fn dispatch_message(&self, msg: Message) -> Result<(), gst::ErrorMessage> {
        let upstream_is_live = self.upstream_is_live();

        match msg {
            Message::Text(text) => {
                let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
                let message_type = {
                    let obj = json.as_object_mut().expect("object");
                    let Some(message_type) = obj.remove("message") else {
                        return Err(gst::error_msg!(
                            gst::StreamError::Failed,
                            ["Missing message field in object: {}", text]
                        ));
                    };
                    if let serde_json::Value::String(s) = message_type {
                        s
                    } else {
                        panic!("message field not a string");
                    }
                };

                let now = self.obj().current_running_time();

                let (transcriber_language_code, join_punctuation, mask_profanities) = {
                    let settings = self.settings.lock().unwrap();
                    (
                        settings.language_code.clone(),
                        settings.join_punctuation,
                        settings.mask_profanities,
                    )
                };

                let first_pts = self.state.lock().unwrap().first_buffer_pts.unwrap();

                match message_type.as_str() {
                    "AddTranslation" => {
                        let translation: Translation =
                            serde_json::from_value(json).map_err(|err| {
                                gst::error_msg!(
                                    gst::StreamError::Failed,
                                    ["Unexpected message: {} ({})", text, err]
                                )
                            })?;

                        gst::log!(CAT, imp = self, "Parsed translation {:?}", translation);

                        let s = gst::Structure::builder("speechmatics/raw")
                            .field("translation", &*text)
                            .field("arrival-time", now)
                            .field("language-code", &translation.language)
                            .build();

                        gst::trace!(
                            CAT,
                            imp = self,
                            "received translation event, posting message"
                        );

                        let _ = self.obj().post_message(
                            gst::message::Element::builder(s).src(&*self.obj()).build(),
                        );

                        for srcpad in &self.state.lock().unwrap().srcpads {
                            if Some(&translation.language)
                                != srcpad.imp().settings.lock().unwrap().language_code.as_ref()
                            {
                                continue;
                            }

                            srcpad
                                .imp()
                                .enqueue_translation(&translation, now, first_pts);
                        }
                    }
                    "AddTranscript" | "AddPartialTranscript" => {
                        let mut transcript: Transcript =
                            serde_json::from_value(json).map_err(|err| {
                                gst::error_msg!(
                                    gst::StreamError::Failed,
                                    ["Unexpected message: {} ({})", text, err]
                                )
                            })?;

                        gst::log!(CAT, imp = self, "Parsed transcript {:?}", transcript);

                        for item in transcript.results.iter_mut() {
                            if mask_profanities {
                                for alternative in item.alternatives.iter_mut() {
                                    if alternative.tags.contains(&Tag::Profanity) {
                                        alternative.content =
                                            std::iter::repeat_n("*", alternative.content.len())
                                                .collect();
                                    }
                                }
                            }
                        }

                        let s = gst::Structure::builder("speechmatics/raw")
                            .field("transcript", &*text)
                            .field("arrival-time", now)
                            .field("language-code", transcriber_language_code)
                            .build();

                        gst::trace!(
                            CAT,
                            imp = self,
                            "received transcript event, posting message"
                        );

                        let _ = self.obj().post_message(
                            gst::message::Element::builder(s).src(&*self.obj()).build(),
                        );

                        for srcpad in &self.state.lock().unwrap().srcpads {
                            if srcpad
                                .imp()
                                .settings
                                .lock()
                                .unwrap()
                                .language_code
                                .is_some()
                            {
                                continue;
                            }

                            srcpad.imp().enqueue_transcript(
                                &transcript,
                                now,
                                first_pts,
                                join_punctuation,
                            );
                        }
                    }
                    "EndOfTranscript" => {
                        for srcpad in &self.state.lock().unwrap().srcpads {
                            srcpad.imp().enqueue_eos();
                        }
                    }
                    _ => (),
                }

                let (lateness, latency) = {
                    let settings = self.settings.lock().unwrap();

                    (
                        gst::ClockTime::from_mseconds(settings.lateness_ms as u64),
                        gst::ClockTime::from_mseconds(settings.latency_ms as u64),
                    )
                };

                let max_srcpad_delay = self
                    .state
                    .lock()
                    .unwrap()
                    .srcpads
                    .iter()
                    .map(|pad| pad.imp().state.lock().unwrap().observed_max_delay)
                    .max();
                if let Some(max_srcpad_delay) = max_srcpad_delay {
                    let mut do_notify = false;
                    let mut state = self.state.lock().unwrap();

                    if state.observed_max_delay < max_srcpad_delay {
                        gst::log!(CAT, imp = self, "new max delay {max_srcpad_delay}");
                        state.observed_max_delay = max_srcpad_delay;
                        do_notify = true;
                    }

                    drop(state);

                    if do_notify {
                        self.obj().notify("max-observed-delay");
                        if max_srcpad_delay > latency + lateness {
                            let details =
                                gst::Structure::builder("speechmaticstranscriber/excessive-delay")
                                    .field("new-observed-max-delay", max_srcpad_delay)
                                    .build();
                            if upstream_is_live == Some(true) {
                                gst::element_warning!(
                                    self.obj(),
                                    gst::CoreError::Clock,
                                    ["Maximum observed delay {} exceeds configured lateness + latency", max_srcpad_delay],
                                    details: details
                                );
                            }
                        }
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn ensure_connection(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();

        if state.connected {
            return Ok(());
        }

        let in_caps = self
            .sinkpad
            .current_caps()
            .ok_or_else(|| gst::error_msg!(gst::CoreError::Failed, ["No caps set on sinkpad"]))?;
        let s = in_caps.structure(0).unwrap();
        let sample_rate: i32 = s.get::<i32>("rate").unwrap();

        let settings = self.settings.lock().unwrap();

        gst::info!(CAT, imp = self, "Connecting ..");

        let url = match &settings.url {
            Some(url) => url.to_string(),
            None => "ws://0.0.0.0:9000".to_string(),
        };

        let uri = Url::parse(&url).map_err(|e| {
            gst::error_msg!(
                gst::CoreError::Failed,
                ["Failed to parse provided url: {}", e]
            )
        })?;
        let Some(api_key) = settings.api_key.clone() else {
            return Err(gst::error_msg!(
                gst::CoreError::Failed,
                ["An API key is required"]
            ));
        };
        let authority = uri.authority();
        let host = authority.splitn(2, '@').last().unwrap_or("");

        let request = Request::builder()
            .method("GET")
            .uri(&url)
            .header("Host", host)
            .header("Upgrade", "websocket")
            .header("Connection", "keep-alive, upgrade")
            .header(
                "Sec-Websocket-Key",
                async_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Sec-Websocket-Version", "13")
            .header("Authorization", format!("Bearer {}", &api_key))
            .body(())
            .unwrap();

        let (ws, _) = RUNTIME.block_on(connect_async(request)).map_err(|err| {
            gst::error!(CAT, imp = self, "Failed to connect: {}", err);
            gst::error_msg!(gst::CoreError::Failed, ["Failed to connect: {}", err])
        })?;

        let (mut ws_sink, mut ws_stream) = ws.split();

        if settings.latency_ms + settings.lateness_ms < 700 {
            gst::error!(
                CAT,
                imp = self,
                "latency + lateness must be above 700 milliseconds",
            );
            return Err(gst::error_msg!(
                gst::LibraryError::Settings,
                ["latency + lateness must be above 700 milliseconds",]
            ));
        }

        let translation_languages = state
            .srcpads
            .iter()
            .flat_map(|pad| pad.imp().settings.lock().unwrap().language_code.clone())
            .collect();
        gst::info!(
            CAT,
            "Translation languages: {:?} ({})",
            translation_languages,
            state.srcpads.len()
        );

        let max_delay = if settings.max_delay_ms == 0 {
            ((settings.latency_ms + settings.lateness_ms) as f32) / 1000.
        } else {
            settings.max_delay_ms as f32 / 1000.
        };

        let start_message = StartRecognition {
            message: "StartRecognition".to_string(),
            audio_format: AudioType {
                type_: "raw".to_string(),
                encoding: "pcm_s16le".to_string(),
                sample_rate: sample_rate as u32,
            },
            transcription_config: TranscriptionConfig {
                language: settings
                    .language_code
                    .clone()
                    .unwrap_or_else(|| "en".to_string()),
                enable_partials: false,
                max_delay,
                additional_vocab: state.additional_vocabulary.clone(),
                diarization: settings.diarization.into(),
                speaker_diarization_config: SpeakerDiarizationConfig {
                    max_speakers: settings.max_speakers,
                },
                transcript_filtering_config: TranscriptFilteringConfig {
                    remove_disfluencies: settings.remove_disfluencies,
                },
            },
            translation_config: TranslationConfig {
                target_languages: translation_languages,
                enable_partials: false,
            },
        };

        let message = serde_json::to_string(&start_message).unwrap();

        gst::debug!(CAT, imp = self, "Sending start message: {}", message);

        RUNTIME
            .block_on(ws_sink.send(Message::text(message)))
            .map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to send StartRecognition: {err}");
                gst::error_msg!(
                    gst::CoreError::Failed,
                    ["Failed to send StartRecognition: {err}"]
                )
            })?;

        loop {
            let res = RUNTIME
                .block_on(ws_stream.next())
                .ok_or_else(|| {
                    gst::error!(CAT, imp = self, "Connection closed unexpectedly");
                    gst::error_msg!(gst::CoreError::Failed, ["Connection closed unexpectedly"])
                })?
                .map_err(|err| {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to receive RecognitionStarted: {err}"
                    );
                    gst::error_msg!(
                        gst::CoreError::Failed,
                        ["Failed to receive RecognitionStarted: {err}"]
                    )
                })?;

            let text = match res {
                Message::Text(text) => Ok(text),
                _ => {
                    gst::error!(CAT, imp = self, "Invalid message type: {}", res);
                    Err(gst::error_msg!(
                        gst::CoreError::Failed,
                        ["Invalid message type: {}", res]
                    ))
                }
            }?;

            let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
            let message_type = {
                let obj = json.as_object_mut().expect("object");
                let message_type = obj.remove("message").expect("`message` field");
                if let serde_json::Value::String(s) = message_type {
                    s
                } else {
                    panic!("message field not a string");
                }
            };

            match message_type.as_str() {
                "RecognitionStarted" => {
                    gst::info!(CAT, imp = self, "Recognition started!");
                    break;
                }
                "Error" => {
                    let error: TranscriptError = serde_json::from_value(json).map_err(|err| {
                        gst::error_msg!(
                            gst::StreamError::Failed,
                            ["Unexpected message: {} ({})", text, err]
                        )
                    })?;
                    gst::error!(
                        CAT,
                        imp = self,
                        "StartRecognition failed: {} ({})",
                        error.type_,
                        error.reason
                    );
                    Err(gst::error_msg!(
                        gst::CoreError::Failed,
                        ("StartRecognition failed"),
                        ["{} ({})", error.type_, error.reason]
                    ))
                }
                _ => {
                    continue;
                }
            }?;
        }

        *self.ws_sink.borrow_mut() = Some(Box::pin(ws_sink));

        for srcpad in &state.srcpads {
            if let Err(err) = srcpad.imp().start_task() {
                gst::error!(CAT, imp = self, "Failed to start srcpad task: {}", err);
                return Err(gst::error_msg!(
                    gst::CoreError::Failed,
                    ["Failed to start srcpad task: {err}"]
                ));
            }
        }

        let this_weak = self.downgrade();
        let future = async move {
            while let Some(this) = this_weak.upgrade() {
                let Some(msg) = ws_stream.next().await else {
                    break;
                };

                let msg = match msg {
                    Ok(msg) => msg,
                    Err(err) => {
                        gst::error!(CAT, imp = this, "Failed to receive data: {}", err);
                        gst::element_imp_error!(
                            this,
                            gst::StreamError::Failed,
                            ["Streaming failed: {}", err]
                        );
                        break;
                    }
                };

                if let Err(err) = this.dispatch_message(msg) {
                    gst::error!(CAT, imp = this, "Failed to dispatch message: {}", err);
                    gst::element_imp_error!(
                        this,
                        gst::StreamError::Failed,
                        ["Failed to dispatch message: {}", err]
                    );
                    break;
                }
            }
        };

        let (future, abort_handle) = abortable(future);

        state.recv_abort_handle = Some(abort_handle);

        RUNTIME.spawn(future);

        state.connected = true;

        gst::info!(CAT, imp = self, "Connected");

        Ok(())
    }

    fn disconnect<'a>(&'a self, mut state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        gst::info!(CAT, imp = self, "Unpreparing");

        if let Some(abort_handle) = state.recv_abort_handle.take() {
            abort_handle.abort();
        }

        if let Some(abort_handle) = state.send_abort_handle.take() {
            abort_handle.abort();
        }

        state.draining = false;

        drop(state);

        self.state_cond.notify_one();

        let _lock = self.sinkpad.stream_lock();

        if let Some(mut ws_sink) = self.ws_sink.borrow_mut().take() {
            RUNTIME.block_on(async {
                let _ = ws_sink.close().await;
            });
        }

        let mut state = self.state.lock().unwrap();

        let srcpads = state.srcpads.clone();

        *state = State::default();

        drop(state);

        for srcpad in &srcpads {
            let _ = srcpad.stop_task();

            let mut sstate = srcpad.imp().state.lock().unwrap();
            *sstate = TranscriberSrcPadState::default();
        }

        let mut state = self.state.lock().unwrap();

        state.srcpads = srcpads;

        gst::info!(
            CAT,
            imp = self,
            "Unprepared, connected: {}!",
            state.connected
        );

        state
    }
}

// Implementation of gst::ChildProxy virtual methods.
//
// This allows accessing the pads and their properties from e.g. gst-launch.
impl ChildProxyImpl for Transcriber {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Transcriber {
    const NAME: &'static str = "GstSpeechmaticsTranscriber";
    type Type = super::Transcriber;
    type ParentType = gst::Element;
    type Interfaces = (gst::ChildProxy,);

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |transcriber| transcriber.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.sink_event(pad, event),
                )
            })
            .build();

        let settings = Mutex::new(Settings::default());

        Self {
            sinkpad,
            settings,
            state: Default::default(),
            state_cond: Condvar::new(),
            ws_sink: Default::default(),
        }
    }
}

impl GstObjectImpl for Transcriber {}

impl ObjectImpl for Transcriber {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("The Language of the Stream, ISO code")
                    .default_value("en")
                    .build(),
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow for transcription")
                    .default_value(DEFAULT_LATENCY_MS)
                    .build(),
                glib::ParamSpecUInt::builder("max-delay")
                    .nick("Max Delay")
                    .blurb("Max delay to pass to the speechmatics API (0 = use latency)")
                    .default_value(DEFAULT_LATENCY_MS)
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Lateness")
                    .blurb("Amount of milliseconds to introduce as lateness")
                    .default_value(DEFAULT_LATENESS_MS)
                    .build(),
                glib::ParamSpecString::builder("url")
                    .nick("URL")
                    .blurb("URL of the transcription server")
                    .default_value("ws://0.0.0.0:9000")
                    .build(),
                gst::ParamSpecArray::builder("additional-vocabulary")
                    .nick("Additional Vocabulary")
                    .blurb("Additional vocabulary speechmatics should use")
                    .element_spec(
                        &glib::ParamSpecBoxed::builder::<gst::Structure>("vocable")
                            .nick("Vocable")
                            .blurb("A vocable in the vocabulary")
                            .build(),
                    )
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("api-key")
                    .nick("API Key")
                    .blurb("Speechmatics API Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("join-punctuation")
                    .nick("Join punctuation")
                    .blurb("Whether punctuation should be joined with the preceding word")
                    .default_value(DEFAULT_JOIN_PUNCTUATION)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("enable-late-punctuation-hack")
                    .nick("Enable late punctuation hack")
                    .blurb(
                        "deprecated: speechmatics now appears to group punctuation reliably",
                    )
                    .default_value(DEFAULT_ENABLE_LATE_PUNCTUATION_HACK)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("diarization", DEFAULT_DIARIZATION)
                    .nick("Diarization")
                    .blurb("Defines how to separate speakers in the audio")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-speakers")
                    .nick("Max Speakers")
                    .blurb("The maximum number of speakers that may be detected with diarization=speaker")
                    .default_value(DEFAULT_MAX_SPEAKERS)
                    .build(),
                glib::ParamSpecBoolean::builder("mask-profanities")
                    .nick("Mask profanities")
                    .blurb(
                        "Mask profanities with * of the same length as the word"
                    )
                    .default_value(DEFAULT_MASK_PROFANITIES)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-observed-delay")
                    .nick("Maximum Observed Delay")
                    .blurb("Maximum delay observed between the sending of an audio sample and the reception of an item")
                    .default_value(0)
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("remove-disfluencies")
                    .nick("Remove disfluencies")
                    .blurb(
                        "Remove hesitation sounds from transcript"
                    )
                    .default_value(DEFAULT_REMOVE_DISFLUENCIES)
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

        let templ = obj.class().pad_template("src").unwrap();
        let srcpad: super::TranscriberSrcPad = gst::PadBuilder::from_template(&templ)
            .activatemode_function(|pad, parent, mode, active| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating src pad with mode"
                        ))
                    },
                    |transcriber| transcriber.src_activatemode(pad, mode, active),
                )
            })
            .query_function(|pad, parent, query| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();
        obj.add_pad(&srcpad).unwrap();

        self.state.lock().unwrap().srcpads.insert(srcpad);

        obj.set_element_flags(gst::ElementFlags::PROVIDE_CLOCK | gst::ElementFlags::REQUIRE_CLOCK);
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "language-code" => {
                let mut settings = self.settings.lock().unwrap();
                settings.language_code = value.get().expect("type checked upstream");
            }
            "latency" => {
                let mut settings = self.settings.lock().unwrap();
                let old_latency_ms = settings.latency_ms;
                settings.latency_ms = value.get().expect("type checked upstream");
                if settings.latency_ms != old_latency_ms {
                    gst::debug!(CAT, imp = self, "Latency changed: {}", settings.latency_ms);
                    drop(settings);
                    let _ = self
                        .obj()
                        .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }
            "max-delay" => {
                let mut settings = self.settings.lock().unwrap();
                settings.max_delay_ms = value.get().expect("type checked upstream");
            }
            "lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lateness_ms = value.get().expect("type checked upstream");
            }
            "url" => {
                let mut settings = self.settings.lock().unwrap();
                settings.url = value.get().expect("type checked upstream");
            }
            "additional-vocabulary" => {
                let mut state = self.state.lock().unwrap();
                state.additional_vocabulary = vec![];
                let vocables: gst::Array = value.get().expect("type checked upstream");
                for vocable in vocables.as_slice() {
                    let Some(s) = vocable
                        .get::<Option<gst::Structure>>()
                        .expect("type checked upstream")
                    else {
                        continue;
                    };

                    let Ok(content) = s.get::<String>("word") else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "skipping vocable: {s}, expected word field",
                        );
                        continue;
                    };

                    let sounds_like: Vec<String> = match s.get::<gst::Array>("sounds_like") {
                        Ok(sounds_like) => sounds_like
                            .as_slice()
                            .iter()
                            .filter_map(|s| s.get::<Option<String>>().unwrap_or(None))
                            .collect(),
                        Err(_) => vec![],
                    };

                    state.additional_vocabulary.push(Vocable {
                        content,
                        sounds_like,
                    });
                }
            }
            "api-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.api_key = value.get().expect("type checked upstream");
            }
            "join-punctuation" => {
                let mut settings = self.settings.lock().unwrap();
                settings.join_punctuation = value.get().expect("type checked upstream");
            }
            "enable-late-punctuation-hack" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_late_punctuation_hack = value.get().expect("type checked upstream");
            }
            "diarization" => {
                let mut settings = self.settings.lock().unwrap();
                settings.diarization = value
                    .get::<SpeechmaticsTranscriberDiarization>()
                    .expect("type checked upstream");
            }
            "max-speakers" => {
                let mut settings = self.settings.lock().unwrap();
                settings.max_speakers = value.get().expect("type checked upstream");
            }
            "mask-profanities" => {
                let mut settings = self.settings.lock().unwrap();
                settings.mask_profanities = value.get().expect("type checked upstream");
            }
            "remove-disfluencies" => {
                let mut settings = self.settings.lock().unwrap();
                settings.remove_disfluencies = value.get().expect("type checked upstream");
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
            "latency" => {
                let settings = self.settings.lock().unwrap();
                settings.latency_ms.to_value()
            }
            "max-delay" => {
                let settings = self.settings.lock().unwrap();
                settings.max_delay_ms.to_value()
            }
            "lateness" => {
                let settings = self.settings.lock().unwrap();
                settings.lateness_ms.to_value()
            }
            "url" => {
                let settings = self.settings.lock().unwrap();
                settings.url.to_value()
            }
            "additional-vocabulary" => {
                let state = self.state.lock().unwrap();
                let mut additional_vocabulary = vec![];
                for vocable in &state.additional_vocabulary {
                    let mut s = gst::Structure::new_empty(&vocable.content);
                    if !vocable.sounds_like.is_empty() {
                        s.set(
                            "sounds_like",
                            gst::Array::new(
                                vocable.sounds_like.iter().map(|word| word.to_send_value()),
                            ),
                        );
                    }
                    additional_vocabulary.push(s.to_send_value());
                }
                gst::Array::new(additional_vocabulary).to_value()
            }
            "api-key" => {
                let settings = self.settings.lock().unwrap();
                settings.api_key.to_value()
            }
            "join-punctuation" => {
                let settings = self.settings.lock().unwrap();
                settings.join_punctuation.to_value()
            }
            "enable-late-punctuation-hack" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_late_punctuation_hack.to_value()
            }
            "diarization" => self.settings.lock().unwrap().diarization.to_value(),
            "max-speakers" => self.settings.lock().unwrap().max_speakers.to_value(),
            "mask-profanities" => {
                let settings = self.settings.lock().unwrap();
                settings.mask_profanities.to_value()
            }
            "remove-disfluencies" => {
                let settings = self.settings.lock().unwrap();
                settings.remove_disfluencies.to_value()
            }
            "max-observed-delay" => {
                let state = self.state.lock().unwrap();
                (state.observed_max_delay.mseconds() as u32).to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl ElementImpl for Transcriber {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Transcriber",
                "Audio/Text/Filter",
                "Speech to Text filter, using Speechmatics transcribe",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
            let src_pad_template = gst::PadTemplate::with_gtype(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
                super::TranscriberSrcPad::static_type(),
            )
            .unwrap();
            let req_src_pad_template = gst::PadTemplate::with_gtype(
                "translate_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Request,
                &src_caps,
                super::TranscriberSrcPad::static_type(),
            )
            .unwrap();

            let sink_caps = gst_audio::AudioCapsBuilder::new()
                .format(gst_audio::AUDIO_FORMAT_S16)
                .rate_range(8000..=48000)
                .channels(1)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            vec![src_pad_template, req_src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut state = self.state.lock().unwrap();

        let pad: super::TranscriberSrcPad = gst::PadBuilder::from_template(templ)
            .activatemode_function(|pad, parent, mode, active| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating src pad with mode"
                        ))
                    },
                    |transcriber| transcriber.src_activatemode(pad, mode, active),
                )
            })
            .query_function(|pad, parent, query| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.src_query(pad, query),
                )
            })
            .name(format!("translate_src_{}", state.pad_serial).as_str())
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        state.srcpads.insert(pad.clone());

        gst::info!(CAT, "New pad requested, {}", state.srcpads.len());

        state.pad_serial += 1;
        drop(state);

        self.obj().add_pad(&pad).unwrap();

        pad.set_active(true).unwrap();

        self.obj().child_added(&pad, &pad.name());

        Some(pad.upcast())
    }

    fn release_pad(&self, pad: &gst::Pad) {
        pad.set_active(false).unwrap();
        self.obj().remove_pad(pad).unwrap();
        self.state.lock().unwrap().srcpads.remove(pad);

        self.obj().child_removed(pad, &pad.name());

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::info!(CAT, imp = self, "Changing state {:?}", transition);

        let success = self.parent_change_state(transition)?;

        if transition == gst::StateChange::PausedToReady {
            drop(self.disconnect(self.state.lock().unwrap()));
        }

        Ok(success)
    }
}

#[derive(Debug, Default, Clone)]
struct TranscriberSrcPadSettings {
    language_code: Option<String>,
}

#[derive(Debug)]
enum TranscriberOutput {
    Buffer(gst::Buffer),
    Event(gst::Event),
    Eos,
    Position(gst::ClockTime),
}

#[derive(Debug)]
struct TranscriberSrcPadState {
    sender: Option<mpsc::UnboundedSender<TranscriberOutput>>,
    discont: bool,
    out_segment: gst::FormattedSegment<gst::ClockTime>,
    seqnum: gst::Seqnum,
    current_speaker: Option<String>,
    input_times: VecDeque<(gst::ClockTime, gst::ClockTime)>,
    observed_max_delay: gst::ClockTime,
}

impl Default for TranscriberSrcPadState {
    fn default() -> Self {
        Self {
            sender: None,
            discont: true,
            out_segment: gst::FormattedSegment::new(),
            seqnum: gst::Seqnum::next(),
            current_speaker: None,
            input_times: VecDeque::default(),
            observed_max_delay: gst::ClockTime::ZERO,
        }
    }
}

#[derive(Debug, Default)]
pub struct TranscriberSrcPad {
    settings: Mutex<TranscriberSrcPadSettings>,
    state: Mutex<TranscriberSrcPadState>,
}

impl TranscriberSrcPadState {
    fn trim_input_times(&mut self, pts: gst::ClockTime) -> Option<gst::ClockTime> {
        let mut iter = self.input_times.iter().peekable();

        let mut pos = 0;

        let input_running_time = loop {
            let Some(current) = iter.next() else {
                break None;
            };

            if let Some(next_input_time) = iter.peek() {
                if next_input_time.0 > pts {
                    break Some(current.1);
                }
            } else {
                break Some(current.1);
            }

            pos += 1;
        };

        self.input_times = self.input_times.split_off(pos);

        input_running_time
    }

    fn push_buffer(&mut self, now: Option<gst::ClockTime>, mut buf: gst::Buffer) {
        if self.discont {
            let buf = buf.make_mut();
            buf.set_flags(gst::BufferFlags::DISCONT);
            self.discont = false;
        }

        if let Some(input_running_time) = self.trim_input_times(buf.pts().unwrap()) {
            if let Some(now) = now {
                let item_delay = now.saturating_sub(input_running_time);

                self.observed_max_delay = self.observed_max_delay.max(item_delay);
            }
        }

        if let Some(ref mut sender) = self.sender {
            let _ = sender.send(TranscriberOutput::Buffer(buf));
        }
    }

    fn advance_position(&mut self, start_time: gst::ClockTime, end_time: gst::ClockTime) {
        self.trim_input_times(start_time);

        if let Some(ref mut sender) = self.sender {
            let _ = sender.send(TranscriberOutput::Position(end_time));
        }
    }

    fn push_event(&mut self, event: gst::Event) {
        if let Some(ref mut sender) = self.sender {
            let _ = sender.send(TranscriberOutput::Event(event));
        }
    }

    fn push_speaker(&mut self, speaker: Option<String>) {
        let event = gst::event::CustomDownstream::builder(
            gst::Structure::builder("rstranscribe/speaker-change")
                .field("speaker", &speaker)
                .build(),
        )
        .build();

        self.current_speaker = speaker;

        self.push_event(event);
    }

    fn push_eos(&mut self) {
        if let Some(ref mut sender) = self.sender {
            let _ = sender.send(TranscriberOutput::Eos);
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TranscriberSrcPad {
    const NAME: &'static str = "GstSpeechmaticsTranscriberSrcPad";
    type Type = super::TranscriberSrcPad;
    type ParentType = gst::Pad;

    fn new() -> Self {
        Default::default()
    }
}

const OUTPUT_LANG_CODE_PROPERTY: &str = "language-code";
const DEFAULT_OUTPUT_LANG_CODE: Option<&str> = None;

impl ObjectImpl for TranscriberSrcPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder(OUTPUT_LANG_CODE_PROPERTY)
                .nick("Language Code")
                .blurb("The Language the Stream must be translated to")
                .default_value(DEFAULT_OUTPUT_LANG_CODE)
                .mutable_ready()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            OUTPUT_LANG_CODE_PROPERTY => {
                let language_code: Option<String> = value.get().unwrap();

                self.settings.lock().unwrap().language_code = language_code.clone();

                if let Some(language_code) = language_code {
                    // Make sure our tags do not get overwritten
                    let sev = gst::event::StreamStart::builder("transcription").build();
                    let _ = self.obj().store_sticky_event(&sev);

                    let mut tl = gst::TagList::new();
                    tl.make_mut().add::<gst::tags::LanguageCode>(
                        &language_code.as_str(),
                        gst::TagMergeMode::Append,
                    );
                    let ev = gst::event::Tag::builder(tl).build();
                    let _ = self.obj().store_sticky_event(&ev);
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            OUTPUT_LANG_CODE_PROPERTY => self.settings.lock().unwrap().language_code.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TranscriberSrcPad {}

impl PadImpl for TranscriberSrcPad {}
