// Copyright (C) 2025 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! Text accumulator element
//!
//! This element accumulates text

use super::CAT;
use anyhow::{anyhow, Error};
use gst::subclass::prelude::*;
use gst::{glib, prelude::*};
use itertools::Itertools;
use regex::Regex;
use std::collections::VecDeque;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::LazyLock;
use std::sync::{mpsc, Condvar, Mutex};

const DEFAULT_LATENCY: gst::ClockTime = gst::ClockTime::from_seconds(3);
const DEFAULT_LATENESS: gst::ClockTime = gst::ClockTime::from_seconds(0);
const DEFAULT_TIMEOUT_TERMINATORS: &str = r"\,\s|\:\s|\;\s";
const DEFAULT_DRAIN_ON_FINAL_TRANSCRIPTS: bool = true;
const DEFAULT_DRAIN_ON_SPEAKER_CHANGE: bool = true;
const DEFAULT_EXTEND_DURATION: bool = false;
const DEFAULT_EXTENDED_DURATION_GAP: gst::ClockTime = gst::ClockTime::from_mseconds(500);

#[derive(Debug, Clone)]
struct Settings {
    latency: gst::ClockTime,
    lateness: gst::ClockTime,
    timeout_terminators: String,
    drain_on_final_transcripts: bool,
    drain_on_speaker_change: bool,
    extend_duration: bool,
    extended_duration_gap: gst::ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            lateness: DEFAULT_LATENESS,
            timeout_terminators: DEFAULT_TIMEOUT_TERMINATORS.to_string(),
            drain_on_final_transcripts: DEFAULT_DRAIN_ON_FINAL_TRANSCRIPTS,
            drain_on_speaker_change: DEFAULT_DRAIN_ON_SPEAKER_CHANGE,
            extend_duration: DEFAULT_EXTEND_DURATION,
            extended_duration_gap: DEFAULT_EXTENDED_DURATION_GAP,
        }
    }
}

#[derive(Debug, Clone)]
struct Item {
    content: String,
    pts: gst::ClockTime,
    rtime: gst::ClockTime,
    duration: gst::ClockTime,
    buffer: Option<gst::Buffer>,
}

#[derive(Debug)]
pub struct Input {
    segmenter: icu_segmenter::SentenceSegmenter,
    items: VecDeque<Item>,
}

impl Input {
    fn try_new(language_identifier: Option<&str>) -> Result<Self, Error> {
        let mut options = icu_segmenter::options::SentenceBreakOptions::default();
        let id = match language_identifier {
            Some(id) => Some(icu_locale::LanguageIdentifier::try_from_str(id)?),
            None => None,
        };
        options.content_locale = id.as_ref();
        Ok(Self {
            segmenter: icu_segmenter::SentenceSegmenter::try_new(options)?,
            items: VecDeque::default(),
        })
    }
}

