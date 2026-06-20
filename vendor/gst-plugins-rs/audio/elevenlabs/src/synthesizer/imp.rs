// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! ElevenLabs TTS element.
//!
//! This element calls ElevenLabs to generate audio speech from text.
//!
//! It makes use of the POST API because one of the design goals is to
//! preserve the exact timestamping of the input data, and while this would
//! be possible with the websockets API it would make the implementation much
//! more complex.
//!
//! Control over the latency would also be made more complex, as ElevenLabs
//! `chunk_length_schedule` would have forced us to use a timeout and send
//! flushing requests from time to time in the absence of input.
//!
//! Example usage with srt file as input:
//!
//! ```
//! gst-launch-1.0 filesrc location=/home/meh/Documents/chaplin-fr-shifted.srt ! \
//! subparse ! clocksync ! queue ! \
//! elevenlabssynthesizer voice-id=kENkNtk0xyzG09WW40xE overflow=shift api-key=XXX ! autoaudiosink
//! ```

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};

use crate::RUNTIME;
use futures::future::{abortable, AbortHandle};
use reqwest::Response;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, Error};

use super::Overflow;

#[cfg(feature = "signalsmith_stretch")]
use signalsmith_stretch::Stretch;

#[derive(serde::Serialize, Debug)]
struct VoiceSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f64>,
}

#[derive(serde::Serialize, Debug)]
struct SendText {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_code: Option<String>,
    model_id: String,
    previous_request_ids: Vec<String>,
    voice_settings: VoiceSettings,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "elevenlabssynthesizer",
        gst::DebugColorFlags::empty(),
        Some("ElevenLabs Text to Speech element"),
    )
});

const DEFAULT_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(2);
const DEFAULT_OVERFLOW: Overflow = Overflow::Clip;
const DEFAULT_API_KEY: Option<&str> = None;
const DEFAULT_VOICE_ID: &str = "9BWtsMINqrJLrRacOk9x"; // Aria
const DEFAULT_MODEL_ID: &str = "eleven_flash_v2_5";
const DEFAULT_LANGUAGE_CODE: Option<&str> = None;
const DEFAULT_RETRY_WITH_SPEED: bool = true;
const DEFAULT_MAX_PREVIOUS_REQUESTS: u32 = 0;
const DEFAULT_MAX_OVERFLOW: gst::ClockTime = gst::ClockTime::from_seconds(0);
const DEFAULT_USE_VOICE_ID_EVENTS: bool = true;

#[derive(Debug, Clone)]
pub(super) struct Settings {
    latency: gst::ClockTime,
    overflow: Overflow,
    api_key: Option<String>,
    voice_id: String,
    model_id: String,
    language_code: Option<String>,
    retry_with_speed: bool,
    max_overflow: gst::ClockTime,
    max_previous_requests: u32,
    use_voice_id_events: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            overflow: DEFAULT_OVERFLOW,
            api_key: DEFAULT_API_KEY.map(String::from),
            voice_id: String::from(DEFAULT_VOICE_ID),
            model_id: String::from(DEFAULT_MODEL_ID),
            language_code: DEFAULT_LANGUAGE_CODE.map(String::from),
            retry_with_speed: DEFAULT_RETRY_WITH_SPEED,
            max_overflow: DEFAULT_MAX_OVERFLOW,
            max_previous_requests: DEFAULT_MAX_PREVIOUS_REQUESTS,
            use_voice_id_events: DEFAULT_USE_VOICE_ID_EVENTS,
        }
    }
}

struct State {
    out_segment: gst::FormattedSegment<gst::ClockTime>,
    client: Option<Client>,
    send_abort_handle: Option<AbortHandle>,
    previous_request_ids: VecDeque<String>,
    outcaps: Option<gst::Caps>,
    // (live, min, max)
    upstream_latency: Option<(bool, gst::ClockTime, Option<gst::ClockTime>)>,
    #[cfg(feature = "signalsmith_stretch")]
    stretch: Option<Stretch>,
    current_speaker: Option<String>,
    speaker_map: HashMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            out_segment: gst::FormattedSegment::new(),
            client: None,
            send_abort_handle: None,
            previous_request_ids: VecDeque::new(),
            outcaps: None,
            upstream_latency: None,
            #[cfg(feature = "signalsmith_stretch")]
            stretch: None,
            current_speaker: None,
            speaker_map: HashMap::default(),
        }
    }
}

