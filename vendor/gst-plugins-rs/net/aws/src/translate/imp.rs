// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! AWS Translate element.
//!
//! This element translates text

use super::CAT;
use crate::s3utils::RUNTIME;
use crate::transcriber::{
    translate::{span_tokenize_items, TranslatedItem},
    TranslationTokenizationMethod,
};
use anyhow::{anyhow, Error};
use aws_sdk_s3::config::StalledStreamProtectionConfig;
use aws_sdk_translate::error::ProvideErrorMetadata;
use futures::future::{abortable, AbortHandle};
use gst::subclass::prelude::*;
use gst::{glib, prelude::*};
use std::sync::LazyLock;
use std::sync::{Mutex, MutexGuard};

#[allow(deprecated)]
static AWS_BEHAVIOR_VERSION: LazyLock<aws_config::BehaviorVersion> =
    LazyLock::new(aws_config::BehaviorVersion::v2023_11_09);

const DEFAULT_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(2);
const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::from_seconds(0);
const DEFAULT_REGION: &str = "us-east-1";
const DEFAULT_INPUT_LANG_CODE: &str = "en-US";
const DEFAULT_OUTPUT_LANG_CODE: &str = "fr-FR";
const DEFAULT_TOKENIZATION_METHOD: TranslationTokenizationMethod =
    TranslationTokenizationMethod::SpanBased;
const DEFAULT_BREVITY_ON: bool = false;

const STANDARD_JOINABLE_PUNCTUATION: &str = "!.?,:;";
const FRENCH_JOINABLE_PUNCTUATION: &str = ".,";

#[derive(Debug, Clone)]
struct Settings {
    latency: gst::ClockTime,
    accumulator_lateness: gst::ClockTime,
    input_language_code: String,
    output_language_code: String,
    access_key: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    tokenization_method: TranslationTokenizationMethod,
    brevity_on: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            accumulator_lateness: DEFAULT_LATENESS,
            input_language_code: String::from(DEFAULT_INPUT_LANG_CODE),
            output_language_code: String::from(DEFAULT_OUTPUT_LANG_CODE),
            access_key: None,
            secret_access_key: None,
            session_token: None,
            tokenization_method: DEFAULT_TOKENIZATION_METHOD,
            brevity_on: DEFAULT_BREVITY_ON,
        }
    }
}

#[derive(Debug, Clone)]
struct InputItem {
    content: String,
    pts: gst::ClockTime,
    end_pts: gst::ClockTime,
    starts_with_joinable_punctuation: bool,
    discont: bool,
    awstranscribe_item: Option<String>,
    speechmatics_items: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct InputItems(Vec<InputItem>);

impl InputItems {
    fn start_pts(&self) -> Option<gst::ClockTime> {
        self.0.first().map(|item| item.pts)
    }

    fn discont(&self) -> bool {
        self.0.first().map(|item| item.discont).unwrap_or(false)
    }
}

enum TranslateOutput {
    Item(gst::Buffer),
}

struct State {
    // (live, min, max)
    upstream_latency: Option<(bool, gst::ClockTime, Option<gst::ClockTime>)>,
    segment: Option<gst::FormattedSegment<gst::ClockTime>>,
    client: Option<aws_sdk_translate::Client>,
    send_abort_handle: Option<AbortHandle>,
    discont: bool,
    seqnum: gst::Seqnum,
    current_speaker: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            upstream_latency: None,
            segment: None,
            client: None,
            send_abort_handle: None,
            discont: false,
            seqnum: gst::Seqnum::next(),
            current_speaker: None,
        }
    }
}

pub struct Translate {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    aws_config: Mutex<Option<aws_config::SdkConfig>>,
}

const SPAN_START: &str = "<span>";
const SPAN_END: &str = "</span>";