impl Input {
    fn start_rtime(&self) -> Option<gst::ClockTime> {
        self.items.front().map(|item| item.rtime)
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn push(
        &mut self,
        content: String,
        pts: gst::ClockTime,
        rtime: gst::ClockTime,
        duration: gst::ClockTime,
        buffer: Option<gst::Buffer>,
    ) {
        self.items.push_back(Item {
            content,
            pts,
            rtime,
            duration,
            buffer,
        });
    }

    fn drain_to_idx(&mut self, idx: usize) -> Option<Vec<Item>> {
        let mut ret = vec![];

        let mut offset = 0;

        loop {
            if offset >= idx {
                break;
            }

            let mut item = self.items.pop_front().unwrap();

            if offset + item.content.len() <= idx {
                offset += item.content.len() + 1;
                ret.push(item);
            } else {
                let original_duration = item.duration;

                item.duration = gst::ClockTime::from_nseconds(
                    ((idx - offset) as u64)
                        .mul_div_floor(item.duration.nseconds(), item.content.len() as u64)
                        .unwrap(),
                );

                let new_content = item.content.split_off(idx - offset);

                let new_item = Item {
                    content: new_content,
                    pts: item.pts + item.duration,
                    rtime: item.rtime + item.duration,
                    duration: original_duration - item.duration,
                    buffer: item.buffer.as_ref().cloned(),
                };

                ret.push(item);
                self.items.push_front(new_item);

                break;
            }
        }

        if ret.is_empty() {
            None
        } else {
            Some(ret)
        }
    }

    fn next_sentence(&mut self) -> Option<Vec<Item>> {
        let content = self.items.iter().map(|item| item.content.clone()).join(" ");

        let (_start, end) = self
            .segmenter
            .as_borrowed()
            .segment_str(&content)
            .tuple_windows()
            .next()?;

        if end < content.len() {
            self.drain_to_idx(end)
        } else {
            None
        }
    }

    fn drain_to_next_terminator(&mut self, timeout_terminators_regex: &Regex) -> Option<Vec<Item>> {
        let content = self.items.iter().map(|item| item.content.clone()).join(" ");

        if let Some(idx) = timeout_terminators_regex.find_iter(&content).last() {
            self.drain_to_idx(idx.end())
        } else {
            self.drain_all()
        }
    }

    fn timeout(
        &mut self,
        now: gst::ClockTime,
        latency: gst::ClockTime,
        lateness: gst::ClockTime,
        timeout_terminators_regex: &Regex,
    ) -> Option<Vec<Item>> {
        if let Some(start_rtime) = self.start_rtime() {
            if start_rtime + latency + lateness < now {
                gst::debug!(
                    CAT,
                    "draining on timeout: {start_rtime} + {latency} + {lateness} < {now}",
                );
                self.drain_to_next_terminator(timeout_terminators_regex)
            } else {
                gst::trace!(
                    CAT,
                    "queued content is not late: {start_rtime} + {latency} >= {now}"
                );
                None
            }
        } else {
            gst::trace!(CAT, "no queued content, cannot be late");
            None
        }
    }

    fn drain_all(&mut self) -> Option<Vec<Item>> {
        gst::log!(CAT, "drained all tokens: {:?}", self.items);

        let ret: Vec<Item> = self.items.split_off(0).into();

        if ret.is_empty() {
            None
        } else {
            Some(ret)
        }
    }
}

enum AccumulateInput {
    Items(Vec<Item>),
    Gap {
        pts: gst::ClockTime,
        duration: Option<gst::ClockTime>,
    },
    Event(gst::Event),
    Query(std::ptr::NonNull<gst::QueryRef>),
    RecalculateTimeout,
}

// SAFETY: Need to be able to pass *mut gst::QueryRef
unsafe impl Send for AccumulateInput {}

struct State {
    // (live, min, max)
    upstream_latency: Option<(bool, gst::ClockTime, Option<gst::ClockTime>)>,
    segment: gst::FormattedSegment<gst::ClockTime>,
    input: Option<Input>,
    accumulate_tx: Option<mpsc::SyncSender<AccumulateInput>>,
    serialized_query_return: Option<bool>,
    seqnum: gst::Seqnum,
    timeout_terminators_regex: Option<Regex>,
    task_started: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            upstream_latency: None,
            segment: gst::FormattedSegment::new(),
            input: None,
            accumulate_tx: None,
            serialized_query_return: None,
            seqnum: gst::Seqnum::next(),
            timeout_terminators_regex: None,
            task_started: false,
        }
    }
}

impl State {
    fn timeout(
        &mut self,
        now: gst::ClockTime,
        latency: gst::ClockTime,
        lateness: gst::ClockTime,
    ) -> Option<Vec<Item>> {
        if let Some(input) = self.input.as_mut() {
            input.timeout(
                now,
                latency,
                lateness,
                self.timeout_terminators_regex
                    .as_ref()
                    .expect("valid regex generated at state change"),
            )
        } else {
            None
        }
    }
}

// Locking order: settings -> state
pub struct Accumulate {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    serialized_query_cond: Condvar,
}

