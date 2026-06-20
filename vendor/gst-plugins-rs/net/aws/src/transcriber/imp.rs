// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2023 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! AWS Transcriber element.
//!
//! This element calls AWS Transcribe to extract transcripts from an audio stream.
//! The element can optionally translate the resulting transcripts to one or
//! multiple languages.
//!
//! This module contains the element implementation as well as the `TranslateSrcPad`
//! subclass and its `TranslationPadTask`.
//!
//! Web service specific code can be found in the `transcribe` and `translate` modules.

use gst::subclass::prelude::*;
use gst::{glib, prelude::*};

use aws_sdk_s3::config::StalledStreamProtectionConfig;
use aws_sdk_transcribestreaming as aws_transcribe;

use futures::channel::mpsc;
use futures::future::AbortHandle;
use futures::prelude::*;
use tokio::{sync::broadcast, task};

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use std::sync::LazyLock;

use super::transcribe::{TranscriberSettings, TranscriberStream, TranscriptEvent, TranscriptItem};
use super::translate::{TranslateLoop, TranslatedItem, Translation};
use super::{
    AwsTranscriberResultStability, AwsTranscriberVocabularyFilterMethod,
    TranslationTokenizationMethod, CAT,
};
use crate::s3utils::RUNTIME;

#[allow(deprecated)]
static AWS_BEHAVIOR_VERSION: LazyLock<aws_config::BehaviorVersion> =
    LazyLock::new(aws_config::BehaviorVersion::v2023_11_09);

const DEFAULT_TRANSCRIBER_REGION: &str = "us-east-1";

// Deprecated in 0.11.0: due to evolutions of the transcriber element,
// this property has been replaced by `TRANSCRIBE_LATENCY_PROPERTY`.
const DEPRECATED_LATENCY_PROPERTY: &str = "latency";

const TRANSCRIBE_LATENCY_PROPERTY: &str = "transcribe-latency";
pub const DEFAULT_TRANSCRIBE_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(8);

const TRANSLATE_LATENCY_PROPERTY: &str = "translate-latency";
pub const DEFAULT_TRANSLATE_LATENCY: gst::ClockTime = gst::ClockTime::from_mseconds(500);

const TRANSLATE_LOOKAHEAD_PROPERTY: &str = "translate-lookahead";
pub const DEFAULT_TRANSLATE_LOOKAHEAD: gst::ClockTime = gst::ClockTime::from_seconds(3);

const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::ZERO;
pub const DEFAULT_INPUT_LANG_CODE: &str = "en-US";

const DEFAULT_STABILITY: AwsTranscriberResultStability = AwsTranscriberResultStability::Low;
const DEFAULT_VOCABULARY_FILTER_METHOD: AwsTranscriberVocabularyFilterMethod =
    AwsTranscriberVocabularyFilterMethod::Mask;

// The period at which the event loops will check if they need to push
// anything downstream when no other events show up.
pub const GRANULARITY: gst::ClockTime = gst::ClockTime::from_mseconds(100);

const OUTPUT_LANG_CODE_PROPERTY: &str = "language-code";
const DEFAULT_OUTPUT_LANG_CODE: Option<&str> = None;
const DEFAULT_POST_LATE_WARNINGS: bool = false;

const TRANSLATION_TOKENIZATION_PROPERTY: &str = "tokenization-method";

#[derive(Debug, Clone)]
pub(super) struct Settings {
    transcribe_latency: gst::ClockTime,
    translate_latency: gst::ClockTime,
    translate_lookahead: gst::ClockTime,
    lateness: gst::ClockTime,
    pub language_code: String,
    pub vocabulary: Option<String>,
    pub session_id: Option<String>,
    pub results_stability: AwsTranscriberResultStability,
    access_key: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    pub vocabulary_filter: Option<String>,
    pub vocabulary_filter_method: AwsTranscriberVocabularyFilterMethod,
    post_late_warnings: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            transcribe_latency: DEFAULT_TRANSCRIBE_LATENCY,
            translate_latency: DEFAULT_TRANSLATE_LATENCY,
            translate_lookahead: DEFAULT_TRANSLATE_LOOKAHEAD,
            lateness: DEFAULT_LATENESS,
            language_code: DEFAULT_INPUT_LANG_CODE.to_string(),
            vocabulary: None,
            session_id: None,
            results_stability: DEFAULT_STABILITY,
            access_key: None,
            secret_access_key: None,
            session_token: None,
            vocabulary_filter: None,
            vocabulary_filter_method: DEFAULT_VOCABULARY_FILTER_METHOD,
            post_late_warnings: DEFAULT_POST_LATE_WARNINGS,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct OutputItem {
    pts: gst::ClockTime,
    duration: gst::ClockTime,
    content: String,
}

impl From<&TranscriptItem> for OutputItem {
    fn from(item: &TranscriptItem) -> Self {
        OutputItem {
            pts: item.pts,
            duration: item.duration,
            content: item.content.clone(),
        }
    }
}

impl From<TranslatedItem> for OutputItem {
    fn from(item: TranslatedItem) -> Self {
        OutputItem {
            pts: item.pts,
            duration: item.duration,
            content: item.content,
        }
    }
}

struct State {
    // second tuple member is running time
    buffer_tx: Option<mpsc::Sender<(gst::Buffer, gst::ClockTime)>>,
    transcriber_loop_handle: Option<task::JoinHandle<Result<(), gst::ErrorMessage>>>,
    srcpads: BTreeSet<super::TranslateSrcPad>,
    pad_serial: u32,
    seqnum: gst::Seqnum,
    start_time: Option<gst::ClockTime>,
    in_segment: gst::FormattedSegment<gst::ClockTime>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            buffer_tx: None,
            transcriber_loop_handle: None,
            srcpads: Default::default(),
            pad_serial: 0,
            seqnum: gst::Seqnum::next(),
            start_time: None,
            in_segment: gst::FormattedSegment::new(),
        }
    }
}

pub struct Transcriber {
    static_srcpad: super::TranslateSrcPad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    pub(super) aws_config: Mutex<Option<aws_config::SdkConfig>>,
    // sender to broadcast transcript items to the src pads for translation.
    transcript_event_for_translate_tx: broadcast::Sender<TranscriptEvent>,
    // sender to broadcast transcript items to the src pads, not intended for translation.
    transcript_event_tx: broadcast::Sender<TranscriptEvent>,
}

impl Transcriber {
    fn start_srcpad_tasks(&self, state: &State) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Starting tasks");

        if self.static_srcpad.is_linked() {
            self.static_srcpad.imp().start_task()?;
        }

        for pad in state.srcpads.iter() {
            pad.imp().start_task()?;
        }

        gst::debug!(CAT, imp = self, "Tasks Started");

