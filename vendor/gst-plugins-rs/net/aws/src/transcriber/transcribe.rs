// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2023 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};

use aws_sdk_transcribestreaming as aws_transcribe;
use aws_sdk_transcribestreaming::error::ProvideErrorMetadata;
use aws_sdk_transcribestreaming::types;

use futures::channel::mpsc;
use futures::prelude::*;

use std::sync::{Arc, Mutex};

use super::imp::{Settings, Transcriber};
use super::remote_types::TranscriptDef;
use super::CAT;

#[derive(Debug)]
pub struct TranscriberSettings {
    lang_code: types::LanguageCode,
    sample_rate: i32,
    vocabulary: Option<String>,
    vocabulary_filter: Option<String>,
    vocabulary_filter_method: types::VocabularyFilterMethod,
    session_id: Option<String>,
    results_stability: types::PartialResultsStability,
}

impl TranscriberSettings {
    pub(super) fn from(settings: &Settings, sample_rate: i32) -> Self {
        TranscriberSettings {
            lang_code: settings.language_code.as_str().into(),
            sample_rate,
            vocabulary: settings.vocabulary.clone(),
            vocabulary_filter: settings.vocabulary_filter.clone(),
            vocabulary_filter_method: settings.vocabulary_filter_method.into(),
            session_id: settings.session_id.clone(),
            results_stability: settings.results_stability.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TranscriptItem {
    pub pts: gst::ClockTime,
    pub duration: gst::ClockTime,
    pub content: String,
    pub is_punctuation: bool,
}

impl TranscriptItem {
    pub fn from(
        item: types::Item,
        lateness: gst::ClockTime,
        discont_offset: gst::ClockTime,
    ) -> Option<TranscriptItem> {
        let content = item.content?;

        let start_time =
            ((item.start_time * 1_000_000_000.0) as u64).nseconds() + lateness + discont_offset;
        let end_time =
            ((item.end_time * 1_000_000_000.0) as u64).nseconds() + lateness + discont_offset;

        gst::log!(CAT, "Discont offset is {discont_offset}");

        Some(TranscriptItem {
            pts: start_time,
            duration: end_time - start_time,
            content,
            is_punctuation: matches!(item.r#type, Some(types::ItemType::Punctuation)),
        })
    }
}

#[derive(Clone)]
pub enum TranscriptEvent {
    Transcript {
        items: Arc<Vec<TranscriptItem>>,
        serialized: Option<String>,
    },
    Eos,
}

struct DiscontOffsetTracker {
    discont_offset: gst::ClockTime,
    last_chained_buffer_rtime: Option<gst::ClockTime>,
}

pub struct TranscriberStream {
    imp: glib::subclass::ObjectImplRef<Transcriber>,
    output: aws_transcribe::operation::start_stream_transcription::StartStreamTranscriptionOutput,
    lateness: gst::ClockTime,
    partial_index: usize,
    discont_offset_tracker: Arc<Mutex<DiscontOffsetTracker>>,
}

impl TranscriberStream {
    pub async fn try_new(
        imp: &Transcriber,
        settings: TranscriberSettings,
        lateness: gst::ClockTime,
        buffer_rx: mpsc::Receiver<(gst::Buffer, gst::ClockTime)>,
    ) -> Result<Self, gst::ErrorMessage> {
        let client = {
            let aws_config = imp.aws_config.lock().unwrap();
            let aws_config = aws_config
                .as_ref()
                .expect("aws_config must be initialized at this stage");
            aws_transcribe::Client::new(aws_config)
        };

        let discont_offset_tracker = Arc::new(Mutex::new(DiscontOffsetTracker {
            discont_offset: gst::ClockTime::ZERO,
            last_chained_buffer_rtime: gst::ClockTime::NONE,
        }));

        let discont_offset_tracker_clone = discont_offset_tracker.clone();

        // Stream the incoming buffers chunked
        let chunk_stream = buffer_rx.flat_map(move |(buffer, running_time)| {
            let mut discont_offset_tracker = discont_offset_tracker_clone.lock().unwrap();
            if buffer.flags().contains(gst::BufferFlags::DISCONT) {
                if let Some(last_chained_buffer_rtime) = discont_offset_tracker.last_chained_buffer_rtime {
                    discont_offset_tracker.discont_offset += running_time.saturating_sub(last_chained_buffer_rtime);
                }
            }

            discont_offset_tracker.last_chained_buffer_rtime = Some(running_time);
            async_stream::stream! {
                let data = buffer.map_readable().unwrap();
                use aws_transcribe::{types::{AudioEvent, AudioStream}, primitives::Blob};
                for chunk in data.chunks(8192) {
                    yield Ok(AudioStream::AudioEvent(AudioEvent::builder().audio_chunk(Blob::new(chunk)).build()));
                }
            }
        });

        let mut transcribe_builder = client
            .start_stream_transcription()
            .language_code(settings.lang_code)
            .media_sample_rate_hertz(settings.sample_rate)
            .media_encoding(types::MediaEncoding::Pcm)
            .enable_partial_results_stabilization(true)
            .partial_results_stability(settings.results_stability)
            .set_vocabulary_name(settings.vocabulary)
            .set_session_id(settings.session_id);

        if let Some(vocabulary_filter) = settings.vocabulary_filter {
            transcribe_builder = transcribe_builder
                .vocabulary_filter_name(vocabulary_filter)
                .vocabulary_filter_method(settings.vocabulary_filter_method);
        }

        let output = transcribe_builder
            .audio_stream(chunk_stream.into())
            .send()
            .await
            .map_err(|err| {
                let err = format!("Transcribe ws init error: {err}: {} ({err:?})", err.meta());
                gst::error!(CAT, imp = imp, "{err}");
                gst::error_msg!(gst::LibraryError::Init, ["{err}"])
            })?;

        Ok(TranscriberStream {
            imp: imp.ref_counted(),
            output,
            lateness,
            partial_index: 0,
            discont_offset_tracker,
        })
    }

    pub async fn next(&mut self) -> Result<TranscriptEvent, gst::ErrorMessage> {
        loop {
            let event = self
                .output
                .transcript_result_stream
                .recv()
                .await
                .map_err(|err| {
                    let err = format!("Transcribe ws stream error: {err}: {} {err:?}", err.meta());
                    gst::error!(CAT, imp = self.imp, "{err}");
                    gst::error_msg!(gst::LibraryError::Failed, ["{err}"])
                })?;

            let Some(event) = event else {
                gst::debug!(CAT, imp = self.imp, "Transcriber loop sending EOS");
                return Ok(TranscriptEvent::Eos);
            };

            if let types::TranscriptResultStream::TranscriptEvent(mut transcript_evt) = event {
                let Some(ref mut transcript) = transcript_evt.transcript else {
                    continue;
                };

                let t = TranscriptDef {
                    results: transcript.results.clone(),
                };

                let serialized = serde_json::to_string(&t).expect("serializable");

                let Some(result) = transcript
                    .results
                    .as_mut()
                    .and_then(|results| results.drain(..).next())
                else {
                    continue;
                };

                let ready_items = result
                    .alternatives
                    .and_then(|mut alternatives| alternatives.drain(..).next())
                    .and_then(|alternative| alternative.items)
                    .map(|items| self.get_ready_transcript_items(items, result.is_partial))
                    .unwrap_or(vec![]);

                return Ok(TranscriptEvent::Transcript {
                    items: ready_items.into(),
                    serialized: Some(serialized),
                });
            } else {
                gst::warning!(
                    CAT,
                    imp = self.imp,
                    "Transcribe ws returned unknown event: consider upgrading the SDK"
                )
            }
        }
    }

    /// Builds a list from the provided stable items.
    fn get_ready_transcript_items(
        &mut self,
        mut items: Vec<types::Item>,
        partial: bool,
    ) -> Vec<TranscriptItem> {
        let mut output = vec![];

        if items.len() < self.partial_index {
            gst::error!(
                CAT,
                imp = self.imp,
                "sanity check failed, alternative length {} < partial_index {}",
                items.len(),
                self.partial_index
            );

            if !partial {
                self.partial_index = 0;
            }

            return output;
        }

        for item in items.drain(self.partial_index..) {
            if !item.stable().unwrap_or(false) {
                break;
            }

            let discont_offset = self.discont_offset_tracker.lock().unwrap().discont_offset;

            let Some(item) = TranscriptItem::from(item, self.lateness, discont_offset) else {
                continue;
            };
            gst::debug!(
                CAT,
                imp = self.imp,
                "Item is ready for queuing: {}, PTS {}",
                item.content,
                item.pts,
            );

            self.partial_index += 1;
            output.push(item);
        }

        if !partial {
            self.partial_index = 0;
        }

        output
    }
}
