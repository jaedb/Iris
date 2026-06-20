// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! Deepgram Transcriber element.
//!
//! This element calls the Deepgram streaming transcription API to generate text from audio

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};

use std::sync::{Condvar, Mutex, MutexGuard};

use std::sync::LazyLock;

use std::collections::VecDeque;

use tokio::runtime;

use futures::channel::mpsc;
use futures::prelude::*;

use super::{DeepgramInterimStrategy, CAT};
use deepgram::{
    common::{options::Encoding, options::OptionsBuilder, stream_response::StreamResponse},
    Deepgram,
};

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("gst-deepgram-runtime")
        .build()
        .unwrap()
});

const GRANULARITY: gst::ClockTime = gst::ClockTime::from_mseconds(100);

const DEFAULT_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(1);
const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::ZERO;
const DEFAULT_INTERIM_STRATEGY: DeepgramInterimStrategy = DeepgramInterimStrategy::Index;
const DEFAULT_INTERIM_TIMING_THRESHOLD: gst::ClockTime = gst::ClockTime::from_mseconds(40);
const DEFAULT_DIARIZATION: bool = false;
const DEFAULT_API_KEY: Option<&str> = None;
const DEFAULT_INPUT_LANG_CODE: &str = "en";

#[derive(Debug, Clone)]
pub(super) struct Settings {
    latency: gst::ClockTime,
    lateness: gst::ClockTime,
    interim_strategy: DeepgramInterimStrategy,
    interim_timing_threshold: gst::ClockTime,
    diarization: bool,
    api_key: Option<String>,
    language_code: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            lateness: DEFAULT_LATENESS,
            interim_strategy: DEFAULT_INTERIM_STRATEGY,
            interim_timing_threshold: DEFAULT_INTERIM_TIMING_THRESHOLD,
            diarization: DEFAULT_DIARIZATION,
            api_key: DEFAULT_API_KEY.map(String::from),
            language_code: DEFAULT_INPUT_LANG_CODE.to_string(),
        }
    }
}

struct State {
    // (live, min, max)
    upstream_latency: Option<(bool, gst::ClockTime, Option<gst::ClockTime>)>,
    first_buffer_pts: Option<gst::ClockTime>,
    discont: bool,
    client: Option<Deepgram>,
    in_segment: Option<gst::FormattedSegment<gst::ClockTime>>,
    out_segment: Option<gst::FormattedSegment<gst::ClockTime>>,
    audio_tx: Option<mpsc::Sender<gst::Buffer>>,
    result_tx: Option<std::sync::mpsc::Sender<TranscriptOutput>>,
    receive_handle: Option<tokio::task::JoinHandle<()>>,
    interim_index: usize,
    interim_start_time: Option<gst::ClockTime>,
    seqnum: gst::Seqnum,
    last_result: Option<StreamResponse>,
    last_speaker: Option<i32>,
    draining: bool,
    audio_info: Option<gst_audio::AudioInfo>,
    input_times: VecDeque<(gst::ClockTime, gst::ClockTime)>,
    observed_max_delay: gst::ClockTime,
}

impl Default for State {
    fn default() -> Self {
        Self {
            upstream_latency: None,
            first_buffer_pts: None,
            discont: true,
            client: None,
            in_segment: None,
            out_segment: None,
            audio_tx: None,
            result_tx: None,
            receive_handle: None,
            interim_index: 0,
            interim_start_time: None,
            seqnum: gst::Seqnum::next(),
            last_result: None,
            last_speaker: None,
            draining: false,
            audio_info: None,
            input_times: VecDeque::default(),
            observed_max_delay: gst::ClockTime::ZERO,
        }
    }
}

// Locking order: state -> settings
pub struct Transcriber {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
    state_cond: Condvar,
    settings: Mutex<Settings>,
}

#[derive(Debug)]
enum TranscriptOutput {
    Event(gst::Event),
    Item(gst::Buffer),
    Eos,
}

impl Transcriber {
    fn trim_input_times(
        &self,
        input_times: &mut VecDeque<(gst::ClockTime, gst::ClockTime)>,
        pts: gst::ClockTime,
    ) -> Option<gst::ClockTime> {
        let mut iter = input_times.iter().peekable();

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

        input_times.drain(..pos);

        input_running_time
    }