pub struct Synthesizer {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

async fn send_first_request(
    client: Client,
    voice_id: String,
    model_id: String,
) -> Result<Response, Error> {
    let url =
        format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}?output_format=pcm_22050");
    let body = serde_json::to_string(&SendText {
        text: String::from("first"),
        language_code: None,
        model_id,
        voice_settings: VoiceSettings { speed: None },
        previous_request_ids: vec![],
    })
    .unwrap();

    client
        .post(url)
        .body(body)
        .send()
        .await
        .map_err(|err| anyhow!("failed sending request: {err}"))
}

impl Synthesizer {
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

    fn negotiate(&self) -> Result<gst::Caps, Error> {
        let mut allowed_caps = self
            .srcpad
            .allowed_caps()
            .unwrap_or_else(|| self.srcpad.pad_template_caps());

        allowed_caps.fixate();

        self.state.lock().unwrap().outcaps = Some(allowed_caps.clone());

        gst::debug!(CAT, imp = self, "negotiated output caps {}", allowed_caps);

        Ok(allowed_caps)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {event:?}");

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
                self.state.lock().unwrap().out_segment = segment;
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            Caps(c) => {
                let caps = match self.negotiate() {
                    Ok(caps) => caps,
                    Err(err) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["negotiation failed: {err}"]
                        );
                        return false;
                    }
                };

                #[cfg(feature = "signalsmith_stretch")]
                if matches!(self.settings.lock().unwrap().overflow, Overflow::Compress) {
                    let out_info = gst_audio::AudioInfo::from_caps(&caps).unwrap();
                    self.state.lock().unwrap().stretch = Some(
                        signalsmith_stretch::Stretch::preset_default(1, out_info.rate()),
                    );
                }

                let event = gst::event::Caps::builder(&caps).seqnum(c.seqnum()).build();