        Ok(())
    }

    fn stop_tasks(&self, state: &mut State) {
        gst::debug!(CAT, imp = self, "Stopping tasks");

        if self.static_srcpad.is_linked() {
            self.static_srcpad.imp().stop_task();
        }

        for pad in state.srcpads.iter() {
            pad.imp().stop_task();
        }

        // Terminate the audio buffer stream
        state.buffer_tx = None;

        if let Some(transcriber_loop_handle) = state.transcriber_loop_handle.take() {
            transcriber_loop_handle.abort();
        }

        state.start_time = None;

        gst::debug!(CAT, imp = self, "Tasks Stopped");
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        match event.view() {
            Eos(_) => {
                // Terminate the audio buffer stream
                self.state.lock().unwrap().buffer_tx = None;

                true
            }
            FlushStart(_) => {
                gst::info!(CAT, imp = self, "Received flush start, disconnecting");
                let ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                self.stop_tasks(&mut self.state.lock().unwrap());

                ret
            }
            FlushStop(_) => {
                gst::info!(CAT, imp = self, "Received flush stop, restarting task");

                if gst::Pad::event_default(pad, Some(&*self.obj()), event) {
                    let state = self.state.lock().unwrap();
                    match self.start_srcpad_tasks(&state) {
                        Err(err) => {
                            gst::error!(CAT, imp = self, "Failed to start srcpad tasks: {err}");
                            false
                        }
                        Ok(_) => true,
                    }
                } else {
                    false
                }
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

                let mut state = self.state.lock().unwrap();
                state.seqnum = e.seqnum();
                state.in_segment = segment;

                true
            }
            Tag(_) => true,
            Caps(c) => {
                gst::info!(CAT, "Received caps {c:?}");
                true
            }
            StreamStart(_) => {
                let state = self.state.lock().unwrap();
                match self.start_srcpad_tasks(&state) {
                    Err(err) => {
                        gst::error!(CAT, imp = self, "Failed to start srcpad tasks: {err}");
                        false
                    }
                    Ok(_) => true,
                }
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {buffer:?}");

        if buffer.pts().is_none() {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Stream with timestamped buffers required"]
            );

            return Err(gst::FlowError::Error);
        }

        self.ensure_connection().map_err(|err| {
            gst::element_imp_error!(self, gst::StreamError::Failed, ["Streaming failed: {err}"]);
            gst::FlowError::Error
        })?;

        let rtime = match self
            .state
            .lock()
            .unwrap()
            .in_segment
            .to_running_time(buffer.pts())
        {
            Some(rtime) => rtime,
            None => {
                gst::debug!(CAT, "Buffer outside segment, clipping (buffer:?)");
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let Some(mut buffer_tx) = self.state.lock().unwrap().buffer_tx.take() else {
            gst::log!(CAT, obj = pad, "Flushing");
            return Err(gst::FlowError::Flushing);
        };

        futures::executor::block_on(buffer_tx.send((buffer, rtime))).map_err(|err| {
            gst::element_imp_error!(self, gst::StreamError::Failed, ["Streaming failed: {err}"]);
            gst::FlowError::Error
        })?;

        self.state.lock().unwrap().buffer_tx = Some(buffer_tx);

        Ok(gst::FlowSuccess::Ok)
    }
}

#[derive(Default)]
struct TranslateQueue {
    items: VecDeque<TranscriptItem>,
}

impl TranslateQueue {
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Pushes the provided item.
    ///
    /// Returns `Some(..)` if items are ready for translation.
    fn push(&mut self, transcript_item: &TranscriptItem) -> Option<Vec<TranscriptItem>> {
        // Keep track of the item individually so we can schedule translation precisely.
        self.items.push_back(transcript_item.clone());

        if transcript_item.is_punctuation {
            // This makes it a good chunk for translation.
            // Concatenate as a single item for translation

            return Some(self.items.drain(..).collect());
        }

        // Regular case: no separator detected, don't push transcript items
        // to translation now. They will be pushed either if a punctuation
        // is found or of a `dequeue()` is requested.

        None
    }

    /// Dequeues items from the specified `deadline` up to `lookahead`.
    ///
    /// Returns `Some(..)` if some items match the criteria.
    fn dequeue(
        &mut self,
        latency: gst::ClockTime,
        threshold: gst::ClockTime,
        lookahead: gst::ClockTime,
    ) -> Option<Vec<TranscriptItem>> {
        let first_pts = self.items.front()?.pts;
        if first_pts + latency > threshold {
            // First item is too early to be sent to translation now
            // we can wait for more items to accumulate.
            return None;
        }

        // Can't wait any longer to send the first item to translation
        // Try to get up to lookahead worth of items to improve translation accuracy
        let limit = first_pts + lookahead;

        let mut items_acc = vec![self.items.pop_front().unwrap()];
        while let Some(item) = self.items.front() {
            if item.pts > limit {
                break;
            }

            items_acc.push(self.items.pop_front().unwrap());
        }

        Some(items_acc)
    }

    fn drain(&mut self) -> impl Iterator<Item = TranscriptItem> + '_ {
        self.items.drain(..)
    }
}

impl Transcriber {
    fn ensure_connection(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();

        if state.buffer_tx.is_some() {
            return Ok(());
        }

        let settings = self.settings.lock().unwrap();

        let in_caps = self.sinkpad.current_caps().unwrap();
        let s = in_caps.structure(0).unwrap();
        let sample_rate = s.get::<i32>("rate").unwrap();

        let transcription_settings = TranscriberSettings::from(&settings, sample_rate);

        let (buffer_tx, buffer_rx) = mpsc::channel(1);

        let _enter = RUNTIME.enter();
        let mut transcriber_stream = futures::executor::block_on(TranscriberStream::try_new(
            self,
            transcription_settings,
            settings.lateness,
            buffer_rx,
        ))?;

        // Latency budget for an item to be pushed to stream on time
        // Margin:
        // - 2 * GRANULARITY: to make sure we don't push items up to GRANULARITY late.
        // - 1 * GRANULARITY: extra margin to account for additional overheads.
        let latency = settings.transcribe_latency.saturating_sub(3 * GRANULARITY);
        let translate_lookahead = settings.translate_lookahead;
        let mut translate_queue = TranslateQueue::default();
        let imp = self.ref_counted();
        let transcriber_loop_handle = RUNTIME.spawn(async move {
            loop {
                // This is to make sure we send items on a timely basis or at least Gap events.
                let timeout = tokio::time::sleep(GRANULARITY.into()).fuse();
                futures::pin_mut!(timeout);

                let transcriber_next = transcriber_stream.next().fuse();
                futures::pin_mut!(transcriber_next);

                // `transcriber_next` takes precedence over `timeout`
                // because we don't want to loose any incoming items.
                let res = futures::select_biased! {
                    event = transcriber_next => {
                        match event {
                            Ok(event) => Some(event),
                            Err(err) => {
                                gst::element_imp_error!(imp, gst::StreamError::Failed, ["Streaming failed: {err}"]);
                                break;
                            }
                        }
                    }
                    _ = timeout => None,
                };

                use TranscriptEvent::*;
                match res {
                    None => (),
                    Some(Transcript {
                        items,
                        serialized
                    }) => {
                        if imp.transcript_event_tx.receiver_count() > 0 {
                            let _ = imp.transcript_event_tx.send(
                                Transcript {
                                    items: items.clone(),
                                    serialized: serialized.clone()
                                });
                        }

                        if imp.transcript_event_for_translate_tx.receiver_count() > 0 {
                            for item in items.iter() {
                                if let Some(items_to_translate) = translate_queue.push(item) {
                                    let _ = imp
                                        .transcript_event_for_translate_tx
                                        .send(Transcript {
                                            items: items_to_translate.into(),
                                            serialized: None,
                                        });
                                }
                            }
                        }
                    }
                    Some(Eos) => {
                        gst::debug!(CAT, imp = imp, "Transcriber loop sending EOS");

                        if imp.transcript_event_tx.receiver_count() > 0 {
                            let _ = imp.transcript_event_tx.send(Eos);
                        }

                        if imp.transcript_event_for_translate_tx.receiver_count() > 0 {
                            let items_to_translate: Vec<TranscriptItem> =
                                translate_queue.drain().collect();
                            let _ = imp
                                .transcript_event_for_translate_tx
                                .send(Transcript {
                                    items: items_to_translate.into(),
                                    serialized: None,
                                });

                            let _ = imp.transcript_event_for_translate_tx.send(Eos);
                        }

                        break;
                    }
                }

                if imp.transcript_event_for_translate_tx.receiver_count() > 0 {
                    // Check if we need to push items for translation

                    let Some((start_time, now)) = imp.get_start_time_and_now() else {
                        continue;
                    };

                    if !translate_queue.is_empty() {
                        let threshold = now - start_time;

                        if let Some(items_to_translate) =
                            translate_queue.dequeue(latency, threshold, translate_lookahead)
                        {
                            gst::debug!(
                                CAT,
                                imp = imp,
                                "Forcing to translation (threshold {threshold}): {items_to_translate:?}"
                            );
                            let _ = imp
                                .transcript_event_for_translate_tx
                                .send(Transcript {
                                    items: items_to_translate.into(),
                                    serialized: None,
                                });
                        }
                    }
                }
            }

            gst::debug!(CAT, imp = imp, "Exiting transcriber loop");

            Ok(())
        });

        state.transcriber_loop_handle = Some(transcriber_loop_handle);
        state.buffer_tx = Some(buffer_tx);

        Ok(())
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let (access_key, secret_access_key, session_token) = {
            let settings = self.settings.lock().unwrap();
            (
                settings.access_key.clone(),
                settings.secret_access_key.clone(),
                settings.session_token.clone(),
            )
        };

        gst::info!(CAT, imp = self, "Loading aws config...");
        let _enter_guard = RUNTIME.enter();

        let config_loader = match (access_key, secret_access_key) {
            (Some(key), Some(secret_key)) => {
                gst::debug!(CAT, imp = self, "Using settings credentials");
                aws_config::defaults(*AWS_BEHAVIOR_VERSION).credentials_provider(
                    aws_transcribe::config::Credentials::new(
                        key,
                        secret_key,
                        session_token,
                        None,
                        "translate",
                    ),
                )
            }
            _ => {
                gst::debug!(CAT, imp = self, "Attempting to get credentials from env...");
                aws_config::defaults(*AWS_BEHAVIOR_VERSION)
            }
        };

        let config_loader = config_loader.region(
            aws_config::meta::region::RegionProviderChain::default_provider()
                .or_else(DEFAULT_TRANSCRIBER_REGION),
        );

        let config_loader =
            config_loader.stalled_stream_protection(StalledStreamProtectionConfig::disabled());

        let config = futures::executor::block_on(config_loader.load());
        gst::debug!(CAT, imp = self, "Using region {}", config.region().unwrap());

        *self.aws_config.lock().unwrap() = Some(config);

        gst::debug!(CAT, imp = self, "Prepared");

        Ok(())
    }

    fn disconnect(&self) {
        gst::info!(CAT, imp = self, "Unpreparing");
        let mut state = self.state.lock().unwrap();

        self.stop_tasks(&mut state);

        for pad in state.srcpads.iter() {
            pad.imp().set_discont();
        }
        gst::info!(CAT, imp = self, "Unprepared");
    }

    fn get_start_time_and_now(&self) -> Option<(gst::ClockTime, gst::ClockTime)> {
        let now = self.obj().current_running_time()?;

        let mut state = self.state.lock().unwrap();

        if state.start_time.is_none() {
            state.start_time = Some(now);
        }

        Some((state.start_time.unwrap(), now))
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Transcriber {
    const NAME: &'static str = "GstAwsTranscriber";
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

        let templ = klass.pad_template("src").unwrap();
        let static_srcpad = gst::PadBuilder::<super::TranslateSrcPad>::from_template(&templ)
            .activatemode_function(|pad, parent, mode, active| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating TranslateSrcPad"
                        ))
                    },
                    |elem| TranslateSrcPad::activatemode(elem, pad, mode, active),
                )
            })
            .query_function(|pad, parent, query| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |elem| TranslateSrcPad::src_query(elem, pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = klass.pad_template("unsynced_src").unwrap();
        let static_unsynced_srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();
        // Setting the channel capacity so that a TranslateSrcPad that would lag
        // behind for some reasons get a chance to catch-up without loosing items.
        // Receiver will be created by subscribing to sender later.
        let (transcript_event_for_translate_tx, _) = broadcast::channel(128);
        let (transcript_event_tx, _) = broadcast::channel(128);

        static_srcpad
            .imp()
            .set_unsynced_pad(&static_unsynced_srcpad);

        Self {
            static_srcpad,
            sinkpad,
            settings: Default::default(),
            state: Default::default(),
            aws_config: Default::default(),
            transcript_event_for_translate_tx,
            transcript_event_tx,
        }
    }
}

