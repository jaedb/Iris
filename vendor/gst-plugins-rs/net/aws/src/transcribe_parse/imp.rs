// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{element_imp_error, error_msg};
use serde_derive::Deserialize;

use std::sync::LazyLock;

use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "awstranscribeparse",
        gst::DebugColorFlags::empty(),
        Some("AWS transcript parser"),
    )
});

struct State {
    adapter: gst_base::UniqueAdapter,
}

impl Default for State {
    fn default() -> Self {
        Self {
            adapter: gst_base::UniqueAdapter::new(),
        }
    }
}

pub struct TranscribeParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct Alternative {
    #[allow(dead_code)]
    confidence: serde_json::Value,
    content: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct Item {
    start_time: Option<String>,
    end_time: Option<String>,
    alternatives: Vec<Alternative>,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Results {
    #[allow(dead_code)]
    transcripts: serde_json::Value,
    items: Vec<Item>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Transcript {
    #[allow(dead_code)]
    job_name: String,
    #[allow(dead_code)]
    account_id: String,
    results: Results,
}

impl TranscribeParse {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();

        state.adapter.push(buffer);

        Ok(gst::FlowSuccess::Ok)
    }

    fn drain(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let available = state.adapter.available();
        let buffer = state
            .adapter
            .take_buffer(available)
            .unwrap()
            .into_mapped_buffer_readable()
            .unwrap();
        drop(state);

        self.srcpad.push_event(gst::event::Caps::new(
            &gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build(),
        ));
        self.srcpad
            .push_event(gst::event::Segment::new(&gst::FormattedSegment::<
                gst::format::Time,
            >::new()));
        let json = std::str::from_utf8(buffer.as_slice()).map_err(|err| {
            error_msg!(
                gst::StreamError::Failed,
                ["Couldn't parse input as utf8: {}", err]
            )
        })?;
        let mut transcript: Transcript = serde_json::from_str(json).map_err(|err| {
            error_msg!(
                gst::StreamError::Failed,
                ["Unexpected transcription format: {}", err]
            )
        })?;

        let mut last_pts = gst::ClockTime::ZERO;

        for mut item in transcript.results.items.drain(..) {
            match item.type_.as_str() {
                "punctuation" => {
                    if !item.alternatives.is_empty() {
                        let alternative = item.alternatives.remove(0);
                        let mut outbuf =
                            gst::Buffer::from_mut_slice(alternative.content.into_bytes());

                        {
                            let outbuf = outbuf.get_mut().unwrap();

                            outbuf.set_pts(last_pts);
                            outbuf.set_duration(gst::ClockTime::ZERO);
                        }

                        self.srcpad.push(outbuf).map_err(|err| {
                            error_msg!(
                                gst::StreamError::Failed,
                                ["Failed to push transcript item: {}", err]
                            )
                        })?;
                    }
                }
                "pronunciation" => {
                    let start_time: f64 = match item.start_time.as_ref().unwrap().parse() {
                        Ok(start_time) => start_time,
                        Err(err) => {
                            return Err(error_msg!(
                                gst::StreamError::Failed,
                                ["Failed to parse start_time as float ({})", err]
                            ));
                        }
                    };
                    let end_time: f64 = match item.end_time.as_ref().unwrap().parse() {
                        Ok(end_time) => end_time,
                        Err(err) => {
                            return Err(error_msg!(
                                gst::StreamError::Failed,
                                ["Failed to parse end_time as float ({})", err]
                            ));
                        }
                    };

                    let start_pts = ((start_time * 1_000_000_000.0) as u64).nseconds();
                    let end_pts = ((end_time * 1_000_000_000.0) as u64).nseconds();
                    let duration = end_pts.saturating_sub(start_pts);

                    if start_pts > last_pts {
                        let gap_event = gst::event::Gap::builder(last_pts)
                            .duration(start_pts - last_pts)
                            .build();
                        if !self.srcpad.push_event(gap_event) {
                            return Err(error_msg!(
                                gst::StreamError::Failed,
                                ["Failed to push gap"]
                            ));
                        }
                    }

                    if !(item.alternatives.is_empty()) {
                        let alternative = item.alternatives.remove(0);
                        let mut outbuf =
                            gst::Buffer::from_mut_slice(alternative.content.into_bytes());

                        {
                            let outbuf = outbuf.get_mut().unwrap();

                            outbuf.set_pts(start_pts);
                            outbuf.set_duration(duration);
                        }

                        self.srcpad.push(outbuf).map_err(|err| {
                            error_msg!(
                                gst::StreamError::Failed,
                                ["Failed to push transcript item: {}", err]
                            )
                        })?;

                        last_pts = end_pts;
                    }
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::FlushStop(..) => {
                let mut state = self.state.lock().unwrap();
                *state = State::default();
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Eos(..) => match self.drain() {
                Ok(()) => gst::Pad::event_default(pad, Some(&*self.obj()), event),
                Err(err) => {
                    gst::error!(CAT, imp = self, "failed to drain on EOS: {}", err);
                    element_imp_error!(
                        self,
                        gst::StreamError::Failed,
                        ["Streaming failed: {}", err]
                    );

                    false
                }
            },
            EventView::Segment(..) | EventView::Caps(..) => true,
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TranscribeParse {
    const NAME: &'static str = "GstAwsTranscribeParse";
    type Type = super::TranscribeParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                TranscribeParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TranscribeParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::from_template(&templ);

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
        }
    }
}

impl ObjectImpl for TranscribeParse {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for TranscribeParse {}

impl ElementImpl for TranscribeParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "AWS transcript parser",
                "Text/Subtitle",
                "Parses AWS transcripts into timed text buffers",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-json").build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
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
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