    fn upstream_latency(&self) -> Option<(bool, gst::ClockTime, Option<gst::ClockTime>)> {
        if let Some(latency) = self.state.lock().unwrap().upstream_latency {
            return Some(latency);
        }

        let mut peer_query = gst::query::Latency::new();

        let ret = self.sinkpad.peer_query(&mut peer_query);

        if ret {
            let upstream_latency = peer_query.result();
            gst::info!(
                CAT,
                imp = self,
                "queried upstream latency: {upstream_latency:?}"
            );

            self.state.lock().unwrap().upstream_latency = Some(upstream_latency);

            Some(upstream_latency)
        } else {
            gst::trace!(CAT, imp = self, "could not query upstream latency");

            None
        }
    }

    fn drain<'a>(
        &'a self,
        mut state: MutexGuard<'a, State>,
    ) -> Result<MutexGuard<'a, State>, gst::FlowError> {
        let Some(mut audio_tx) = state.audio_tx.take() else {
            gst::debug!(CAT, imp = self, "Flushing");
            return Err(gst::FlowError::Flushing);
        };

        state.draining = true;

        drop(state);

        let _ = RUNTIME
            .block_on(audio_tx.send(gst::Buffer::new()))
            .map_err(|err| {
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Failed,
                    ["Streaming failed: {err}"]
                );
                gst::FlowError::Error
            });

        let mut state = self.state.lock().unwrap();

        while (self.srcpad.task_state() == gst::TaskState::Started) && state.draining {
            state = self.state_cond.wait(state).unwrap();
        }

        Ok(self.disconnect(state))
    }

    fn process_result(
        &self,
        result: &StreamResponse,
    ) -> Result<bool, std::sync::mpsc::SendError<TranscriptOutput>> {
        let now = self.obj().current_running_time();
        let upstream_latency = self.upstream_latency();

        let mut state_guard = self.state.lock().unwrap();
        let state: &mut State = &mut state_guard;
        let settings = self.settings.lock().unwrap();

        let upstream_min_latency = upstream_latency
            .map(|upstream_latency| upstream_latency.1)
            .unwrap_or(gst::ClockTime::MAX);
        let upstream_is_live = upstream_latency
            .map(|upstream_latency| upstream_latency.0)
            .unwrap_or(false);

        let Some(ref result_tx) = state.result_tx else {
            return Ok(true);
        };

        let (is_final, channel, speech_final, start, duration) = match result {
            StreamResponse::TranscriptResponse {
                start,
                duration,
                is_final,
                speech_final,
                channel,
                ..
            } => (is_final, channel, speech_final, start, duration),
            StreamResponse::TerminalResponse { .. } => {
                result_tx.send(TranscriptOutput::Eos)?;
                return Ok(true);
            }
            _ => {
                return Ok(true);
            }
        };

        let out_segment = state
            .out_segment
            .as_mut()
            .expect("segment received before result");

        let mut do_notify_delay = false;

        if let Some(alternative) = channel.alternatives.first() {
            for (idx, item) in alternative.words.iter().enumerate() {
                let dg_start_time: gst::ClockTime =
                    ((item.start * 1_000_000_000.0) as u64).nseconds();
                let dg_end_time: gst::ClockTime = ((item.end * 1_000_000_000.0) as u64).nseconds();

                let pts = dg_start_time + state.first_buffer_pts.unwrap();

                match settings.interim_strategy {
                    DeepgramInterimStrategy::Timing => {
                        if let Some(interim_start_time) = state.interim_start_time {
                            if dg_start_time
                                <= interim_start_time + settings.interim_timing_threshold
                            {
                                continue;
                            }
                        }
                    }
                    DeepgramInterimStrategy::Index => {
                        if idx < state.interim_index {
                            continue;
                        }
                    }
                    _ => (),
                }

                let rtime = out_segment.to_running_time(pts).unwrap();

                if !is_final {
                    let Some(now) = now else {
                        return Ok(*is_final);
                    };

                    if rtime + settings.latency + upstream_min_latency > now + GRANULARITY {
                        gst::trace!(
                            CAT,
                            imp = self,
                            "interim item {item:?} still has time to stabilize, {} + {} + {} > {} + {}",
                            rtime,
                            settings.latency,
                            upstream_min_latency,
                            now,
                            GRANULARITY
                        );
                        break;
                    }

                    gst::log!(CAT, imp = self, "interim {item:?} needs to be output now");
                } else {
                    gst::log!(CAT, imp = self, "{item:?} is final");
                }

                let duration = dg_end_time.saturating_sub(dg_start_time);

                if let Some(position) = out_segment.position() {
                    if pts.cmp(&position) == std::cmp::Ordering::Greater {
                        gst::log!(
                            CAT,
                            imp = self,
                            "position lagging, queuing gap \
                            with pts {position} and duration {}",
                            pts - position
                        );
                        result_tx.send(TranscriptOutput::Event(
                            gst::event::Gap::builder(position)
                                .duration(pts - position)
                                .seqnum(state.seqnum)
                                .build(),
                        ))?;
                    }
                }

                if item.speaker != state.last_speaker {
                    result_tx.send(TranscriptOutput::Event(
                        gst::event::CustomDownstream::builder(
                            gst::Structure::builder("rstranscribe/speaker-change")
                                .field("speaker", item.speaker.map(|n| n.to_string()))
                                .build(),
                        )
                        .build(),
                    ))?;
                }

                let punctuated_word = item.punctuated_word.as_ref().expect("punctuation enabled");

                result_tx.send(TranscriptOutput::Event(
                    gst::event::CustomUpstream::builder(
                        gst::Structure::builder("rstranscribe/new-item")
                            .field("speaker", item.speaker.map(|n| n.to_string()))
                            .field("content", punctuated_word)
                            .field("running-time", rtime)
                            .field("duration", duration)
                            .build(),
                    )
                    .build(),
                ))?;

                state.last_speaker = item.speaker;

                gst::debug!(
                    CAT,
                    imp = self,
                    "queuing item {} with pts {pts} \
                    and duration {duration}",
                    punctuated_word,
                );

                out_segment.set_position(Some(pts + duration));

                let mut buf = gst::Buffer::from_mut_slice(punctuated_word.clone().into_bytes());
                {
                    let buf_mut = buf.get_mut().unwrap();
                    buf_mut.set_pts(pts);
                    buf_mut.set_duration(duration);

                    if state.discont {
                        buf_mut.set_flags(gst::BufferFlags::DISCONT);
                        state.discont = false;
                    }
                }

                result_tx.send(TranscriptOutput::Item(buf))?;

                if let Some(input_running_time) = self.trim_input_times(&mut state.input_times, pts)
                {
                    if let Some(now) = now {
                        let item_delay = now.saturating_sub(input_running_time);

                        if item_delay > state.observed_max_delay {
                            state.observed_max_delay = item_delay;
                            do_notify_delay = true;
                        }
                    }
                }

                state.interim_index = idx;
                state.interim_start_time = Some(dg_start_time);

                gst::trace!(
                    CAT,
                    imp = self,
                    "partial index is now {}",
                    state.interim_index
                );
            }
        }

        if *speech_final {
            result_tx.send(TranscriptOutput::Event(
                gst::event::CustomDownstream::builder(
                    gst::Structure::builder("rstranscribe/final-transcript").build(),
                )
                .build(),
            ))?;
        }

        if *is_final {
            gst::log!(CAT, imp = self, "result was final, partial index reset");

            state.interim_index = 0;
            state.interim_start_time = None;

            let transcript_start_time: gst::ClockTime =
                ((start * 1_000_000_000.0) as u64).nseconds();
            let transcript_duration: gst::ClockTime =
                ((duration * 1_000_000_000.0) as u64).nseconds();

            let transcript_end_pts =
                transcript_start_time + state.first_buffer_pts.unwrap() + transcript_duration;

            if let Some(position) = out_segment.position() {
                if transcript_end_pts.cmp(&position) == std::cmp::Ordering::Greater {
                    gst::log!(
                        CAT,
                        imp = self,
                        "position lagging, queuing gap \
                        with pts {position} and duration {}",
                        transcript_end_pts - position
                    );
                    result_tx.send(TranscriptOutput::Event(
                        gst::event::Gap::builder(position)
                            .duration(transcript_end_pts - position)
                            .seqnum(state.seqnum)
                            .build(),
                    ))?;
                }
            }

            out_segment.set_position(Some(transcript_end_pts));
        }

        let observed_max_delay = state.observed_max_delay;
        let (latency, lateness) = (settings.latency, settings.lateness);

        drop(state_guard);
        drop(settings);

        if do_notify_delay {
            self.obj().notify("max-observed-delay");
            if observed_max_delay > latency + lateness {
                let details = gst::Structure::builder("deepgramtranscriber/excessive-delay")
                    .field("new-observed-max-delay", observed_max_delay)
                    .build();
                if upstream_is_live {
                    gst::element_warning!(
                        self.obj(),
                        gst::CoreError::Clock,
                        ["Maximum observed delay {} exceeds configured lateness + latency", observed_max_delay],
                        details: details
                    );
                }
            }
        }

        Ok(*is_final)
    }

    fn pause_srcpad_task(&self) {
        gst::info!(CAT, imp = self, "no more results, pausing",);
        let _ = self.srcpad.pause_task();

        self.state_cond.notify_one();
    }

    fn do_eos(&self) {
        let (seqnum, draining) = {
            let state = self.state.lock().unwrap();
            (state.seqnum, state.draining)
        };

        if !draining {
            let _ = self
                .srcpad
                .push_event(gst::event::Eos::builder().seqnum(seqnum).build());
        }

        self.pause_srcpad_task();
    }

    fn start_srcpad_task(&self, state: &mut State) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "starting source pad task");

        let (result_tx, result_rx) = std::sync::mpsc::channel::<TranscriptOutput>();
        state.result_tx = Some(result_tx);

        let this_weak = self.downgrade();
        let res = self.srcpad.start_task(move || loop {
            let Some(this) = this_weak.upgrade() else {
                break;
            };

            match result_rx.recv() {
                Ok(TranscriptOutput::Item(buffer)) => {
                    gst::debug!(CAT, imp = this, "pushing buffer {buffer:?}");

                    if let Err(err) = this.srcpad.push(buffer) {
                        if err != gst::FlowError::Flushing {
                            gst::element_error!(
                                this.obj(),
                                gst::StreamError::Failed,
                                ["Streaming failed: {}", err]
                            );
                        }
                        this.pause_srcpad_task();
                    }
                }
                Ok(TranscriptOutput::Event(event)) => {
                    gst::debug!(CAT, imp = this, "pushing event {event:?}");

                    if event.is_downstream() {
                        this.srcpad.push_event(event);
                    } else {
                        this.sinkpad.push_event(event);
                    }
                }
                Ok(TranscriptOutput::Eos) | Err(_) => {
                    this.do_eos();
                    break;
                }
            }
        });

        if res.is_err() {
            return Err(gst::loggable_error!(CAT, "Failed to start pad task"));
        }

        gst::debug!(CAT, imp = self, "started source pad task");

        Ok(())
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::trace!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        match event.view() {
            StreamStart(_) => {
                self.state.lock().unwrap().seqnum = event.seqnum();

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            FlushStart(_) => {
                gst::debug!(CAT, imp = self, "received flush start, disconnecting");
                let ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                let _ = self.state.lock().unwrap().result_tx.take();
                self.pause_srcpad_task();
                drop(self.disconnect(self.state.lock().unwrap()));
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

                let lateness = self.settings.lock().unwrap().lateness;

                let mut state = self.state.lock().unwrap();

                if let Some(ref in_segment) = state.in_segment {
                    // If segment changes in any other way than the position
                    // we'll have to drain first.
                    let mut compare_segment = segment.clone();
                    compare_segment.set_position(in_segment.position());
                    if *in_segment != compare_segment {
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
                }

                gst::info!(CAT, imp = self, "input segment is now {:?}", segment);

                state.in_segment = Some(segment.clone());

                let mut out_segment = segment.clone();

                out_segment.set_base(segment.base().unwrap_or(gst::ClockTime::ZERO) + lateness);

                state.out_segment = Some(out_segment.clone());

                let out_event = gst::event::Segment::builder(&out_segment)
                    .seqnum(state.seqnum)
                    .build();

                drop(state);

                self.srcpad.push_event(out_event)
            }
            Caps(_) => {
                let caps = self.srcpad.pad_template_caps();

                let event = gst::event::Caps::builder(&caps)
                    .seqnum(self.state.lock().unwrap().seqnum)
                    .build();

                self.srcpad.push_event(event)
            }
            Eos(_) => {
                let Some(mut audio_tx) = self.state.lock().unwrap().audio_tx.take() else {
                    gst::debug!(CAT, obj = pad, "Flushing");
                    return true;
                };

                let _ = RUNTIME
                    .block_on(audio_tx.send(gst::Buffer::new()))
                    .map_err(|err| {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Failed,
                            ["Streaming failed: {err}"]
                        );
                        gst::FlowError::Error
                    });

                self.state.lock().unwrap().audio_tx = Some(audio_tx);

                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling {buffer:?}");

        let now = self.obj().current_running_time();

        self.ensure_connection().map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Failed,
                ["Streaming failed: {}", err]
            );
            gst::FlowError::Error
        })?;

        {
            let mut state = self.state.lock().unwrap();

            if buffer.flags().contains(gst::BufferFlags::DISCONT) {
                gst::info!(
                    CAT,
                    imp = self,
                    "buffer is discont, storing pending discont"
                );
                state.discont = true;
            }

            let Some(segment) = state.in_segment.as_ref() else {
                gst::warning!(CAT, imp = self, "dropping buffer before segment");
                return Ok(gst::FlowSuccess::Ok);
            };

            let Some(audio_info) = state.audio_info.as_ref() else {
                gst::warning!(CAT, imp = self, "dropping buffer before caps");
                return Ok(gst::FlowSuccess::Ok);
            };

            buffer = match gst_audio::audio_buffer_clip(
                buffer,
                segment.upcast_ref(),
                audio_info.rate(),
                audio_info.bpf(),
            ) {
                None => {
                    gst::warning!(CAT, imp = self, "dropping buffer outside segment");
                    return Ok(gst::FlowSuccess::Ok);
                }
                Some(buffer) => buffer,
            };

            let Some(pts) = buffer.pts() else {
                gst::warning!(CAT, imp = self, "dropping buffer without a PTS");
                return Ok(gst::FlowSuccess::Ok);
            };

            if state.first_buffer_pts.is_none() {
                state.first_buffer_pts = Some(pts);
            }
            if let Some(now) = now {
                state.input_times.push_back((pts, now));
            }
        }

        let Some(mut audio_tx) = self.state.lock().unwrap().audio_tx.take() else {
            gst::debug!(CAT, obj = pad, "Flushing");
            return Err(gst::FlowError::Flushing);
        };

        RUNTIME.block_on(audio_tx.send(buffer)).map_err(|err| {
            gst::element_imp_error!(self, gst::StreamError::Failed, ["Streaming failed: {err}"]);
            gst::FlowError::Error
        })?;

        self.state.lock().unwrap().audio_tx = Some(audio_tx);

        Ok(gst::FlowSuccess::Ok)
    }

    fn ensure_connection(&self) -> Result<(), gst::ErrorMessage> {
        let Some(audio_info) = self
            .sinkpad
            .current_caps()
            .and_then(|caps| gst_audio::AudioInfo::from_caps(&caps).ok())
        else {
            return Err(gst::error_msg!(
                gst::CoreError::Failed,
                ["No caps set on sinkpad"]
            ));
        };

        let sample_rate = audio_info.rate();
        let channels = audio_info.channels();

        let mut state = self.state.lock().unwrap();

        state.audio_info = Some(audio_info);

        if state.client.is_none() {
            let (use_interim_results, diarize, api_key, language_code) = {
                let settings = self.settings.lock().unwrap();

                let Some(api_key) = settings.api_key.clone() else {
                    return Err(gst::error_msg!(
                        gst::CoreError::Failed,
                        ["An API key is required"]
                    ));
                };

                (
                    !matches!(settings.interim_strategy, DeepgramInterimStrategy::Disabled),
                    settings.diarization,
                    api_key,
                    settings.language_code.clone(),
                )
            };

            let client = Deepgram::new(api_key).map_err(|err| {
                let err = format!("Failed to create deepgram client: {err}");
                gst::error!(CAT, imp = self, "{err}");
                gst::error_msg!(gst::LibraryError::Init, ["{err}"])
            })?;

            let (audio_tx, audio_rx) = mpsc::channel::<gst::Buffer>(1);

            state.audio_tx = Some(audio_tx);
            state.client = Some(client.clone());

            if let Err(err) = self.start_srcpad_task(&mut state) {
                gst::error!(CAT, imp = self, "Failed to start srcpad task: {}", err);
                return Err(gst::error_msg!(
                    gst::CoreError::Failed,
                    ["Failed to start srcpad task: {err}"]
                ));
            }

            let this_weak = self.downgrade();
            state.receive_handle = Some(RUNTIME.spawn(async move {
                let mut results = if let Some(this) = this_weak.upgrade() {
                    gst::info!(CAT, imp = this, "establishing connection");

                    match client
                        .transcription()
                        .stream_request_with_options(
                            OptionsBuilder::new()
                                .punctuate(true)
                                .diarize(diarize)
                                .model(deepgram::common::options::Model::Nova3)
                                .language(language_code.into())
                                .build(),
                        )
                        .keep_alive()
                        .encoding(Encoding::Linear16)
                        .sample_rate(sample_rate)
                        .channels(channels as u16)
                        .interim_results(use_interim_results)
                        .stream(audio_rx.map(|buffer| {
                            let data = buffer.map_readable().unwrap();
                            Ok::<bytes::Bytes, std::sync::mpsc::RecvError>(
                                bytes::Bytes::copy_from_slice(data.as_slice()),
                            )
                        }))
                        .await
                    {
                        Err(err) => {
                            let err = format!("Error while building transcription stream: {err}");
                            gst::error!(CAT, imp = this, "{err}");
                            this.post_error_message(gst::error_msg!(
                                gst::LibraryError::Failed,
                                ["{err}"]
                            ));
                            return;
                        }
                        Ok(results) => {
                            gst::info!(CAT, imp = this, "connection established!");

                            results
                        }
                    }
                } else {
                    return;
                };

                loop {
                    let result = match tokio::time::timeout(
                        std::time::Duration::from_millis(GRANULARITY.mseconds()),
                        results.next(),
                    )
                    .await
                    {
                        Ok(Some(Ok(result))) => result,
                        Ok(Some(Err(err))) => {
                            let err = format!("Transcribe ws stream error: {err}",);
                            if let Some(this) = this_weak.upgrade() {
                                gst::error!(CAT, imp = this, "{err}");
                                this.post_error_message(gst::error_msg!(
                                    gst::LibraryError::Failed,
                                    ["{err}"]
                                ));
                            };
                            break;
                        }
                        Ok(None) => {
                            if let Some(this) = this_weak.upgrade() {
                                gst::debug!(CAT, imp = this, "Transcriber loop sending EOS");
                            }
                            break;
                        }
                        Err(_) => {
                            let Some(this) = this_weak.upgrade() else {
                                break;
                            };

                            let last_result = this.state.lock().unwrap().last_result.take();

                            if let Some(last_result) = last_result {
                                last_result
                            } else {
                                continue;
                            }
                        }
                    };

                    let Some(this) = this_weak.upgrade() else {
                        break;
                    };

                    let is_final = match this.process_result(&result) {
                        Err(_) => {
                            gst::info!(CAT, imp = this, "Result tx closed");
                            break;
                        }
                        Ok(is_final) => is_final,
                    };

                    if !is_final {
                        this.state.lock().unwrap().last_result = Some(result);
                    } else {
                        this.state.lock().unwrap().last_result = None;
                    }
                }
            }));
        } else {
            gst::trace!(CAT, imp = self, "connection was already established");
        }

        Ok(())
    }

    fn disconnect<'a>(&'a self, mut state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        if let Some(handle) = state.receive_handle.take() {
            gst::info!(CAT, imp = self, "aborting result reception");
            handle.abort();
        }

        *state = State::default();

        self.state_cond.notify_one();

        drop(state);

        let _ = self.srcpad.stop_task();

        gst::info!(CAT, imp = self, "disconnected");

        self.state.lock().unwrap()
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        match query.view_mut() {
            gst::QueryViewMut::Latency(ref mut q) => {
                self.state.lock().unwrap().upstream_latency = None;

                if let Some(upstream_latency) = self.upstream_latency() {
                    let (live, min, max) = upstream_latency;
                    let our_latency = self.settings.lock().unwrap().latency;

                    if live {
                        q.set(true, min + our_latency, max.map(|max| max + our_latency));
                    } else {
                        q.set(live, min, max);
                    }
                    true
                } else {
                    false
                }
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Transcriber {
    const NAME: &'static str = "GstDeepgramTranscriber";
    type Type = super::Transcriber;
    type ParentType = gst::Element;

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

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .query_function(|pad, parent, query| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |transcriber| transcriber.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Default::default(),
            settings: Default::default(),
            state_cond: Condvar::new(),
        }
    }
}

impl ObjectImpl for Transcriber {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow AWS transcribe")
                    .default_value(DEFAULT_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Lateness")
                    .blurb("Amount of milliseconds to introduce as lateness")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default(
                    "interim-strategy",
                    DEFAULT_INTERIM_STRATEGY,
                )
                .nick("Interim Strategy")
                .blurb("Defines how interim results should be used, if at all")
                .mutable_ready()
                .build(),
                glib::ParamSpecUInt::builder("interim-timing-threshold")
                    .nick("Interim Timing Threshold")
                    .blurb(
                        "Amount of milliseconds to consider a word in an interim result new. \
                        Only used with interim-strategy=timing",
                    )
                    .default_value(DEFAULT_INTERIM_TIMING_THRESHOLD.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("diarization")
                    .nick("Diarization")
                    .blurb("Whether diarization should be enabled")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("api-key")
                    .nick("API Key")
                    .blurb("ElevenLabs API Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb(
                        "The Language of the Stream, see \
                        <https://developers.deepgram.com/docs/language>",
                    )
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-observed-delay")
                    .nick("Maximum Observed Delay")
                    .blurb("Maximum delay observed between the sending of an audio sample and the reception of an item")
                    .default_value(0)
                    .read_only()
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
            "latency" => {
                let mut settings = self.settings.lock().unwrap();
                settings.latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "interim-strategy" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interim_strategy = value
                    .get::<DeepgramInterimStrategy>()
                    .expect("type checked upstream");
            }
            "interim-timing-threshold" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interim_timing_threshold = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "diarization" => {
                let mut settings = self.settings.lock().unwrap();
                settings.diarization = value.get::<bool>().expect("type checked upstream");
            }
            "api-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.api_key = value.get().expect("type checked upstream");
            }
            "language-code" => {
                let language_code: String = value.get().expect("type checked upstream");
                let mut settings = self.settings.lock().unwrap();
                settings.language_code = language_code;
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
            "interim-strategy" => {
                let settings = self.settings.lock().unwrap();
                settings.interim_strategy.to_value()
            }
            "interim-timing-threshold" => {
                let settings = self.settings.lock().unwrap();
                (settings.interim_timing_threshold.mseconds() as u32).to_value()
            }
            "diarization" => {
                let settings = self.settings.lock().unwrap();
                settings.diarization.to_value()
            }
            "api-key" => {
                let settings = self.settings.lock().unwrap();
                settings.api_key.to_value()
            }
            "language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.language_code.to_value()
            }
            "max-observed-delay" => {
                let state = self.state.lock().unwrap();
                (state.observed_max_delay.mseconds() as u32).to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Transcriber {}

impl ElementImpl for Transcriber {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Transcriber",
                "Audio/Text/Filter",
                "Speech to Text filter, using Deepgram streaming transcription API",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst_audio::AudioCapsBuilder::new()
                .format(gst_audio::AudioFormat::S16le)
                .rate_list([24_000, 8_000, 16_000, 32_000, 48_000])
                .channels_range(1..)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
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
        gst::log!(CAT, imp = self, "Changing state {transition:?}");

        if transition == gst::StateChange::PausedToReady {
            let mut state = self.state.lock().unwrap();
            let _ = state.result_tx.take();
            drop(self.disconnect(state));
        }

        self.parent_change_state(transition)
    }
}