                self.srcpad.push_event(event)
            }
            Gap(g) => {
                let (pts, duration) = g.get();

                let mut state = self.state.lock().unwrap();

                let new_gap_event = if let Some(position) = state.out_segment.position() {
                    if let Some(duration) = duration {
                        let end_pts = pts + duration;

                        if end_pts > position {
                            // Output our own gap event that starts at our current position
                            Some(
                                gst::event::Gap::builder(position)
                                    .duration(end_pts - position)
                                    .seqnum(event.seqnum())
                                    .build(),
                            )
                        } else {
                            // We have already advanced past this gap's end
                            None
                        }
                    } else if pts > position {
                        Some(gst::event::Gap::builder(pts).seqnum(event.seqnum()).build())
                    } else {
                        // This duration-less gap was older that our current position, do
                        // nothing
                        None
                    }
                } else {
                    // Position wasn't set yet, the gap can be forwarded unchanged
                    Some(event.clone())
                };

                if let Some(ref event) = new_gap_event {
                    let Gap(gap) = event.view() else {
                        unreachable!()
                    };
                    let (new_pts, new_duration) = gap.get();

                    gst::log!(
                        CAT,
                        imp = self,
                        "pushing gap with pts {new_pts} and duration {new_duration:?}"
                    );

                    state.out_segment.set_position(match new_duration {
                        Some(new_duration) => new_duration + new_pts,
                        _ => new_pts,
                    });
                }

                drop(state);

                if let Some(event) = new_gap_event {
                    gst::Pad::event_default(pad, Some(&*self.obj()), event)
                } else {
                    true
                }
            }
            CustomDownstreamOob(c) => {
                let Some(s) = c.structure() else {
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                if s.name().as_str() == "elevenlabs/speaker-voice" {
                    let Ok(speaker) = s.get::<String>("speaker") else {
                        return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                    };
                    let Ok(voice_id) = s.get::<String>("voice-id") else {
                        return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                    };

                    gst::log!(
                        CAT,
                        imp = self,
                        "storing voice id {voice_id} for speaker {speaker}"
                    );

                    self.state
                        .lock()
                        .unwrap()
                        .speaker_map
                        .insert(speaker, voice_id);
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            CustomDownstream(c) => {
                let Some(s) = c.structure() else {
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                if s.name().as_str() == "rstranscribe/speaker-change" {
                    let Ok(speaker) = s.get::<String>("speaker") else {
                        return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                    };

                    self.state.lock().unwrap().current_speaker = Some(speaker);
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    async fn send(
        &self,
        content: String,
        mut pts: gst::ClockTime,
        input_duration: gst::ClockTime,
    ) -> Result<Option<gst::Buffer>, Error> {
        let (
            default_voice_id,
            model_id,
            language_code,
            retry_with_speed,
            our_latency,
            max_previous_requests,
            use_voice_id_events,
        ) = {
            let settings = self.settings.lock().unwrap();

            (
                settings.voice_id.clone(),
                settings.model_id.clone(),
                settings.language_code.as_ref().cloned(),
                settings.retry_with_speed,
                settings.latency,
                settings.max_previous_requests,
                settings.use_voice_id_events,
            )
        };

        let upstream_latency = self.upstream_latency();

        let (client, previous_request_ids, out_info, out_segment, voice_id) = {
            let state = self.state.lock().unwrap();

            let Some(client) = state.client.as_ref().cloned() else {
                return Ok(None);
            };

            let voice_id = if use_voice_id_events {
                state
                    .current_speaker
                    .as_ref()
                    .and_then(|speaker| state.speaker_map.get(speaker).cloned())
                    .unwrap_or(default_voice_id.clone())
            } else {
                default_voice_id.clone()
            };

            (
                client,
                state.previous_request_ids.clone(),
                gst_audio::AudioInfo::from_caps(state.outcaps.as_ref().expect("negotiated"))
                    .unwrap(),
                state.out_segment.clone(),
                voice_id,
            )
        };

        let output_format = format!("pcm_{}", out_info.rate());

        let bytes_per_second = (out_info.bpf() * out_info.rate()) as u64;

        let max_expected_bytes = input_duration
            .nseconds()
            .mul_div_floor(bytes_per_second, 1_000_000_000)
            .unwrap()
            / 2
            * 2;

        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{voice_id}?output_format={output_format}"
        );

        gst::debug!(CAT, imp = self, "sending request to {url} for {content}");

        let mut speed: Option<f64> = None;

        let (mut bytes, request_id) = loop {
            let job = {
                let body = serde_json::to_string(&SendText {
                    text: content.clone(),
                    language_code: language_code.clone(),
                    model_id: model_id.clone(),
                    voice_settings: VoiceSettings { speed },
                    previous_request_ids: previous_request_ids.clone().into(),
                })
                .unwrap();
                client.post(&url).body(body).send()
            };

            let resp = job.await.map_err(|err| {
                gst::error!(CAT, imp = self, "Failed sending text chunk: {}", err);
                err
            })?;

            gst::trace!(CAT, "response: {:?}", resp);

            if !resp.status().is_success() {
                gst::error!(CAT, imp = self, "Request failed: {}", resp.status());
                let status = resp.status();
                if let Ok(text) = resp.text().await {
                    return Err(anyhow!("Request failed: {} ({})", status, text));
                } else {
                    return Err(anyhow!("Request failed: {}", status));
                }
            }

            let request_id = resp
                .headers()
                .get("request-id")
                .and_then(|h| h.to_str().ok())
                .map(|id| id.to_string());

            let bytes = resp
                .bytes()
                .await
                .map_err(|err| anyhow!("Failed getting response bytes: {err}"))?;

            let n_bytes = bytes.len() as u64;

            gst::trace!(CAT, "n_bytes with speed {:?}: {}", speed, n_bytes);

            if retry_with_speed && speed.is_none() && n_bytes > max_expected_bytes {
                let new_speed: f64 = (n_bytes as f64 / max_expected_bytes as f64).min(1.2);
                gst::debug!(
                    CAT,
                    "Got larger duration than expected ({} > {}), retrying with speed {}",
                    bytes.len(),
                    max_expected_bytes,
                    new_speed
                );
                speed = Some(new_speed)
            } else {
                break (bytes, request_id);
            }
        };

        let overflow = self.settings.lock().unwrap().overflow;

        if matches!(overflow, Overflow::Clip) {
            gst::debug!(
                CAT,
                "Received {} bytes, max expected {}",
                bytes.len(),
                max_expected_bytes
            );

            bytes.truncate(max_expected_bytes as usize);
        }

        #[cfg(feature = "signalsmith_stretch")]
        let mut compression_factor: Option<f64> = None;
        #[cfg(not(feature = "signalsmith_stretch"))]
        let compression_factor: Option<f64> = None;

        #[cfg(feature = "signalsmith_stretch")]
        if matches!(overflow, Overflow::Compress) {
            let max_overflow = self.settings.lock().unwrap().max_overflow;
            let overflow_budget = match self.state.lock().unwrap().out_segment.position() {
                Some(position) => {
                    if pts > position {
                        max_overflow
                    } else {
                        let budget = pts + max_overflow - position;
                        pts = position;
                        budget
                    }
                }
                None => 2 * gst::ClockTime::SECOND,
            };

            gst::debug!(CAT, "Overflow budget: {}", overflow_budget);

            let max_expected_bytes = (input_duration + overflow_budget)
                .nseconds()
                .mul_div_floor(bytes_per_second, 1_000_000_000)
                .unwrap()
                / 2
                * 2;

            gst::log!(
                CAT,
                "max expected bytes for duration {input_duration} is {max_expected_bytes}"
            );

            if bytes.len() > max_expected_bytes as usize {
                let factor = bytes.len() as f64 / max_expected_bytes as f64;

                gst::debug!(
                    CAT,
                    imp = self,
                    "compressing {content} by a factor of {factor}",
                );

                compression_factor = Some(factor);

                let samples: Vec<_> = bytes
                    .chunks_exact(2)
                    .map(|chunk| {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        (sample as f32) / 32768.
                    })
                    .collect();
                let mut output = vec![0.0f32; (max_expected_bytes / 2) as usize];
                let mut state = self.state.lock().unwrap();
                state.stretch.as_mut().unwrap().exact(samples, &mut output);

                bytes.truncate(max_expected_bytes as usize);
                let mut bytes_mut: bytes::BytesMut = bytes.into();

                for (out_bytes, sample) in
                    Iterator::zip(bytes_mut.chunks_exact_mut(2), output.iter())
                {
                    let scaled_sample = f32::clamp(sample * 32_768., -32_768., 32_767.) as i16;
                    let chunk = scaled_sample.to_le_bytes();
                    out_bytes.copy_from_slice(&chunk);
                }

                bytes = bytes_mut.into();
            }
        }

        let mut duration = gst::ClockTime::from_nseconds(
            (bytes.len() as u64)
                .mul_div_round(1_000_000_000, bytes_per_second)
                .unwrap(),
        );

        if duration > input_duration {
            gst::debug!(
                CAT,
                imp = self,
                "received duration is superior to input duration ({duration} > {input_duration})"
            );
        }

        let mut buf = gst::Buffer::from_slice(bytes);
        let mut state = self.state.lock().unwrap();

        if let Some(id) = request_id {
            state.previous_request_ids.push_back(id);
            while state.previous_request_ids.len() > max_previous_requests as usize {
                state.previous_request_ids.pop_front();
            }
        } else {
            gst::warning!(CAT, imp = self, "No request ID, flushing id queue");
            state.previous_request_ids.clear();
        }

        if let Some(position) = state.out_segment.position() {
            if matches!(overflow, Overflow::Shift) && pts < position {
                gst::debug!(
                    CAT,
                    "received pts {pts} < position {position}, shifting forward"
                );
                pts = position;
            }
        }

        let Some(buffer_rtime) = out_segment.to_running_time(pts) else {
            gst::warning!(
                CAT,
                imp = self,
                "buffer PTS {pts} not in segment {out_segment:?}"
            );
            return Ok(None);
        };

        if let Some(upstream_latency) = upstream_latency {
            let (upstream_live, upstream_min, _) = upstream_latency;

            if upstream_live {
                let current_rtime = self
                    .obj()
                    .current_running_time()
                    .expect("upstream is live and should have provided a clock");

                let deadline = buffer_rtime + upstream_min + our_latency;

                if deadline < current_rtime {
                    let delta = current_rtime - deadline;

                    if duration > delta {
                        gst::warning!(
                            CAT,
                            "received running time {buffer_rtime} + {upstream_min} + {our_latency} < current rtime {current_rtime}, shifting forward by {delta}, consider increasing latency"
                        );
                        pts += delta;
                        duration -= delta;
                    } else {
                        gst::warning!(
                            CAT,
                            "received running time {buffer_rtime} + {upstream_min} + {our_latency} < current rtime {current_rtime} and delta {delta} is greater than duration {duration}, dropping, consider increasing latency"
                        );
                        return Ok(None);
                    }
                }
            }
        }

        let discont = state
            .out_segment
            .position()
            .is_none_or(|position| position < pts + duration);

        {
            let buf_mut = buf.get_mut().unwrap();
            buf_mut.set_pts(pts);
            buf_mut.set_duration(duration);

            if let Ok(mut meta) =
                gst::meta::CustomMeta::add(buf_mut, "GstScaletempoTargetDurationMeta")
            {
                meta.mut_structure()
                    .set("duration", input_duration.nseconds());
            }

            if discont {
                gst::debug!(CAT, imp = self, "Marking buffer discont");
                buf_mut.set_flags(gst::BufferFlags::DISCONT);
            }
        }

        state.out_segment.set_position(pts + duration);

        let mut s_builder = gst::Structure::builder("elevenlabs/synthesized-audio")
            .field("content", content)
            .field("pts", pts)
            .field("input-duration", duration)
            .field("actual-duration", duration);

        if let Some(factor) = compression_factor {
            s_builder = s_builder.field("compression-factor", factor)
        }

        let s = s_builder.build();

        drop(state);

        let _ = self
            .obj()
            .post_message(gst::message::Element::builder(s).src(&*self.obj()).build());

        Ok(Some(buf))
    }

    fn do_send(
        &self,
        content: String,
        pts: gst::ClockTime,
        duration: gst::ClockTime,
    ) -> Result<Option<gst::Buffer>, gst::FlowError> {
        let (future, abort_handle) = abortable(self.send(content, pts, duration));

        {
            let mut state = self.state.lock().unwrap();

            if let Some(handle) = state.send_abort_handle.take() {
                handle.abort();
            }

            if state.client.is_none() {
                return Err(gst::FlowError::Flushing);
            }

            state.send_abort_handle = Some(abort_handle);
        }

        let ret = match RUNTIME.block_on(future) {
            Err(_) => {
                gst::debug!(CAT, imp = self, "send aborted, returning flushing");
                Err(gst::FlowError::Flushing)
            }
            Ok(res) => match res {
                Err(e) => {
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Failed,
                        ["Failed sending data: {}", e]
                    );
                    Err(gst::FlowError::Error)
                }
                Ok(buf) => Ok(buf),
            },
        };

        self.state.lock().unwrap().send_abort_handle = None;

        ret
    }

    fn read_buffer(
        &self,
        buffer: &gst::Buffer,
    ) -> Result<(gst::ClockTime, gst::ClockTime, String), Error> {
        let pts = buffer
            .pts()
            .ok_or_else(|| anyhow!("Stream with timestamped buffers required"))?;

        let duration = buffer
            .duration()
            .ok_or_else(|| anyhow!("Buffers of stream need to have a duration"))?;

        let data = buffer
            .map_readable()
            .map_err(|_| anyhow!("Can't map buffer readable"))?;

        let data =
            std::str::from_utf8(&data).map_err(|err| anyhow!("Can't decode utf8: {}", err))?;

        Ok((pts, duration, data.to_owned()))
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {buffer:?}");

        let (pts, duration, data) = self.read_buffer(&buffer).map_err(|err| {
            gst::element_imp_error!(self, gst::StreamError::Failed, ["{}", err]);
            gst::FlowError::Error
        })?;

        let Some(mut outbuf) = self.do_send(data, pts, duration)? else {
            return Ok(gst::FlowSuccess::Ok);
        };

        {
            let outbuf_mut = outbuf.get_mut().unwrap();
            buffer.foreach_meta(|meta| {
                if meta.tags().is_empty() {
                    if let Err(err) =
                        meta.transform(outbuf_mut, &gst::meta::MetaTransformCopy::new(..))
                    {
                        gst::trace!(CAT, imp = self, "Could not copy meta {}: {err}", meta.api());
                    }
                }
                std::ops::ControlFlow::Continue(())
            });
        }

        self.srcpad.push(outbuf)
    }

    fn sink_chain_list(
        &self,
        _pad: &gst::Pad,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(
            CAT,
            imp = self,
            "Handling buffer list with size {}",
            list.len()
        );

        let mut list_pts: Option<gst::ClockTime> = None;
        let mut list_end_pts: Option<gst::ClockTime> = None;
        let mut list_content: Vec<String> = vec![];

        for buffer in list.iter_owned() {
            let (pts, duration, data) = self.read_buffer(&buffer).map_err(|err| {
                gst::element_imp_error!(self, gst::StreamError::Failed, ["{}", err]);
                gst::FlowError::Error
            })?;

            if list_pts.is_none() {
                list_pts = Some(pts);
            }

            list_end_pts = Some(pts + duration);

            list_content.push(data);
        }

        if let Some((pts, end_pts)) = Option::zip(list_pts, list_end_pts) {
            let duration = end_pts.saturating_sub(pts);

            let content = list_content.join(" ");

            let Some(mut outbuf) = self.do_send(content, pts, duration)? else {
                return Ok(gst::FlowSuccess::Ok);
            };

            {
                let outbuf_mut = outbuf.get_mut().unwrap();
                for buffer in list.iter() {
                    buffer.foreach_meta(|meta| {
                        if meta.tags().is_empty() {
                            if let Err(err) =
                                meta.transform(outbuf_mut, &gst::meta::MetaTransformCopy::new(..))
                            {
                                gst::trace!(
                                    CAT,
                                    imp = self,
                                    "Could not copy meta {}: {err}",
                                    meta.api()
                                );
                            }
                        }
                        std::ops::ControlFlow::Continue(())
                    });
                }
            }

            self.srcpad.push(outbuf)
        } else {
            Ok(gst::FlowSuccess::Ok)
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
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));

            state.client = Some(Client::builder().default_headers(headers).build().map_err(
                |err| gst::error_msg!(gst::CoreError::Failed, ["Failed to create client: {err}"]),
            )?);
        }
        Ok(())
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        gst::debug!(CAT, imp = self, "Prepared");

        Ok(())
    }

    fn disconnect(&self) {
        gst::info!(CAT, imp = self, "Disconnecting");
        let mut state = self.state.lock().unwrap();

        state.client = None;

        if let Some(abort_handle) = state.send_abort_handle.take() {
            abort_handle.abort();
        }

        *state = State::default();

        gst::info!(CAT, imp = self, "Disconnected");
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            gst::QueryViewMut::Latency(ref mut q) => {
                let mut peer_query = gst::query::Latency::new();

                let ret = self.sinkpad.peer_query(&mut peer_query);

                if ret {
                    let (live, min, max) = peer_query.result();
                    let our_latency = self.settings.lock().unwrap().latency;

                    if live {
                        q.set(true, min + our_latency, max.map(|max| max + our_latency));
                    } else {
                        q.set(live, min, max);
                    }
                }
                ret
            }
            gst::QueryViewMut::Position(ref mut q) => {
                if q.format() == gst::Format::Time {
                    let state = self.state.lock().unwrap();
                    q.set(
                        state
                            .out_segment
                            .to_stream_time(state.out_segment.position()),
                    );
                    true
                } else {
                    false
                }
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }

    fn post_start(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Start, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }

    fn post_complete(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Complete, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }

    fn post_cancelled(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Canceled, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Synthesizer {
    const NAME: &'static str = "GstElevenLabsSynthesizer";
    type Type = super::Synthesizer;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Synthesizer::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain(pad, buffer),
                )
            })
            .chain_list_function(|pad, parent, list| {
                Synthesizer::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain_list(pad, list),
                )
            })
            .event_function(|pad, parent, event| {
                Synthesizer::catch_panic_pad_function(
                    parent,
                    || false,
                    |imp| imp.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .query_function(|pad, parent, query| {
                Synthesizer::catch_panic_pad_function(
                    parent,
                    || false,
                    |synthesizer| synthesizer.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            settings: Default::default(),
            state: Default::default(),
        }
    }
}

impl ObjectImpl for Synthesizer {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow ElevenLabs")
                    .default_value(DEFAULT_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("overflow", DEFAULT_OVERFLOW)
                    .nick("Overflow")
                    .blurb("Defines how output audio with a longer duration than input text should be handled")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("api-key")
                    .nick("API Key")
                    .blurb("ElevenLabs API Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("voice-id")
                    .nick("Voice ID")
                    .blurb("ElevenLabs Voice ID, see https://elevenlabs.io/app/voice-library")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("model-id")
                    .nick("Model ID")
                    .blurb("ElevenLabs Model ID, see https://help.elevenlabs.io/hc/en-us/articles/21811236079505-How-do-I-find-the-model-ID")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("An optional language code (ISO 639-1), useful with certain models")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("retry-with-speed")
                    .nick("Retry with Speed")
                    .blurb("When synthesis results in larger duration, retry with higher speed")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-overflow")
                    .nick("Max Overflow")
                    .blurb("Amount of milliseconds any given text cue is allowed to overflow \
                        its intended duration. Only used with mode=compress")
                    .default_value(DEFAULT_MAX_OVERFLOW.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-previous-requests")
                    .nick("Max Previous Requests")
                    .blurb("How many previous request IDs to track, see https://elevenlabs.io/docs/api-reference/text-to-speech/convert#request.body.previous_request_ids")
                    .default_value(DEFAULT_MAX_PREVIOUS_REQUESTS)
                    .minimum(0)
                    .maximum(3)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("use-voice-id-events")
                    .nick("Use Voice-id Events")
                    .blurb("Use received elevenlabs/speaker-voice events to pick the current voice")
                    .mutable_ready()
                    .build(),
                /**
                 * GstElevenLabsSynthesizer:speaker-map:
                 *
                 * Set a custom speaker to voice ID map.
                 *
                 * When #GstElevenLabsSynthesizer:use-voice-id-events is set,
                 * the voice ids may still be updated by upstream voice ID
                 * events.
                 */
                glib::ParamSpecBoxed::builder::<gst::Structure>("speaker-map")
                    .nick("Speaker Map")
                    .blurb("Map of speaker to voice id")
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
            "overflow" => {
                let mut settings = self.settings.lock().unwrap();
                settings.overflow = value.get::<Overflow>().expect("type checked upstream");
            }
            "api-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.api_key = value.get().expect("type checked upstream");
            }
            "voice-id" => {
                let mut settings = self.settings.lock().unwrap();
                settings.voice_id = value.get().expect("type checked upstream");
            }
            "model-id" => {
                let mut settings = self.settings.lock().unwrap();
                settings.model_id = value.get().expect("type checked upstream");
            }
            "language-code" => {
                let mut settings = self.settings.lock().unwrap();
                settings.language_code = value.get().expect("type checked upstream");
            }
            "retry-with-speed" => {
                let mut settings = self.settings.lock().unwrap();
                settings.retry_with_speed = value.get().expect("type checked upstream");
            }
            "max-overflow" => {
                let mut settings = self.settings.lock().unwrap();
                settings.max_overflow = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "max-previous-requests" => {
                let mut settings = self.settings.lock().unwrap();
                settings.max_previous_requests = value.get::<u32>().expect("type checked upstream");
            }
            "use-voice-id-events" => {
                let mut settings = self.settings.lock().unwrap();
                settings.use_voice_id_events = value.get().expect("type checked upstream");
            }
            "speaker-map" => {
                let mut state = self.state.lock().unwrap();

                if let Some(s) = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream")
                {
                    for speaker in s.fields() {
                        let Ok(voice_id) = s.get::<String>(speaker) else {
                            gst::error!(
                                CAT,
                                imp = self,
                                "skipping non-string voice id for {speaker}"
                            );
                            continue;
                        };

                        state.speaker_map.insert(speaker.to_string(), voice_id);
                    }
                }
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
            "overflow" => {
                let settings = self.settings.lock().unwrap();
                settings.overflow.to_value()
            }
            "api-key" => {
                let settings = self.settings.lock().unwrap();
                settings.api_key.to_value()
            }
            "voice-id" => {
                let settings = self.settings.lock().unwrap();
                settings.voice_id.to_value()
            }
            "model-id" => {
                let settings = self.settings.lock().unwrap();
                settings.model_id.to_value()
            }
            "language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.language_code.to_value()
            }
            "retry-with-speed" => {
                let settings = self.settings.lock().unwrap();
                settings.retry_with_speed.to_value()
            }
            "max-overflow" => {
                let settings = self.settings.lock().unwrap();
                (settings.latency.mseconds() as u32).to_value()
            }
            "max-previous-requests" => {
                let settings = self.settings.lock().unwrap();
                settings.max_previous_requests.to_value()
            }
            "use-voice-id-events" => {
                let settings = self.settings.lock().unwrap();
                settings.use_voice_id_events.to_value()
            }
            "speaker-map" => gst::Structure::from_iter(
                "application/x-elevenlabs-speaker-map",
                self.state
                    .lock()
                    .unwrap()
                    .speaker_map
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.to_send_value())),
            )
            .to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Synthesizer {}

impl ElementImpl for Synthesizer {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Synthesizer",
                "Audio/Text/Filter",
                "Text to Speech filter, using ElevenLabs",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder_full()
                .structure(
                    gst::Structure::builder("text/x-raw")
                        .field("format", "utf8")
                        .build(),
                )
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
                .rate_list([22_050, 48_000, 44_100, 24_000, 16_000, 8_000])
                .channels(1)
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
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::ReadyToPaused => {
                // Loading up a new voice can cause unpredictable latency
                // (up to 5 seconds or more), so we always emit a first request
                // here, the application can then listen to progress messages
                // before switching to PLAYING
                self.ensure_connection().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;

                let (voice_id, model_id) = {
                    let settings = self.settings.lock().unwrap();

                    (settings.voice_id.clone(), settings.model_id.clone())
                };

                let client = {
                    let state = self.state.lock().unwrap();

                    state.client.as_ref().expect("connected").clone()
                };

                let (future, abort_handle) =
                    abortable(send_first_request(client, voice_id, model_id));

                self.state.lock().unwrap().send_abort_handle = Some(abort_handle);

                self.post_start("request", "first request sent");
                let this_weak = self.downgrade();
                RUNTIME.spawn(async move {
                    let res = future.await;

                    if let Some(this) = this_weak.upgrade() {
                        this.state.lock().unwrap().send_abort_handle = None;
                        match res {
                            Err(_) => {
                                this.post_cancelled("request", "first request cancelled");
                            }
                            _ => {
                                this.post_complete("request", "first request complete");
                            }
                        }
                    }
                });
            }
            gst::StateChange::PausedToReady => {
                self.disconnect();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