impl Translate {
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

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::trace!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        match event.view() {
            FlushStart(_) => {
                gst::info!(CAT, imp = self, "received flush start, disconnecting");
                let ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                self.disconnect();
                ret
            }
            StreamStart(_) => {
                self.state.lock().unwrap().seqnum = event.seqnum();

                gst::debug!(
                    CAT,
                    imp = self,
                    "stored stream start seqnum {:?}",
                    event.seqnum()
                );

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            Caps(_) => {
                let caps = gst::Caps::builder("text/x-raw")
                    .field("format", "utf8")
                    .build();

                let event = gst::event::Caps::builder(&caps)
                    .seqnum(self.state.lock().unwrap().seqnum)
                    .build();

                self.srcpad.push_event(event)
            }
            Segment(e) => {
                {
                    let mut state = self.state.lock().unwrap();

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

                    state.segment = Some(segment);

                    gst::debug!(CAT, imp = self, "stored segment {:?}", state.segment);
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            CustomDownstream(c) => {
                let Some(s) = c.structure() else {
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                if s.name().as_str() == "rstranscribe/speaker-change" {
                    gst::debug!(CAT, imp = self, "speaker change, draining");
                    self.state.lock().unwrap().current_speaker = s.get::<String>("speaker").ok();
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    async fn send(&self, to_translate: InputItems) -> Result<Vec<TranslateOutput>, Error> {
        let (input_lang, output_lang, latency, tokenization_method, lateness, brevity_on) = {
            let settings = self.settings.lock().unwrap();
            (
                settings.input_language_code.clone(),
                settings.output_language_code.clone(),
                settings.latency,
                settings.tokenization_method,
                settings.accumulator_lateness,
                settings.brevity_on,
            )
        };

        let (client, segment, speaker) = {
            let state = self.state.lock().unwrap();

            let Some(segment) = state.segment.clone() else {
                return Err(anyhow!("buffer received before segment"));
            };

            (
                state.client.as_ref().unwrap().clone(),
                segment,
                state.current_speaker.clone(),
            )
        };

        let mut output = vec![];

        gst::log!(
            CAT,
            imp = self,
            "translating sentence {}",
            to_translate
                .0
                .iter()
                .map(|i| i.content.clone())
                .collect::<Vec<_>>()
                .join(" ")
        );

        let mut ts_duration_list: Vec<(gst::ClockTime, gst::ClockTime)> = vec![];
        let mut content: Vec<String> = vec![];
        let mut it = to_translate.0.iter().peekable();

        let items: [String; 0] = [];
        let mut original_aws_items = gst::List::new(items);
        let items: [String; 0] = [];
        let mut original_speechmatics_items = gst::List::new(items);

        while let Some(item) = it.next() {
            let suffix = match it.peek() {
                Some(next_item) => {
                    if next_item.starts_with_joinable_punctuation {
                        ""
                    } else {
                        " "
                    }
                }
                None => "",
            };
            ts_duration_list.push((item.pts, item.end_pts - item.pts));
            content.push(match tokenization_method {
                TranslationTokenizationMethod::None => format!("{}{}", item.content, suffix),
                TranslationTokenizationMethod::SpanBased => {
                    format!("{SPAN_START}{}{SPAN_END}{}", item.content, suffix)
                }
            });

            if let Some(ref original_item) = item.awstranscribe_item {
                original_aws_items.append(original_item.to_send_value());
            }

            if let Some(ref original_items) = item.speechmatics_items {
                for original_item in original_items {
                    original_speechmatics_items.append(original_item.to_send_value());
                }
            }
        }

        let content: String = content.join("");

        gst::debug!(
            CAT,
            imp = self,
            "translating {content} with duration list: {ts_duration_list:?}"
        );

        let mut translate_settings_builder =
            aws_sdk_translate::types::TranslationSettings::builder();

        if brevity_on {
            translate_settings_builder =
                translate_settings_builder.set_brevity(Some(aws_sdk_translate::types::Brevity::On));
        }

        let translated_text = client
            .translate_text()
            .set_source_language_code(Some(input_lang))
            .set_target_language_code(Some(output_lang.clone()))
            .set_settings(Some(translate_settings_builder.build()))
            .set_text(Some(content))
            .send()
            .await
            .map_err(|err| anyhow!("{}: {}", err, err.meta()))?
            .translated_text;

        gst::log!(CAT, imp = self, "translation received: {translated_text}");

        let upstream_latency = self.upstream_latency();

        let mut translated_items = match tokenization_method {
            TranslationTokenizationMethod::None => {
                // Push translation as a single item
                let mut ts_duration_iter = ts_duration_list.into_iter().peekable();

                let &(first_pts, _) = ts_duration_iter.peek().expect("at least one item");
                let (last_pts, last_duration) = ts_duration_iter.last().expect("at least one item");

                gst::trace!(
                    CAT,
                    imp = self,
                    "not performing tokenization, pushing single item"
                );

                vec![TranslatedItem {
                    pts: first_pts,
                    duration: last_pts.saturating_sub(first_pts) + last_duration,
                    content: translated_text.clone(),
                }]
            }
            TranslationTokenizationMethod::SpanBased => {
                gst::trace!(CAT, imp = self, "tokenizing from spans");

                span_tokenize_items(&translated_text, ts_duration_list)
            }
        };

        gst::log!(CAT, imp = self, "translated items: {translated_items:?}");

        let mut translated_items_builder = gst::Structure::builder("awstranslate/items");

        let mut end_pts: Option<gst::ClockTime> = None;

        for mut item in translated_items.drain(..) {
            translated_items_builder = translated_items_builder.field(&item.content, item.pts);

            item.pts += lateness;

            if let Some((upstream_live, upstream_min, _)) = upstream_latency {
                if upstream_live {
                    if let Some(now) = self.obj().current_running_time() {
                        let start_rtime = segment.to_running_time(item.pts).unwrap();
                        let deadline = start_rtime + upstream_min + latency;

                        if deadline < now {
                            let adjusted_pts = item.pts + now - deadline;
                            gst::warning!(
                                CAT,
                                imp = self,
                                "text translated too late, adjusting timestamp {} -> {adjusted_pts}", item.pts,
                            );
                            item.pts = adjusted_pts;
                        }
                    }
                }
            }

            let mut buf = gst::Buffer::from_mut_slice(item.content.into_bytes());
            {
                let buf_mut = buf.get_mut().unwrap();
                buf_mut.set_pts(item.pts);
                buf_mut.set_duration(item.duration);

                if to_translate.discont() {
                    gst::trace!(CAT, imp = self, "marking buffer discont");
                    buf_mut.set_flags(gst::BufferFlags::DISCONT);
                }
            }

            end_pts = Some(item.pts);

            output.push(TranslateOutput::Item(buf));
        }

        let mut message_builder = gst::Structure::builder("awstranslate/raw")
            .field("translation", translated_items_builder.build())
            .field("arrival-time", self.obj().current_running_time())
            .field("start-time", to_translate.start_pts())
            .field("language-code", &output_lang);

        if let Some(speaker) = speaker {
            message_builder = message_builder.field("speaker", speaker);
        }

        if !original_aws_items.is_empty() {
            message_builder =
                message_builder.field("original-awstranscribe-items", original_aws_items);
        }

        if !original_speechmatics_items.is_empty() {
            message_builder =
                message_builder.field("original-speechmatics-items", original_speechmatics_items);
        }

        let _ = self.obj().post_message(
            gst::message::Element::builder(message_builder.build())
                .src(&*self.obj())
                .build(),
        );

        if let Some(end_pts) = end_pts {
            self.state
                .lock()
                .unwrap()
                .segment
                .as_mut()
                .unwrap()
                .set_position(end_pts);
        }

        Ok(output)
    }

    fn ensure_connection(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if state.client.is_none() {
            state.client = Some(aws_sdk_translate::Client::new(
                self.aws_config.lock().unwrap().as_ref().expect("prepared"),
            ));
        }
        Ok(())
    }

    fn disconnect(&self) {
        let mut state = self.state.lock().unwrap();

        if let Some(abort_handle) = state.send_abort_handle.take() {
            gst::info!(CAT, imp = self, "aborting translation sending");
            abort_handle.abort();
        }

        *state = State::default()
    }

    fn input_item_from_buffer(
        &self,
        state: &mut State,
        buffer: &gst::BufferRef,
        is_french: bool,
    ) -> Result<Option<InputItem>, gst::FlowError> {
        let Some(pts) = buffer.pts() else {
            gst::warning!(CAT, imp = self, "dropping first buffer without a PTS");
            return Ok(None);
        };

        let end_pts = match buffer.duration() {
            Some(duration) => duration + pts,
            _ => pts,
        };

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let data = String::from_utf8(data.to_vec()).map_err(|err| {
            gst::error!(CAT, imp = self, "Can't decode utf8: {}", err);

            gst::FlowError::Error
        })?;

        let original_aws_item: Option<String> =
            gst::meta::CustomMeta::from_buffer(buffer, "AWSTranscribeItemMeta")
                .ok()
                .and_then(|m| m.structure().get::<String>("item").ok());

        let original_speechmatics_items: Option<Vec<String>> =
            gst::meta::CustomMeta::from_buffer(buffer, "SpeechmaticsItemMeta")
                .ok()
                .and_then(|m| m.structure().get::<Vec<String>>("items").ok());

        let joinable_punctuation = if is_french {
            FRENCH_JOINABLE_PUNCTUATION
        } else {
            STANDARD_JOINABLE_PUNCTUATION
        };

        let starts_with_joinable_punctuation = data
            .chars()
            .next()
            .map(|c| joinable_punctuation.contains(c))
            .unwrap_or(false);

        let discont = state.discont;
        state.discont = false;

        Ok(Some(InputItem {
            content: data,
            pts,
            end_pts,
            starts_with_joinable_punctuation,
            discont,
            awstranscribe_item: original_aws_item,
            speechmatics_items: original_speechmatics_items,
        }))
    }

    fn do_send(
        &self,
        mut state: MutexGuard<State>,
        to_translate: InputItems,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let (future, abort_handle) = abortable(self.send(to_translate));

        state.send_abort_handle = Some(abort_handle);

        drop(state);

        match RUNTIME.block_on(future) {
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
                Ok(mut output) => {
                    let mut bufferlist = gst::BufferList::new();
                    let bufferlist_mut = bufferlist.get_mut().unwrap();
                    for item in output.drain(..) {
                        match item {
                            TranslateOutput::Item(buffer) => {
                                gst::debug!(CAT, imp = self, "pushing {buffer:?}");
                                bufferlist_mut.add(buffer);
                            }
                        }
                    }

                    self.srcpad.push_list(bufferlist)
                }
            },
        }
    }

    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.ensure_connection().map_err(|err| {
            gst::error!(CAT, "Failed to connect to AWS: {err:?}");
            gst::element_imp_error!(
                self,
                gst::StreamError::Failed,
                ["Streaming failed: {}", err]
            );

            gst::FlowError::Error
        })?;

        let is_french = self
            .settings
            .lock()
            .unwrap()
            .input_language_code
            .starts_with("fr-");

        let mut state = self.state.lock().unwrap();

        let to_translate =
            if let Some(item) = self.input_item_from_buffer(&mut state, &buffer, is_french)? {
                vec![item]
            } else {
                return Ok(gst::FlowSuccess::Ok);
            };

        self.do_send(state, InputItems(to_translate))
    }

    fn sink_chain_list(
        &self,
        _pad: &gst::Pad,
        bufferlist: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.ensure_connection().map_err(|err| {
            gst::error!(CAT, "Failed to connect to AWS: {err:?}");
            gst::element_imp_error!(
                self,
                gst::StreamError::Failed,
                ["Streaming failed: {}", err]
            );

            gst::FlowError::Error
        })?;

        let is_french = self
            .settings
            .lock()
            .unwrap()
            .input_language_code
            .starts_with("fr-");

        let mut state = self.state.lock().unwrap();

        let mut to_translate: Vec<InputItem> = vec![];
        for buffer in bufferlist.iter() {
            if let Some(item) = self.input_item_from_buffer(&mut state, buffer, is_french)? {
                to_translate.push(item);
            }
        }

        self.do_send(state, InputItems(to_translate))
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

        gst::log!(CAT, imp = self, "Loading aws config...");
        let _enter_guard = RUNTIME.enter();

        let config_loader = match (access_key, secret_access_key) {
            (Some(key), Some(secret_key)) => {
                gst::log!(CAT, imp = self, "Using settings credentials");
                aws_config::defaults(*AWS_BEHAVIOR_VERSION).credentials_provider(
                    aws_sdk_translate::config::Credentials::new(
                        key,
                        secret_key,
                        session_token,
                        None,
                        "translate",
                    ),
                )
            }
            _ => {
                gst::log!(CAT, imp = self, "Attempting to get credentials from env...");
                aws_config::defaults(*AWS_BEHAVIOR_VERSION)
            }
        };

        let config_loader = config_loader.region(
            aws_config::meta::region::RegionProviderChain::default_provider()
                .or_else(DEFAULT_REGION),
        );

        let config_loader =
            config_loader.stalled_stream_protection(StalledStreamProtectionConfig::disabled());

        let config = futures::executor::block_on(config_loader.load());
        gst::log!(CAT, imp = self, "Using region {}", config.region().unwrap());

        *self.aws_config.lock().unwrap() = Some(config);

        gst::debug!(CAT, imp = self, "Prepared");

        Ok(())
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::trace!(CAT, obj = pad, "Handling query {:?}", query);

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
impl ObjectSubclass for Translate {
    const NAME: &'static str = "GstAwsTranslate";
    type Type = super::Translate;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Translate::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |translate| translate.sink_chain(pad, buffer),
                )
            })
            .chain_list_function(|pad, parent, buffer| {
                Translate::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |translate| translate.sink_chain_list(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Translate::catch_panic_pad_function(
                    parent,
                    || false,
                    |translate| translate.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .query_function(|pad, parent, query| {
                Translate::catch_panic_pad_function(
                    parent,
                    || false,
                    |translate| translate.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            settings: Default::default(),
            state: Default::default(),
            aws_config: Mutex::new(None),
        }
    }
}

impl ObjectImpl for Translate {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow AWS translate")
                    .default_value(DEFAULT_LATENCY.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
                    .build(),
                /**
                 * gstawstranslate:accumulator-lateness
                 *
                 * The timestamps of the translated text will be shifted forward
                 * by the value of this property.
                 *
                 * Deprecated:plugins-rs-0.15.0: use a textaccumulate element upstream instead
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecUInt::builder("accumulator-lateness")
                    .nick("Lateness")
                    .blurb("By how much to shift input timestamps forward")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .deprecated()
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
                glib::ParamSpecString::builder("input-language-code")
                    .nick("Input Language Code")
                    .blurb("The Language of the input stream")
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("output-language-code")
                    .nick("Input Language Code")
                    .blurb("The Language of the output stream")
                    .default_value(Some(DEFAULT_INPUT_LANG_CODE))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder("tokenization-method")
                    .nick("Translations tokenization method")
                    .blurb("The tokenization method to apply")
                    .default_value(DEFAULT_TOKENIZATION_METHOD)
                    .mutable_ready()
                    .build(),
                /**
                 * gstawstranslate:brevity-on
                 *
                 * Turn on the brevity feature, only available for some languages.
                 *
                 * https://docs.aws.amazon.com/translate/latest/dg/customizing-translations-brevity.html
                 *
                 * since: plugins-rs-0.15.0
                 */
                glib::ParamSpecBoolean::builder("brevity-on")
                    .nick("Brevity On")
                    .blurb("Whether brevity should be turned on")
                    .default_value(DEFAULT_BREVITY_ON)
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
            "latency" => {
                let mut settings = self.settings.lock().unwrap();
                settings.latency = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "accumulator-lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.accumulator_lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
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
            "input-language-code" => {
                let language_code: String = value.get().expect("type checked upstream");
                let mut settings = self.settings.lock().unwrap();
                settings.input_language_code = language_code;
            }
            "output-language-code" => {
                let language_code: String = value.get().expect("type checked upstream");
                let mut settings = self.settings.lock().unwrap();
                settings.output_language_code = language_code;
            }
            "tokenization-method" => {
                self.settings.lock().unwrap().tokenization_method = value.get().unwrap()
            }
            "brevity-on" => self.settings.lock().unwrap().brevity_on = value.get().unwrap(),
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "latency" => {
                let settings = self.settings.lock().unwrap();
                (settings.latency.mseconds() as u32).to_value()
            }
            "accumulator-lateness" => {
                let settings = self.settings.lock().unwrap();
                (settings.accumulator_lateness.mseconds() as u32).to_value()
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
            "input-language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.input_language_code.to_value()
            }
            "output-language-code" => {
                let settings = self.settings.lock().unwrap();
                settings.output_language_code.to_value()
            }
            "tokenization-method" => self.settings.lock().unwrap().tokenization_method.to_value(),
            "brevity-on" => self.settings.lock().unwrap().brevity_on.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Translate {}

impl ElementImpl for Translate {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Aws Translator",
                "Text/Filter",
                "Translates text",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
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
        gst::info!(CAT, imp = self, "Changing state {transition:?}");

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
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

    fn provide_clock(&self) -> Option<gst::Clock> {
        Some(gst::SystemClock::obtain())
    }
}