impl Accumulate {
    fn prepare(&self) -> Result<(), Error> {
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        state.input = Some(
            Input::try_new(None).map_err(|err| anyhow!("Failed to create segmenter: {err:?}"))?,
        );

        state.timeout_terminators_regex = Some(
            Regex::new(&settings.timeout_terminators)
                .map_err(|err| anyhow!("Failed to create timeout terminators regex: {err:?}"))?,
        );

        Ok(())
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

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::trace!(CAT, obj = pad, "Handling event {event:?}");

        use gst::EventView::*;
        match event.view() {
            FlushStart(_) => {
                gst::info!(CAT, imp = self, "received flush start, pausing");
                let ret = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                let _ = self.state.lock().unwrap().accumulate_tx.take();
                let _ = self.srcpad.pause_task();
                self.state.lock().unwrap().task_started = false;
                ret
            }
            FlushStop(_) => {
                if let Err(err) = self.start_srcpad_task() {
                    gst::error!(CAT, imp = self, "Failed to start srcpad task: {err}");
                    return false;
                }

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            StreamStart(_) => {
                if let Err(err) = self.start_srcpad_task() {
                    gst::error!(CAT, imp = self, "Failed to start srcpad task: {err}");
                    return false;
                }

                self.state.lock().unwrap().seqnum = event.seqnum();

                gst::debug!(
                    CAT,
                    imp = self,
                    "stored stream start seqnum {:?}",
                    event.seqnum()
                );

                let (items, accumulate_tx) = {
                    let mut state = self.state.lock().unwrap();

                    (
                        state.input.as_mut().and_then(|input| input.drain_all()),
                        state.accumulate_tx.clone(),
                    )
                };

                if let Some(tx) = accumulate_tx {
                    if let Some(items) = items {
                        let _ = tx.send(AccumulateInput::Items(items));
                    }

                    let _ = tx.send(AccumulateInput::Event(event));
                }

                true
            }
            Segment(e) => {
                let (items, accumulate_tx, event) = {
                    let has_segment_event =
                        self.srcpad.sticky_event::<gst::event::Segment>(0).is_some();
                    let settings = self.settings.lock().unwrap();
                    let mut state = self.state.lock().unwrap();

                    let mut segment = match e.segment().clone().downcast::<gst::ClockTime>() {
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

                    if state.segment == segment && has_segment_event {
                        return true;
                    }

                    state.segment = segment.clone();

                    gst::debug!(CAT, imp = self, "stored segment {:?}", state.segment);

                    segment.set_base(
                        segment.base().unwrap_or(gst::ClockTime::ZERO) + settings.lateness,
                    );

                    (
                        state.input.as_mut().and_then(|input| input.drain_all()),
                        state.accumulate_tx.clone(),
                        gst::event::Segment::builder(&segment)
                            .seqnum(state.seqnum)
                            .build(),
                    )
                };

                if let Some(tx) = accumulate_tx {
                    if let Some(items) = items {
                        let _ = tx.send(AccumulateInput::Items(items));
                    }
                    let _ = tx.send(AccumulateInput::Event(event));
                }

                true
            }
            Caps(_) => {
                let accumulate_tx = self.state.lock().unwrap().accumulate_tx.clone();
                if let Some(tx) = accumulate_tx {
                    let _ = tx.send(AccumulateInput::Event(event));
                }

                true
            }
            Gap(gap) => {
                let state = self.state.lock().unwrap();

                let (pts, duration) = gap.get();

                /* We only insert the gap when the input is empty, as new gaps will otherwise be output
                 * when the position of the accumulator progresses */
                if state
                    .input
                    .as_ref()
                    .map(|input| input.is_empty())
                    .unwrap_or(true)
                {
                    if let Some(accumulate_tx) = state.accumulate_tx.clone() {
                        drop(state);
                        let _ = accumulate_tx.send(AccumulateInput::Gap { pts, duration });
                    }
                }

                true
            }
            Eos(_) => {
                let (items, accumulate_tx) = {
                    let mut state = self.state.lock().unwrap();
                    (
                        state.input.as_mut().and_then(|input| input.drain_all()),
                        state.accumulate_tx.take(),
                    )
                };

                if let Some(tx) = accumulate_tx {
                    gst::debug!(CAT, imp = self, "received EOS, draining");
                    if let Some(items) = items {
                        let _ = tx.send(AccumulateInput::Items(items));
                    }
                    let _ = tx.send(AccumulateInput::Event(event));
                }

                true
            }
            CustomDownstream(c) => {
                let Some(s) = c.structure() else {
                    return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                };

                let drain = match s.name().as_str() {
                    "rstranscribe/final-transcript" => {
                        if self.settings.lock().unwrap().drain_on_final_transcripts {
                            gst::log!(CAT, imp = self, "transcript is final, draining");
                            true
                        } else {
                            false
                        }
                    }
                    "rstranscribe/speaker-change" => {
                        if self.settings.lock().unwrap().drain_on_speaker_change {
                            gst::log!(CAT, imp = self, "speaker change, draining");
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if drain {
                    let (items, accumulate_tx) = {
                        let mut state = self.state.lock().unwrap();
                        (
                            state.input.as_mut().and_then(|input| input.drain_all()),
                            state.accumulate_tx.clone(),
                        )
                    };

                    if let Some(tx) = accumulate_tx {
                        if let Some(items) = items {
                            let _ = tx.send(AccumulateInput::Items(items));
                        }
                    }
                }

                let accumulate_tx = self.state.lock().unwrap().accumulate_tx.clone();

                if let Some(tx) = accumulate_tx {
                    let _ = tx.send(AccumulateInput::Event(event));
                }

                true
            }
            _ => {
                if event.is_serialized() {
                    let accumulate_tx = self.state.lock().unwrap().accumulate_tx.clone();
                    if let Some(tx) = accumulate_tx {
                        let _ = tx.send(AccumulateInput::Event(event));
                    }

                    true
                } else {
                    gst::Pad::event_default(pad, Some(&*self.obj()), event)
                }
            }
        }
    }

    fn maybe_push(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let Some(now) = self.obj().current_running_time() else {
            gst::trace!(
                CAT,
                imp = self,
                "no current running time, we cannot be late"
            );
            return Ok(gst::FlowSuccess::Ok);
        };

        let Some(upstream_latency) = self.upstream_latency() else {
            gst::trace!(CAT, imp = self, "no upstream latency, we cannot be late");
            return Ok(gst::FlowSuccess::Ok);
        };

        let (upstream_live, upstream_min, _) = upstream_latency;

        if !upstream_live {
            gst::trace!(CAT, imp = self, "upstream isn't live, we are not late");
            return Ok(gst::FlowSuccess::Ok);
        }

        let (latency, lateness) = {
            let settings = self.settings.lock().unwrap();
            (settings.latency, settings.lateness)
        };

        loop {
            let to_push = self
                .state
                .lock()
                .unwrap()
                .timeout(now, upstream_min + latency, lateness);

            if let Some(to_push) = to_push {
                gst::log!(CAT, imp = self, "drained on timeout: {:#?}", to_push);
                self.do_push(to_push)?;
            } else {
                return Ok(gst::FlowSuccess::Ok);
            }
        }
    }

    fn do_push(&self, mut to_push: Vec<Item>) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();

        if to_push.is_empty() {
            gst::trace!(CAT, imp = self, "nothing to push, returning early");
            return Ok(gst::FlowSuccess::Ok);
        }

        gst::trace!(CAT, imp = self, "Constructing output bufferlist");

        let mut bufferlist = gst::BufferList::new();
        let bufferlist_mut = bufferlist.get_mut().unwrap();
        let mut new_position = gst::ClockTime::NONE;

        for item in to_push.drain(..) {
            gst::trace!(
                CAT,
                imp = self,
                "adding buffer with content {} pts {} -> {}",
                item.content,
                item.pts,
                item.pts + item.duration,
            );

            let mut buf = gst::Buffer::from_mut_slice(item.content.into_bytes());
            {
                let buf_mut = buf.get_mut().unwrap();
                buf_mut.set_pts(item.pts);
                buf_mut.set_duration(item.duration);

                if let Some(original_buffer) = item.buffer {
                    let _ = original_buffer.copy_into(buf_mut, gst::BufferCopyFlags::META, ..);
                }
            }
            bufferlist_mut.add(buf);

            new_position = Some(item.pts + item.duration);
        }

        let gap = {
            let mut state = self.state.lock().unwrap();

            if let Some(position) = new_position {
                state.segment.set_position(position);
            }

            if let Some(next_item_pts) = state
                .input
                .as_ref()
                .and_then(|input| input.items.front().map(|item| item.pts))
            {
                let mut segment_position = state.segment.position().expect("position was set");

                if settings.extend_duration
                    && segment_position + settings.extended_duration_gap < next_item_pts
                {
                    let buffer = bufferlist_mut.get_mut(bufferlist_mut.len() - 1).unwrap();

                    let new_duration =
                        next_item_pts - settings.extended_duration_gap - buffer.pts().unwrap();

                    gst::debug!(
                        CAT,
                        imp = self,
                        "Updating last buffer duration from {} to {}",
                        buffer.duration().unwrap(),
                        new_duration
                    );

                    buffer.set_duration(new_duration);
                    state
                        .segment
                        .set_position(buffer.pts().unwrap() + buffer.duration().unwrap());
                    segment_position = buffer.pts().unwrap() + buffer.duration().unwrap();
                }

                if segment_position < next_item_pts {
                    gst::log!(
                        CAT,
                        "pushing our own gap {} {}",
                        segment_position,
                        next_item_pts - segment_position
                    );
                    let gap = gst::event::Gap::builder(segment_position)
                        .duration(next_item_pts - segment_position)
                        .seqnum(state.seqnum)
                        .build();

                    state.segment.set_position(next_item_pts);

                    Some(gap)
                } else {
                    None
                }
            } else {
                None
            }
        };

        self.srcpad.push_list(bufferlist)?;

        if let Some(gap) = gap {
            self.srcpad.push_event(gap);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn start_srcpad_task(&self) -> Result<(), gst::LoggableError> {
        if self.state.lock().unwrap().task_started {
            gst::debug!(CAT, imp = self, "Task started already");
            return Ok(());
        }

        gst::debug!(CAT, imp = self, "starting source pad task");

        let (accumulate_tx, accumulate_rx) = mpsc::sync_channel(0);

        self.state.lock().unwrap().accumulate_tx = Some(accumulate_tx);

        let this_weak = self.downgrade();
        let res = self.srcpad.start_task(move || loop {
            let Some(this) = this_weak.upgrade() else {
                break;
            };

            let timeout = match this.upstream_latency() {
                Some((true, upstream_min, _max)) => {
                    if let Some(now) = this.obj().current_running_time() {
                        let (latency, lateness) = {
                            let settings = this.settings.lock().unwrap();
                            (settings.latency, settings.lateness)
                        };

                        if let Some(next_rtime) = this
                            .state
                            .lock()
                            .unwrap()
                            .input
                            .as_ref()
                            .and_then(|input| input.start_rtime())
                        {
                            std::time::Duration::from_nanos(
                                (next_rtime + upstream_min + latency + lateness)
                                    .saturating_sub(now)
                                    .nseconds(),
                            )
                        } else {
                            std::time::Duration::MAX
                        }
                    } else {
                        std::time::Duration::MAX
                    }
                }
                _ => std::time::Duration::MAX,
            };

            gst::log!(CAT, imp = this, "now waiting with timeout {timeout:?}");

            match accumulate_rx.recv_timeout(timeout) {
                Ok(input) => {
                    gst::trace!(CAT, imp = this, "received input on translate queue");

                    match input {
                        AccumulateInput::Items(to_push) => {
                            if let Err(err) = this.do_push(to_push) {
                                if err != gst::FlowError::Flushing {
                                    gst::element_error!(
                                        this.obj(),
                                        gst::StreamError::Failed,
                                        ["Streaming failed: {}", err]
                                    );
                                }
                                let _ = this.srcpad.pause_task();
                            }
                        }
                        AccumulateInput::Gap { pts, duration } => {
                            let event = {
                                let mut state = this.state.lock().unwrap();

                                state
                                    .segment
                                    .set_position(pts + duration.unwrap_or(gst::ClockTime::ZERO));

                                gst::debug!(
                                    CAT,
                                    imp = this,
                                    "forwarding gap {} {:?}",
                                    pts,
                                    duration
                                );

                                gst::event::Gap::builder(pts)
                                    .duration(duration)
                                    .seqnum(state.seqnum)
                                    .build()
                            };

                            let _ = this.srcpad.push_event(event);
                        }
                        AccumulateInput::Event(event) => {
                            gst::debug!(CAT, imp = this, "Forwarding event {event:?}");
                            let _ = this.srcpad.push_event(event);
                        }
                        AccumulateInput::Query(mut query) => {
                            gst::debug!(CAT, imp = this, "Forwarding query {query:?}");
                            let res = this.srcpad.peer_query(unsafe { query.as_mut() });

                            this.state.lock().unwrap().serialized_query_return = Some(res);
                            this.serialized_query_cond.notify_all();
                        }
                        AccumulateInput::RecalculateTimeout => {
                            continue;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    gst::trace!(
                        CAT,
                        imp = this,
                        "timed out waiting for input on accumulate queue"
                    );

                    if let Err(err) = this.maybe_push() {
                        if err != gst::FlowError::Flushing {
                            gst::element_error!(
                                this.obj(),
                                gst::StreamError::Failed,
                                ["Streaming failed: {}", err]
                            );
                        }
                        let _ = this.srcpad.pause_task();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    gst::log!(CAT, imp = this, "accumulate queue disconnected");

                    let _ = this.srcpad.pause_task();
                    break;
                }
            }
        });

        if res.is_err() {
            return Err(gst::loggable_error!(CAT, "Failed to start pad task"));
        }

        self.state.lock().unwrap().task_started = true;

        gst::debug!(CAT, imp = self, "started source pad task");

        Ok(())
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let data = String::from_utf8(data.to_vec()).map_err(|err| {
            gst::error!(CAT, obj = pad, "Can't decode utf8: {}", err);

            gst::FlowError::Error
        })?;

        let mut state = self.state.lock().unwrap();

        if state.input.is_none() {
            state.input = Some(Input::try_new(None).map_err(|err| {
                gst::error!(CAT, obj = pad, "Failed to create segmenter: {err:?}");

                gst::FlowError::Error
            })?);
        }

        let drained_items = if buffer.flags().contains(gst::BufferFlags::DISCONT) {
            gst::log!(CAT, imp = self, "draining on discont");

            state.input.as_mut().and_then(|input| input.drain_all())
        } else {
            None
        };

        if let Some(items) = drained_items {
            if let Some(tx) = state.accumulate_tx.clone() {
                drop(state);
                let _ = tx.send(AccumulateInput::Items(items));
                state = self.state.lock().unwrap();
            }
        }

        let Some(pts) = buffer.pts() else {
            gst::warning!(CAT, imp = self, "dropping first buffer without a PTS");
            return Ok(gst::FlowSuccess::Ok);
        };

        let Some(rtime) = state.segment.to_running_time(pts) else {
            gst::log!(CAT, imp = self, "clipping buffer outside segment");
            return Ok(gst::FlowSuccess::Ok);
        };

        if let Some(input) = state.input.as_mut() {
            input.push(
                data,
                pts,
                rtime,
                buffer.duration().unwrap_or(gst::ClockTime::ZERO),
                Some(buffer.clone()),
            );
        }

        while let Some(items) = state.input.as_mut().and_then(|input| input.next_sentence()) {
            gst::log!(CAT, imp = self, "drained next sentence: {:#?}", items);
            if let Some(tx) = state.accumulate_tx.clone() {
                drop(state);
                let _ = tx.send(AccumulateInput::Items(items));
                state = self.state.lock().unwrap();
            }
        }

        // The queue was empty and has started filling up again, recalculate timeout
        if state
            .input
            .as_ref()
            .map(|input| input.items.len())
            .unwrap_or(0)
            == 1
        {
            if let Some(tx) = state.accumulate_tx.clone() {
                drop(state);
                let _ = tx.send(AccumulateInput::RecalculateTimeout);
                state = self.state.lock().unwrap();
            }
        }

        gst::trace!(CAT, imp = self, "input is now {:#?}", state.input);

        Ok(gst::FlowSuccess::Ok)
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

    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        if query.is_serialized() {
            gst::error!(CAT, obj = pad, "Handling serialized query {:?}", query);

            let accumulate_tx = self.state.lock().unwrap().accumulate_tx.clone();

            if let Some(tx) = accumulate_tx {
                let query = std::ptr::NonNull::from(query);
                let _ = tx.send(AccumulateInput::Query(query));

                let mut state = self.state.lock().unwrap();
                while state.serialized_query_return.is_none() {
                    state = self.serialized_query_cond.wait(state).unwrap();
                }

                state
                    .serialized_query_return
                    .expect("serialized query has returned")
            } else {
                true
            }
        } else {
            gst::Pad::query_default(pad, Some(&*self.obj()), query)
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Accumulate {
    const NAME: &'static str = "GstTextAccumulate";
    type Type = super::Accumulate;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Accumulate::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |imp| imp.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Accumulate::catch_panic_pad_function(
                    parent,
                    || false,
                    |imp| imp.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                Accumulate::catch_panic_pad_function(
                    parent,
                    || false,
                    |imp| imp.sink_query(pad, query),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::PadBuilder::<gst::Pad>::from_template(&templ)
            .query_function(|pad, parent, query| {
                Accumulate::catch_panic_pad_function(
                    parent,
                    || false,
                    |imp| imp.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            settings: Default::default(),
            state: Default::default(),
            serialized_query_cond: Condvar::new(),
        }
    }
}

impl ObjectImpl for Accumulate {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("latency")
                    .nick("Latency")
                    .blurb("Amount of milliseconds to allow for accumulating")
                    .default_value(DEFAULT_LATENCY.mseconds() as u32)
                    .minimum(0u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("lateness")
                    .nick("Latenness")
                    .blurb("By how many milliseconds to shift input timestamps forward for accumulating")
                    .default_value(DEFAULT_LATENESS.mseconds() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("timeout-terminators")
                    .nick("Timeout terminators")
                    .blurb("A regex for finding preferred break points when draining on timeout")
                    .default_value(DEFAULT_TIMEOUT_TERMINATORS)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("drain-on-final-transcripts")
                    .nick("Drain On Final Transcripts")
                    .blurb("whether the element should entirely drain itself when \
                        receiving rstranscribe/final-transcript events")
                    .default_value(DEFAULT_DRAIN_ON_FINAL_TRANSCRIPTS)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("drain-on-speaker-change")
                    .nick("Drain On Speaker Change")
                    .blurb("whether the element should entirely drain itself when \
                        receiving rstranscribe/speaker-change events")
                    .default_value(DEFAULT_DRAIN_ON_SPEAKER_CHANGE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("extend-duration")
                    .nick("Extend Duration")
                    .blurb("whether the element should extend the duration of an item \
                        to match the start time of the next non-drained item, if any. \
                        This is useful when a speech synthesis element is further downstream.")
                    .default_value(DEFAULT_EXTEND_DURATION)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("extended-duration-gap")
                    .nick("Extended Duration Gap")
                    .blurb("Amount of milliseconds to preserve between items when extend-duration is true")
                    .default_value(DEFAULT_EXTENDED_DURATION_GAP.mseconds() as u32)
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
            "lateness" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lateness = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "timeout-terminators" => {
                let mut settings = self.settings.lock().unwrap();
                settings.timeout_terminators =
                    value.get::<String>().expect("type checked upstream");
            }
            "drain-on-final-transcripts" => {
                let mut settings = self.settings.lock().unwrap();
                settings.drain_on_final_transcripts =
                    value.get::<bool>().expect("type checked upstream");
            }
            "drain-on-speaker-change" => {
                let mut settings = self.settings.lock().unwrap();
                settings.drain_on_speaker_change =
                    value.get::<bool>().expect("type checked upstream");
            }
            "extend-duration" => {
                let mut settings = self.settings.lock().unwrap();
                settings.extend_duration = value.get::<bool>().expect("type checked upstream");
            }
            "extended-duration-gap" => {
                let mut settings = self.settings.lock().unwrap();
                settings.extended_duration_gap = gst::ClockTime::from_mseconds(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
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
            "timeout-terminators" => {
                let settings = self.settings.lock().unwrap();
                settings.timeout_terminators.to_value()
            }
            "drain-on-final-transcripts" => {
                let settings = self.settings.lock().unwrap();
                settings.drain_on_final_transcripts.to_value()
            }
            "drain-on-speaker-change" => {
                let settings = self.settings.lock().unwrap();
                settings.drain_on_speaker_change.to_value()
            }
            "extend-duration" => {
                let settings = self.settings.lock().unwrap();
                settings.extend_duration.to_value()
            }
            "extended-duration-gap" => {
                let settings = self.settings.lock().unwrap();
                (settings.extended_duration_gap.mseconds() as u32).to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Accumulate {}

impl ElementImpl for Accumulate {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Text Accumulator",
                "Text/Filter",
                "Accumulates text",
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
            gst::StateChange::PausedToReady => {
                drop(self.state.lock().unwrap().accumulate_tx.take());
                let _ = self.srcpad.stop_task();
                *self.state.lock().unwrap() = State::default();
            }
            gst::StateChange::ReadyToPaused => {
                if let Err(err) = self.prepare() {
                    gst::element_error!(
                        self.obj(),
                        gst::StreamError::Failed,
                        ["Failed to prepare: {}", err]
                    );
                    return Err(gst::StateChangeError);
                }
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}

#[cfg(test)]
mod tests {
    use super::Input;
    use crate::textaccumulate::imp::DEFAULT_TIMEOUT_TERMINATORS;
    use regex::Regex;

    #[test]
    fn accumulator_basic() {
        let mut input = Input::try_new(None).unwrap();

        assert!(input.is_empty());
        assert_eq!(input.start_rtime(), None);
        assert!(input.drain_all().is_none());

        input.push(
            "0".into(),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        input.push(
            "2".into(),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        input.push(
            "10".into(),
            gst::ClockTime::from_nseconds(10),
            gst::ClockTime::from_nseconds(20),
            gst::ClockTime::from_nseconds(0),
            None,
        );

        assert!(!input.is_empty());
        assert_eq!(input.start_rtime(), Some(gst::ClockTime::from_nseconds(0)));

        assert!(input.next_sentence().is_none());
        assert!(input.drain_all().is_some());
    }

    #[test]
    fn accumulator_timeout() {
        let mut input = Input::try_new(None).unwrap();

        let timeout_terminators_regex = Regex::new(DEFAULT_TIMEOUT_TERMINATORS).unwrap();

        input.push(
            "0".into(),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(1),
            None,
        );
        input.push(
            "2".into(),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        let upstream_min = gst::ClockTime::from_nseconds(5);
        let lateness = gst::ClockTime::from_nseconds(0);

        assert!(input
            .timeout(
                gst::ClockTime::from_nseconds(5),
                upstream_min,
                lateness,
                &timeout_terminators_regex
            )
            .is_none());

        assert_eq!(
            input
                .timeout(
                    gst::ClockTime::from_nseconds(6),
                    upstream_min,
                    lateness,
                    &timeout_terminators_regex
                )
                .unwrap()
                .len(),
            2
        );

        assert!(input.is_empty());
    }

    #[test]
    fn accumulator_timeout_punctuation() {
        let mut input = Input::try_new(None).unwrap();

        let timeout_terminators_regex = Regex::new(DEFAULT_TIMEOUT_TERMINATORS).unwrap();

        input.push(
            "0".into(),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        input.push(
            ",".into(),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        input.push(
            "5".into(),
            gst::ClockTime::from_nseconds(5),
            gst::ClockTime::from_nseconds(5),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        let upstream_min = gst::ClockTime::from_nseconds(5);
        let lateness = gst::ClockTime::from_nseconds(0);

        assert!(input
            .timeout(
                gst::ClockTime::from_nseconds(5),
                upstream_min,
                lateness,
                &timeout_terminators_regex
            )
            .is_none());

        assert_eq!(
            input
                .timeout(
                    gst::ClockTime::from_nseconds(6),
                    upstream_min,
                    lateness,
                    &timeout_terminators_regex
                )
                .unwrap()
                .len(),
            2
        );

        assert_eq!(input.items.len(), 1);
    }

    #[test]
    fn accumulator_lateness() {
        let mut input = Input::try_new(None).unwrap();

        let timeout_terminators_regex = Regex::new(DEFAULT_TIMEOUT_TERMINATORS).unwrap();

        input.push(
            "0".into(),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(0),
            gst::ClockTime::from_nseconds(1),
            None,
        );
        input.push(
            "2".into(),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(2),
            gst::ClockTime::from_nseconds(1),
            None,
        );

        let upstream_min = gst::ClockTime::from_nseconds(5);
        let lateness = gst::ClockTime::from_nseconds(10);

        assert!(input
            .timeout(
                gst::ClockTime::from_nseconds(5),
                upstream_min,
                lateness,
                &timeout_terminators_regex
            )
            .is_none());

        assert_eq!(
            input
                .timeout(
                    gst::ClockTime::from_nseconds(16),
                    upstream_min,
                    lateness,
                    &timeout_terminators_regex
                )
                .unwrap()
                .len(),
            2
        );

        assert!(input.is_empty());
    }

    #[test]
    fn input_basic() {
        let mut input = Input::try_new(None).unwrap();

        let mut pts_seconds = 0u64;

        for kanji in "私は幸せです。あなたはそうではありません。 ".chars() {
            input.push(
                kanji.into(),
                gst::ClockTime::from_seconds(pts_seconds),
                gst::ClockTime::from_seconds(pts_seconds),
                gst::ClockTime::from_seconds(1),
                None,
            );

            if let Some(next_sentence) = input.next_sentence() {
                eprintln!("Next sentence: {next_sentence:?}");
            }

            pts_seconds += 1;
        }

        pts_seconds = 0;

        for item in ["Hello", "world", ".", "I", "am", "happy", ",", "are"] {
            input.push(
                item.into(),
                gst::ClockTime::from_seconds(pts_seconds),
                gst::ClockTime::from_seconds(pts_seconds),
                gst::ClockTime::from_seconds(1),
                None,
            );

            if let Some(next_sentence) = input.next_sentence() {
                eprintln!("Next sentence: {next_sentence:?}");
            }

            pts_seconds += 1;
        }

        /*
        let drained = input.drain_to_next_terminator();
        eprintln!("drained: {:?}", drained);
        */
    }
}