fn store_language_tag(pad: &gst::Pad, stream_id: &str, language_code: &str) {
    // Make sure our tags do not get overwritten
    let sev = gst::event::StreamStart::builder(stream_id).build();
    let _ = pad.store_sticky_event(&sev);

    let mut tl = gst::TagList::new();
    tl.make_mut()
        .add::<gst::tags::LanguageCode>(&language_code, gst::TagMergeMode::Append);
    let ev = gst::event::Tag::builder(tl).build();
    let _ = pad.store_sticky_event(&ev);
}

impl ObjectImpl for Transcriber {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("language-code")
                    .nick("Language Code")
                    .blurb("The Language of the Stream, see \
                        <https://docs.aws.amazon.com/transcribe/latest/dg/how-streaming-transcription.html> \
                        for an up to date list of allowed languages")
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder(DEPRECATED_LATENCY_PROPERTY)
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow AWS transcribe (Deprecated. Use transcribe-latency)")
                    .default_value(DEFAULT_TRANSCRIBE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                glib::ParamSpecUInt::builder(TRANSCRIBE_LATENCY_PROPERTY)
                    .nick("AWS Transcribe Latency")
                    .blurb("Amount of milliseconds to allow AWS transcribe")
                    .default_value(DEFAULT_TRANSCRIBE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder(TRANSLATE_LATENCY_PROPERTY)
                    .nick("AWS Translate Latency")
                    .blurb(concat!(
                        "Amount of milliseconds to allow AWS translate ",
                        "(ignored if the input and output languages are the same)",
                    ))
                    .default_value(DEFAULT_TRANSLATE_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder(TRANSLATE_LOOKAHEAD_PROPERTY)
                    .nick("Translate lookahead")
                    .blurb(concat!(
                        "Maximum duration in milliseconds of transcript to lookahead ",
                        "before sending to translation when no separator was encountered",
                    ))
                    .default_value(DEFAULT_TRANSLATE_LOOKAHEAD.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Lateness")
                    .blurb("Amount of milliseconds to introduce as lateness")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("vocabulary-name")
                    .nick("Vocabulary Name")
                    .blurb("The name of a custom vocabulary, see \
                        <https://docs.aws.amazon.com/transcribe/latest/dg/how-vocabulary.html> \
                        for more information")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("session-id")
                    .nick("Session ID")
                    .blurb("The ID of the transcription session, must be length 36")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("results-stability", DEFAULT_STABILITY)
                    .nick("Results stability")
                    .blurb("Defines how fast results should stabilize")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("access-key")
                    .nick("Access Key")
                    .blurb("AWS Access Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("secret-access-key")
                    .nick("Secret Access Key")
                    .blurb("AWS Secret Access Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("session-token")
                    .nick("Session Token")
                    .blurb("AWS temporary Session Token from STS")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("vocabulary-filter-name")
                    .nick("Vocabulary Filter Name")
                    .blurb("The name of a custom filter vocabulary, see \
                        <https://docs.aws.amazon.com/transcribe/latest/help-panel/vocab-filter.html> \
                        for more information")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("vocabulary-filter-method", DEFAULT_VOCABULARY_FILTER_METHOD)
                    .nick("Vocabulary Filter Method")
                    .blurb("Defines how filtered words will be edited, has no effect when vocabulary-filter-name isn't set")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let language_code = self.settings.lock().unwrap().language_code.clone();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.static_srcpad).unwrap();
        self.static_srcpad.set_active(true).unwrap();

        store_language_tag(
            self.static_srcpad.upcast_ref(),
            "transcription",
            &language_code,
        );

        let pad_state = self.static_srcpad.imp().state.lock().unwrap();

        let unsynced_pad = pad_state.unsynced_pad.as_ref().unwrap();

        obj.add_pad(unsynced_pad).unwrap();
        unsynced_pad.set_active(true).unwrap();

        store_language_tag(
            unsynced_pad.upcast_ref(),
            "unsynced-transcription",
            &language_code,
        );

        obj.set_element_flags(gst::ElementFlags::PROVIDE_CLOCK | gst::ElementFlags::REQUIRE_CLOCK);
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "language-code" => {
                let language_code: String = value.get().expect("type checked upstream");

                store_language_tag(
                    self.static_srcpad.upcast_ref(),
                    "transcription",
                    &language_code,
                );

                if let Some(unsynced_pad) = self
                    .static_srcpad
                    .imp()
                    .state
                    .lock()
                    .unwrap()
                    .unsynced_pad
                    .as_ref()
                {
                    store_language_tag(
                        unsynced_pad.upcast_ref(),
                        "unsynced-transcription",
                        &language_code,
                    );
                }

                let mut settings = self.settings.lock().unwrap();
                settings.language_code = language_code;
            }
            DEPRECATED_LATENCY_PROPERTY => {
                let mut settings = self.settings.lock().unwrap();
                settings.transcribe_latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            TRANSCRIBE_LATENCY_PROPERTY => {
                let mut settings = self.settings.lock().unwrap();
                settings.transcribe_latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            TRANSLATE_LATENCY_PROPERTY => {
                self.settings.lock().unwrap().translate_latency =
                    gst::ClockTime::from_mseconds(value.get::<u32>().unwrap().into());
            }
            TRANSLATE_LOOKAHEAD_PROPERTY => {
                self.settings.lock().unwrap().translate_lookahead =
                    gst::ClockTime::from_mseconds(value.get::<u32>().unwrap().into());
            }
            "lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "vocabulary-name" => {
                let mut settings = self.settings.lock().unwrap();
                settings.vocabulary = value.get().expect("type checked upstream");
            }
            "session-id" => {
                let mut settings = self.settings.lock().unwrap();
                settings.session_id = value.get().expect("type checked upstream");
            }
            "results-stability" => {
                let mut settings = self.settings.lock().unwrap();
                settings.results_stability = value
                    .get::<AwsTranscriberResultStability>()
                    .expect("type checked upstream");
            }
            "access-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.access_key = value.get().expect("type checked upstream");
            }
            "secret-access-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.secret_access_key = value.get().expect("type checked upstream");
            }
            "session-token" => {
                let mut settings = self.settings.lock().unwrap();
                settings.session_token = value.get().expect("type checked upstream");
            }
            "vocabulary-filter-name" => {
                let mut settings = self.settings.lock().unwrap();
                settings.vocabulary_filter = value.get().expect("type checked upstream");
            }
            "vocabulary-filter-method" => {
                let mut settings = self.settings.lock().unwrap();
                settings.vocabulary_filter_method = value
                    .get::<AwsTranscriberVocabularyFilterMethod>()
                    .expect("type checked upstream");
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
            DEPRECATED_LATENCY_PROPERTY => {
                let settings = self.settings.lock().unwrap();
                (settings.transcribe_latency.mseconds() as u32).to_value()
            }
            TRANSCRIBE_LATENCY_PROPERTY => {
                let settings = self.settings.lock().unwrap();
                (settings.transcribe_latency.mseconds() as u32).to_value()
            }
            TRANSLATE_LATENCY_PROPERTY => {
                (self.settings.lock().unwrap().translate_latency.mseconds() as u32).to_value()
            }
            TRANSLATE_LOOKAHEAD_PROPERTY => {
                (self.settings.lock().unwrap().translate_lookahead.mseconds() as u32).to_value()
            }
            "lateness" => {
                let settings = self.settings.lock().unwrap();
                (settings.lateness.mseconds() as u32).to_value()
            }
            "vocabulary-name" => {
                let settings = self.settings.lock().unwrap();
                settings.vocabulary.to_value()
            }
            "session-id" => {
                let settings = self.settings.lock().unwrap();
                settings.session_id.to_value()
            }
            "results-stability" => {
                let settings = self.settings.lock().unwrap();
                settings.results_stability.to_value()
            }
            "access-key" => {
                let settings = self.settings.lock().unwrap();
                settings.access_key.to_value()
            }
            "secret-access-key" => {
                let settings = self.settings.lock().unwrap();
                settings.secret_access_key.to_value()
            }
            "session-token" => {
                let settings = self.settings.lock().unwrap();
                settings.session_token.to_value()
            }
            "vocabulary-filter-name" => {
                let settings = self.settings.lock().unwrap();
                settings.vocabulary_filter.to_value()
            }
            "vocabulary-filter-method" => {
                let settings = self.settings.lock().unwrap();
                settings.vocabulary_filter_method.to_value()
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
            "Speech to Text filter, using AWS transcribe",
            "Jordan Petridis <jordan@centricular.com>, Mathieu Duponchelle <mathieu@centricular.com>, François Laignel <francois@centricular.com>",
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
                super::TranslateSrcPad::static_type(),
            )
            .unwrap();
            let req_src_pad_template = gst::PadTemplate::with_gtype(
                "translate_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Request,
                &src_caps,
                super::TranslateSrcPad::static_type(),
            )
            .unwrap();
            let src_caps = gst::Caps::builder("application/x-json").build();
            let unsynced_src_pad_template = gst::PadTemplate::new(
                "unsynced_src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();
            let unsynced_sometimes_src_pad_template = gst::PadTemplate::new(
                "unsynced_translate_src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &src_caps,
            )
            .unwrap();

            let sink_caps = gst_audio::AudioCapsBuilder::new()
                .format(gst_audio::AudioFormat::S16le)
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

            vec![
                src_pad_template,
                req_src_pad_template,
                unsynced_src_pad_template,
                unsynced_sometimes_src_pad_template,
                sink_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::info!(CAT, imp = self, "Changing state {transition:?}");

        if let gst::StateChange::NullToReady = transition {
            self.prepare().map_err(|err| {
                self.post_error_message(err);
                gst::StateChangeError
            })?;
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                self.disconnect();
            }
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut state = self.state.lock().unwrap();

        let pad = gst::PadBuilder::<super::TranslateSrcPad>::from_template(templ)
            .name(format!("translate_src_{}", state.pad_serial).as_str())
            .activatemode_function(|pad, parent, mode, active| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating TranslateSrcPad"
                        ))
                    },
                    |elem| TranslateSrcPad::activatemode(elem, pad, mode, active),
                )
            })
            .query_function(|pad, parent, query| {
                Transcriber::catch_panic_pad_function(
                    parent,
                    || false,
                    |elem| TranslateSrcPad::src_query(elem, pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = self
            .obj()
            .class()
            .pad_template("unsynced_translate_src_%u")
            .unwrap();
        let static_unsynced_srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .name(format!("unsynced_translate_src_{}", state.pad_serial).as_str())
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        pad.imp().set_unsynced_pad(&static_unsynced_srcpad);

        state.srcpads.insert(pad.clone());

        state.pad_serial += 1;
        drop(state);

        self.obj().add_pad(&pad).unwrap();
        self.obj().add_pad(&static_unsynced_srcpad).unwrap();

        pad.set_active(true).unwrap();
        static_unsynced_srcpad.set_active(true).unwrap();

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        self.obj().child_added(&pad, &pad.name());
        Some(pad.upcast())
    }

    fn release_pad(&self, pad: &gst::Pad) {
        pad.set_active(false).unwrap();
        self.obj().remove_pad(pad).unwrap();
        self.state.lock().unwrap().srcpads.remove(pad);

        let translate_srcpad = pad.downcast_ref::<super::TranslateSrcPad>().unwrap();

        if let Some(unsynced_pad) = translate_srcpad.imp().take_unsynced_pad() {
            unsynced_pad.set_active(false).unwrap();
            self.obj().remove_pad(&unsynced_pad).unwrap();
        }

        self.obj().child_removed(pad, &pad.name());
        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
    }

    fn provide_clock(&self) -> Option<gst::Clock> {
        Some(gst::SystemClock::obtain())
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

struct TranslationPadTask {
    pad: glib::subclass::ObjectImplRef<TranslateSrcPad>,
    elem: super::Transcriber,
    transcript_event_rx: broadcast::Receiver<TranscriptEvent>,
    needs_translate: bool,
    translate_loop_handle: Option<task::JoinHandle<Result<(), gst::ErrorMessage>>>,
    to_translate_tx: Option<mpsc::Sender<Arc<Vec<TranscriptItem>>>>,
    from_translate_rx: Option<mpsc::Receiver<Translation>>,
    send_events: bool,
    output_items: VecDeque<OutputItem>,
    our_latency: gst::ClockTime,
    seqnum: gst::Seqnum,
    send_eos: bool,
    pending_translations: usize,
    unsynced_pad: Option<gst::Pad>,
    unsynced: Option<String>,
}

impl TranslationPadTask {
    async fn try_new(
        pad: &TranslateSrcPad,
        elem: super::Transcriber,
    ) -> Result<TranslationPadTask, gst::ErrorMessage> {
        let mut translation_loop = None;
        let mut translate_loop_handle = None;
        let mut to_translate_tx = None;
        let mut from_translate_rx = None;

        let (our_latency, transcript_event_rx, needs_translate);

        {
            let elem_imp = elem.imp();
            let elem_settings = elem_imp.settings.lock().unwrap();

            let pad_settings = pad.settings.lock().unwrap();

            our_latency = TranslateSrcPad::our_latency(&elem_settings, &pad_settings);
            if our_latency + elem_settings.lateness <= 2 * GRANULARITY {
                let err = format!(
                    "total latency + lateness must be greater than {}",
                    2 * GRANULARITY
                );
                gst::error!(CAT, imp = pad, "{err}");
                return Err(gst::error_msg!(gst::LibraryError::Settings, ["{err}"]));
            }

            needs_translate = TranslateSrcPad::needs_translation(
                &elem_settings.language_code,
                pad_settings.language_code.as_deref(),
            );

            if needs_translate {
                let (to_loop_tx, to_loop_rx) = mpsc::channel(64);
                let (from_loop_tx, from_loop_rx) = mpsc::channel(64);

                translation_loop = Some(TranslateLoop::new(
                    elem_imp,
                    pad,
                    &elem_settings.language_code,
                    pad_settings.language_code.as_deref().unwrap(),
                    pad_settings.tokenization_method,
                    to_loop_rx,
                    from_loop_tx,
                ));

                to_translate_tx = Some(to_loop_tx);
                from_translate_rx = Some(from_loop_rx);

                transcript_event_rx = elem_imp.transcript_event_for_translate_tx.subscribe();
            } else {
                transcript_event_rx = elem_imp.transcript_event_tx.subscribe();
            }
        }

        if let Some(translation_loop) = translation_loop {
            translation_loop.check_language().await?;
            translate_loop_handle = Some(RUNTIME.spawn(translation_loop.run()));
        }

        Ok(TranslationPadTask {
            pad: pad.ref_counted(),
            elem,
            transcript_event_rx,
            needs_translate,
            translate_loop_handle,
            to_translate_tx,
            from_translate_rx,
            send_events: true,
            output_items: VecDeque::new(),
            our_latency,
            seqnum: gst::Seqnum::next(),
            send_eos: false,
            pending_translations: 0,
            unsynced_pad: pad.state.lock().unwrap().unsynced_pad.clone(),
            unsynced: None,
        })
    }
}

impl Drop for TranslationPadTask {
    fn drop(&mut self) {
        if let Some(translate_loop_handle) = self.translate_loop_handle.take() {
            translate_loop_handle.abort();
        }
    }
}

impl TranslationPadTask {
    async fn run_iter(&mut self) -> Result<(), gst::ErrorMessage> {
        self.ensure_init_events()?;

        if self.needs_translate {
            self.translate_iter().await?;
        } else {
            self.passthrough_iter().await?;
        }

        if !self.dequeue().await {
            gst::info!(CAT, imp = self.pad, "Failed to dequeue buffer, pausing");
            let _ = self.pad.obj().pause_task();
        }

        Ok(())
    }

    async fn passthrough_iter(&mut self) -> Result<(), gst::ErrorMessage> {
        // This is to make sure we send items on a timely basis or at least Gap events.
        let timeout = tokio::time::sleep(GRANULARITY.into()).fuse();
        futures::pin_mut!(timeout);

        let transcript_event_rx = self.transcript_event_rx.recv().fuse();
        futures::pin_mut!(transcript_event_rx);

        // `transcript_event_rx` takes precedence over `timeout`
        // because we don't want to loose any incoming items.
        futures::select_biased! {
            items_res = transcript_event_rx => {
                use TranscriptEvent::*;
                use broadcast::error::RecvError;
                match items_res {
                    Ok(Transcript {
                        items,
                        serialized,
                    }) => {
                        self.unsynced = serialized;
                        self.output_items.extend(items.iter().map(Into::into));
                    }
                    Ok(Eos) => {
                        gst::debug!(CAT, imp = self.pad, "Got eos");
                        self.send_eos = true;
                    }
                    Err(RecvError::Lagged(nb_msg)) => {
                        gst::warning!(CAT, imp = self.pad, "Missed {nb_msg} transcript sets");
                    }
                    Err(RecvError::Closed) => {
                        gst::debug!(CAT, imp = self.pad, "Transcript chan terminated: setting eos");
                        self.send_eos = true;
                    }
                }
            }
            _ = timeout => (),
        }

        Ok(())
    }

    async fn translate_iter(&mut self) -> Result<(), gst::ErrorMessage> {
        if self
            .translate_loop_handle
            .as_ref()
            .is_none_or(task::JoinHandle::is_finished)
        {
            const ERR: &str = "Translate loop is not running";
            gst::error!(CAT, imp = self.pad, "{ERR}");
            return Err(gst::error_msg!(gst::StreamError::Failed, ["{ERR}"]));
        }

        let items_to_translate = {
            // This is to make sure we send items on a timely basis or at least Gap events.
            let timeout = tokio::time::sleep(GRANULARITY.into()).fuse();
            futures::pin_mut!(timeout);

            let transcript_event_rx = self.transcript_event_rx.recv().fuse();
            futures::pin_mut!(transcript_event_rx);

            // `transcript_event_rx` takes precedence over `timeout`
            // because we don't want to loose any incoming items.
            futures::select_biased! {
                items_res = transcript_event_rx => {
                    use TranscriptEvent::*;
                    use broadcast::error::RecvError;
                    match items_res {
                        Ok(Transcript {
                            items,
                            ..
                        }) => Some(items),
                        Ok(Eos) => {
                            gst::debug!(CAT, imp = self.pad, "Got eos");
                            self.send_eos = true;
                            None
                        }
                        Err(RecvError::Lagged(nb_msg)) => {
                            gst::warning!(CAT, imp = self.pad, "Missed {nb_msg} transcript sets");
                            None
                        }
                        Err(RecvError::Closed) => {
                            gst::debug!(CAT, imp = self.pad, "Transcript chan terminated: setting eos");
                            self.send_eos = true;
                            None
                        }
                    }
                }
                _ = timeout => None,
            }
        };

        if let Some(items_to_translate) = items_to_translate {
            if !items_to_translate.is_empty() {
                let res = self
                    .to_translate_tx
                    .as_mut()
                    .expect("to_translation chan must be available in translation mode")
                    .send(items_to_translate)
                    .await;

                if res.is_err() {
                    const ERR: &str = "to_translation chan terminated";
                    gst::debug!(CAT, imp = self.pad, "{ERR}");
                    return Err(gst::error_msg!(gst::StreamError::Failed, ["{ERR}"]));
                }

                self.pending_translations += 1;
            }
        }

        // Check pending translated items
        let from_translate_rx = self
            .from_translate_rx
            .as_mut()
            .expect("from_translation chan must be available in translation mode");

        while let Ok(translation) = from_translate_rx.try_next() {
            let Some(translation) = translation else {
                const ERR: &str = "translation chan terminated";
                gst::debug!(CAT, imp = self.pad, "{ERR}");
                return Err(gst::error_msg!(gst::StreamError::Failed, ["{ERR}"]));
            };

            if let Some(pts) = translation.items.first().map(|i| i.pts) {
                self.unsynced = Some(
                    serde_json::json!({
                        "translation": translation.translation,
                        "start_time": *pts,
                    })
                    .to_string(),
                );
            }
            self.output_items
                .extend(translation.items.into_iter().map(Into::into));
            self.pending_translations = self.pending_translations.saturating_sub(1);
        }

        Ok(())
    }

    async fn dequeue(&mut self) -> bool {
        let Some((start_time, now)) = self.elem.imp().get_start_time_and_now() else {
            // Wait for the clock to be available
            return true;
        };

        let (mut last_position, mut discont_pending) = {
            let mut state = self.pad.state.lock().unwrap();

            if state.start_time.is_none() {
                state.start_time = Some(start_time);
                state.out_segment.set_position(start_time);
            }

            let last_position = state.out_segment.position().unwrap();

            (last_position, state.discont_pending)
        };

        if let Some(unsynced) = self.unsynced.take() {
            if let Some(ref unsynced_pad) = self.unsynced_pad {
                if unsynced_pad.last_flow_result().is_ok() {
                    gst::log!(
                        CAT,
                        obj = unsynced_pad,
                        "pushing serialized transcript with timestamp {now}"
                    );
                    gst::trace!(CAT, obj = unsynced_pad, "serialized transcript: {unsynced}");

                    let mut buf = gst::Buffer::from_mut_slice(unsynced.into_bytes());
                    {
                        let buf_mut = buf.get_mut().unwrap();

                        buf_mut.set_pts(now);
                    }

                    let _ = unsynced_pad.push(buf);
                } else {
                    gst::log!(
                        CAT,
                        obj = unsynced_pad,
                        "not pushing serialized transcript, last flow result: {:?}",
                        unsynced_pad.last_flow_result()
                    );
                }
            }
        }

        /* First, check our pending buffers */
        while let Some(item) = self.output_items.front() {
            // Note: items pts start from 0 + lateness
            gst::trace!(
                CAT,
                imp = self.pad,
                "Checking now {now} if item is ready for dequeuing, PTS {}, threshold {} vs {}",
                item.pts,
                item.pts + self.our_latency.saturating_sub(3 * GRANULARITY),
                now - start_time
            );

            // Margin:
            // - 2 * GRANULARITY: to make sure we don't push items up to GRANULARITY late.
            // - 1 * GRANULARITY: extra margin to account for additional overheads.
            if item.pts + self.our_latency.saturating_sub(3 * GRANULARITY) < now - start_time {
                /* Safe unwrap, we know we have an item */
                let OutputItem {
                    pts: item_pts,
                    duration,
                    content,
                } = self.output_items.pop_front().unwrap();

                let mut pts = start_time + item_pts;

                let mut buf = gst::Buffer::from_mut_slice(content.clone().into_bytes());
                {
                    let buf = buf.get_mut().unwrap();

                    if discont_pending {
                        buf.set_flags(gst::BufferFlags::DISCONT);
                        discont_pending = false;
                    }

                    buf.set_pts(pts);
                    buf.set_duration(duration);
                }

                use std::cmp::Ordering::*;
                match pts.cmp(&last_position) {
                    Greater => {
                        // The buffer we are about to push starts after the end of
                        // last item previously pushed to the stream.
                        let gap_event = gst::event::Gap::builder(last_position)
                            .duration(pts - last_position)
                            .seqnum(self.seqnum)
                            .build();
                        gst::log!(
                            CAT,
                            imp = self.pad,
                            "Pushing gap:    {last_position} -> {pts}"
                        );
                        if !self.pad.obj().push_event(gap_event) {
                            return false;
                        }
                    }
                    Less => {
                        // The buffer we are about to push was expected to start
                        // before the end of last item previously pushed to the stream.
                        // => update it to fit in stream.

                        gst::warning!(
                            CAT,
                            imp = self.pad,
                            "Updating item PTS ({pts} < {last_position}), consider increasing latency",
                        );

                        let post_late_warnings = self
                            .pad
                            .parent()
                            .imp()
                            .settings
                            .lock()
                            .unwrap()
                            .post_late_warnings;

                        if post_late_warnings {
                            let details = gst::Structure::builder("awstranscriber/late-item")
                                .field("original-pts", pts)
                                .field("last-position", last_position)
                                .build();

                            gst::element_warning!(
                                self.pad.parent(),
                                gst::LibraryError::Settings,
                                ["Late transcription item, updating PTS"],
                                details: details
                            );
                        }

                        pts = last_position;

                        {
                            let buf_mut = buf.get_mut().unwrap();

                            buf_mut.set_pts(pts);
                            buf_mut.set_duration(duration);
                        }
                    }
                    _ => (),
                }

                last_position = pts + duration;

                gst::debug!(
                    CAT,
                    imp = self.pad,
                    "Pushing buffer with content {content}: {pts} -> {}",
                    pts + duration
                );

                if self.pad.obj().push(buf).is_err() {
                    return false;
                }
            } else {
                // Current and subsequent items are not ready to be pushed
                break;
            }
        }

        if self.send_eos && self.pending_translations == 0 && self.output_items.is_empty() {
            /* We're EOS, we can pause and exit early */
            let _ = self.pad.obj().pause_task();

            gst::info!(CAT, imp = self.pad, "Sending eos");
            return self
                .pad
                .obj()
                .push_event(gst::event::Eos::builder().seqnum(self.seqnum).build());
        }

        /* next, push a gap if we're lagging behind the target position */
        gst::trace!(
            CAT,
            imp = self.pad,
            "Checking now: {now} if we need to push a gap, last_position: {last_position}, threshold: {}",
            last_position + self.our_latency.saturating_sub(GRANULARITY)
        );

        if now > last_position + self.our_latency.saturating_sub(GRANULARITY) {
            // We are running out of latency budget since last time we pushed downstream,
            // so push a Gap long enough to keep continuity before we dequeue again:
            // worse case scenario, this is GRANULARITY ms from now.
            let duration = now - last_position - self.our_latency.saturating_sub(GRANULARITY);

            let gap_event = gst::event::Gap::builder(last_position)
                .duration(duration)
                .seqnum(self.seqnum)
                .build();

            gst::log!(
                CAT,
                imp = self.pad,
                "Pushing gap:    {last_position} -> {}",
                last_position + duration
            );

            last_position += duration;

            if !self.pad.obj().push_event(gap_event) {
                return false;
            }
        }

        let mut pad_state = self.pad.state.lock().unwrap();
        pad_state.out_segment.set_position(last_position);
        pad_state.discont_pending = discont_pending;

        true
    }

    fn ensure_init_events(&mut self) -> Result<(), gst::ErrorMessage> {
        if !self.send_events {
            return Ok(());
        }

        let mut events = vec![];
        let mut unsynced_events = vec![];

        {
            let elem_imp = self.elem.imp();
            let elem_state = elem_imp.state.lock().unwrap();

            let mut pad_state = self.pad.state.lock().unwrap();

            self.seqnum = elem_state.seqnum;
            pad_state.out_segment = Default::default();
            pad_state.start_time = None;

            events.push(
                gst::event::StreamStart::builder("transcription")
                    .seqnum(self.seqnum)
                    .build(),
            );

            unsynced_events.push(
                gst::event::StreamStart::builder("unsynced-transcription")
                    .seqnum(self.seqnum)
                    .build(),
            );

            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
            events.push(gst::event::Caps::builder(&caps).seqnum(self.seqnum).build());

            let caps = gst::Caps::builder("application/x-json").build();
            unsynced_events.push(gst::event::Caps::builder(&caps).seqnum(self.seqnum).build());

            events.push(
                gst::event::Segment::builder(&pad_state.out_segment)
                    .seqnum(self.seqnum)
                    .build(),
            );

            unsynced_events.push(
                gst::event::Segment::builder(&pad_state.out_segment)
                    .seqnum(self.seqnum)
                    .build(),
            );
        }

        for event in events.drain(..) {
            gst::info!(CAT, imp = self.pad, "Sending {event:?}");
            if !self.pad.obj().push_event(event) {
                const ERR: &str = "Failed to send initial";
                gst::error!(CAT, imp = self.pad, "{ERR}");
                return Err(gst::error_msg!(gst::StreamError::Failed, ["{ERR}"]));
            }
        }

        if let Some(ref unsynced_pad) = self.unsynced_pad {
            for event in unsynced_events.drain(..) {
                gst::info!(CAT, obj = unsynced_pad, "Sending {event:?}");
                let _ = unsynced_pad.push_event(event);
            }
        }

        self.send_events = false;

        Ok(())
    }
}

#[derive(Debug)]
struct TranslationPadState {
    discont_pending: bool,
    out_segment: gst::FormattedSegment<gst::ClockTime>,
    task_abort_handle: Option<AbortHandle>,
    start_time: Option<gst::ClockTime>,
    unsynced_pad: Option<gst::Pad>,
}

impl Default for TranslationPadState {
    fn default() -> TranslationPadState {
        TranslationPadState {
            discont_pending: true,
            out_segment: Default::default(),
            task_abort_handle: None,
            start_time: None,
            unsynced_pad: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct TranslatePadSettings {
    language_code: Option<String>,
    tokenization_method: TranslationTokenizationMethod,
}

#[derive(Debug, Default)]
pub struct TranslateSrcPad {
    state: Mutex<TranslationPadState>,
    settings: Mutex<TranslatePadSettings>,
}

impl TranslateSrcPad {
    fn set_unsynced_pad(&self, pad: &gst::Pad) {
        self.state.lock().unwrap().unsynced_pad = Some(pad.clone());
    }

    fn take_unsynced_pad(&self) -> Option<gst::Pad> {
        self.state.lock().unwrap().unsynced_pad.take()
    }

    fn start_task(&self) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Starting task");

        let elem = self.parent();
        let _enter = RUNTIME.enter();
        let mut pad_task = futures::executor::block_on(TranslationPadTask::try_new(self, elem))
            .map_err(|err| gst::loggable_error!(CAT, "Failed to start pad task {err}"))?;

        let imp = self.ref_counted();
        let res = self.obj().start_task(move || {
            let (abortable_task_iter, abort_handle) = future::abortable(pad_task.run_iter());
            imp.state.lock().unwrap().task_abort_handle = Some(abort_handle);

            let _enter = RUNTIME.enter();
            match futures::executor::block_on(abortable_task_iter) {
                Ok(Ok(())) => (),
                Ok(Err(err)) => {
                    // Don't bring down the whole element if this Pad fails
                    // FIXME is there a way to mark the Pad in error though?
                    gst::info!(CAT, imp = imp, "Pausing task due to: {err}");
                    let _ = imp.obj().pause_task();
                }
                Err(_) => gst::debug!(CAT, imp = imp, "task iter aborted"),
            }
        });

        if res.is_err() {
            return Err(gst::loggable_error!(CAT, "Failed to start pad task"));
        }

        gst::debug!(CAT, imp = self, "Task started");

        Ok(())
    }

    fn stop_task(&self) {
        gst::debug!(CAT, imp = self, "Stopping task");

        // See also the note in `start_task()`:
        // 1. Mark the task as stopped so no further iteration is executed.
        let _ = self.obj().stop_task();

        // 2. Abort the task iteration if the Future is pending.
        if let Some(task_abort_handle) = self.state.lock().unwrap().task_abort_handle.take() {
            task_abort_handle.abort();
        }

        gst::debug!(CAT, imp = self, "Task stopped");
    }

    fn set_discont(&self) {
        self.state.lock().unwrap().discont_pending = true;
    }

    #[inline]
    fn needs_translation(input_lang: &str, output_lang: Option<&str>) -> bool {
        output_lang.is_some_and(|other| !input_lang.eq_ignore_ascii_case(other.as_ref()))
    }

    #[inline]
    fn our_latency(
        elem_settings: &Settings,
        pad_settings: &TranslatePadSettings,
    ) -> gst::ClockTime {
        if Self::needs_translation(
            &elem_settings.language_code,
            pad_settings.language_code.as_deref(),
        ) {
            elem_settings.transcribe_latency + elem_settings.translate_latency
        } else {
            elem_settings.transcribe_latency
        }
    }

    #[track_caller]
    fn parent(&self) -> super::Transcriber {
        self.obj()
            .parent()
            .map(|elem_obj| {
                elem_obj
                    .downcast::<super::Transcriber>()
                    .expect("Wrong Element type")
            })
            .expect("Pad should have a parent at this stage")
    }
}

impl TranslateSrcPad {
    #[track_caller]
    pub fn activatemode(
        _elem: &Transcriber,
        pad: &super::TranslateSrcPad,
        _mode: gst::PadMode,
        active: bool,
    ) -> Result<(), gst::LoggableError> {
        if !active {
            pad.imp().stop_task();
        }

        Ok(())
    }

    pub fn src_query(
        elem: &Transcriber,
        pad: &super::TranslateSrcPad,
        query: &mut gst::QueryRef,
    ) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {query:?}");

        use gst::QueryViewMut::*;
        match query.view_mut() {
            Latency(q) => {
                let mut peer_query = gst::query::Latency::new();

                let ret = elem.sinkpad.peer_query(&mut peer_query);

                if ret {
                    let (_, min, _) = peer_query.result();

                    let our_latency = {
                        let elem_settings = elem.settings.lock().unwrap();
                        let pad_settings = pad.imp().settings.lock().unwrap();

                        Self::our_latency(&elem_settings, &pad_settings)
                    };

                    gst::info!(CAT, obj = pad, "Our latency {our_latency}");
                    q.set(true, our_latency + min, gst::ClockTime::NONE);
                }
                ret
            }
            Position(q) => {
                if q.format() == gst::Format::Time {
                    let stream_time = {
                        let state = pad.imp().state.lock().unwrap();
                        state
                            .out_segment
                            .to_stream_time(state.out_segment.position())
                    };

                    let Some(stream_time) = stream_time else {
                        return false;
                    };
                    q.set(stream_time);

                    true
                } else {
                    false
                }
            }
            _ => gst::Pad::query_default(pad, Some(pad), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TranslateSrcPad {
    const NAME: &'static str = "GstTranslateSrcPad";
    type Type = super::TranslateSrcPad;
    type ParentType = gst::Pad;

    fn new() -> Self {
        Default::default()
    }
}

impl ObjectImpl for TranslateSrcPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder(OUTPUT_LANG_CODE_PROPERTY)
                    .nick("Language Code")
                    .blurb("The Language the Stream must be translated to")
                    .default_value(DEFAULT_OUTPUT_LANG_CODE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder(TRANSLATION_TOKENIZATION_PROPERTY)
                    .nick("Translations tokenization method")
                    .blurb("The tokenization method to apply to translations")
                    .default_value(TranslationTokenizationMethod::default())
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            OUTPUT_LANG_CODE_PROPERTY => {
                let language_code: Option<String> = value.get().unwrap();

                self.settings.lock().unwrap().language_code = language_code.clone();

                if let Some(language_code) = language_code {
                    store_language_tag(self.obj().upcast_ref(), "transcription", &language_code);

                    if let Some(pad) = self.state.lock().unwrap().unsynced_pad.as_ref() {
                        store_language_tag(pad, "unsynced-transcription", &language_code);
                    }
                }
            }
            TRANSLATION_TOKENIZATION_PROPERTY => {
                self.settings.lock().unwrap().tokenization_method = value.get().unwrap()
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            OUTPUT_LANG_CODE_PROPERTY => self.settings.lock().unwrap().language_code.to_value(),
            TRANSLATION_TOKENIZATION_PROPERTY => {
                self.settings.lock().unwrap().tokenization_method.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TranslateSrcPad {}

impl PadImpl for TranslateSrcPad {}
