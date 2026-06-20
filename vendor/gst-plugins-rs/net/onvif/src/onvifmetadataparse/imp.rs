// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex};

fn utc_time_to_pts(
    segment: &gst::FormattedSegment<gst::ClockTime>,
    utc_time_running_time_mapping: (gst::ClockTime, gst::Signed<gst::ClockTime>),
    utc_time: gst::ClockTime,
) -> Option<gst::ClockTime> {
    let running_time =
        utc_time_to_running_time(utc_time_running_time_mapping, utc_time)?.positive()?;
    segment.position_from_running_time(running_time)
}

fn utc_time_to_running_time(
    utc_time_running_time_mapping: (gst::ClockTime, gst::Signed<gst::ClockTime>),
    utc_time: gst::ClockTime,
) -> Option<gst::Signed<gst::ClockTime>> {
    if utc_time < utc_time_running_time_mapping.0 {
        let diff = utc_time_running_time_mapping.0 - utc_time;
        utc_time_running_time_mapping.1.checked_sub_unsigned(diff)
    } else {
        let diff = utc_time - utc_time_running_time_mapping.0;
        utc_time_running_time_mapping.1.checked_add_unsigned(diff)
    }
}

fn running_time_to_utc_time(
    utc_time_running_time_mapping: (gst::ClockTime, gst::Signed<gst::ClockTime>),
    running_time: gst::Signed<gst::ClockTime>,
) -> Option<gst::ClockTime> {
    let diff = running_time.checked_sub(utc_time_running_time_mapping.1)?;

    use gst::Signed::*;
    match diff {
        Positive(diff) => utc_time_running_time_mapping.0.checked_add(diff),
        Negative(diff) => utc_time_running_time_mapping.0.checked_sub(diff),
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "onvifmetadataparse",
        gst::DebugColorFlags::empty(),
        Some("ONVIF Metadata Parser Element"),
    )
});

#[derive(Clone, Debug)]
struct Settings {
    latency: Option<gst::ClockTime>,
    max_lateness: Option<gst::ClockTime>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            latency: None,
            max_lateness: Some(200.mseconds()),
        }
    }
}

#[derive(Debug)]
struct Frame {
    video_analytics: xmltree::Element,
    other_elements: Vec<xmltree::Element>,
    events: Vec<gst::Event>,
}

impl Default for Frame {
    fn default() -> Self {
        let mut video_analytics = xmltree::Element::new("VideoAnalytics");
        video_analytics.namespace = Some(String::from(crate::ONVIF_METADATA_SCHEMA));
        video_analytics.prefix = Some(String::from("tt"));

        Frame {
            video_analytics,
            other_elements: Vec::new(),
            events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum TimedBufferOrEvent {
    Buffer(gst::Signed<gst::ClockTime>, gst::Buffer),
    Event(gst::Signed<gst::ClockTime>, gst::Event),
}

#[derive(Debug, Clone)]
enum BufferOrEvent {
    Buffer(gst::Buffer),
    Event(gst::Event),
}

#[derive(Debug)]
struct State {
    /// Initially queued buffers and serialized events until we have a UTC time / running time mapping.
    pre_queued_buffers: Vec<TimedBufferOrEvent>,
    /// Mapping of UTC time to running time.
    utc_time_running_time_mapping: Option<(gst::ClockTime, gst::Signed<gst::ClockTime>)>,
    /// UTC time -> XML data and serialized events.
    queued_frames: BTreeMap<gst::ClockTime, Frame>,

    /// Currently configured input segment.
    in_segment: gst::FormattedSegment<gst::ClockTime>,
    /// Currently configured output segment.
    out_segment: gst::FormattedSegment<gst::ClockTime>,

    /// Upstream latency and live'ness.
    upstream_latency: Option<(bool, gst::ClockTime)>,
    /// Configured latency on this element.
    configured_latency: gst::ClockTime,
    /// Last flow return of the source pad.
    last_flow_ret: Result<gst::FlowSuccess, gst::FlowError>,
    /// Clock wait of the source pad task.
    /// Otherwise the source pad task waits on the condition variable.
    clock_wait: Option<gst::SingleShotClockId>,
}

impl Default for State {
    fn default() -> Self {
        let mut segment = gst::FormattedSegment::default();
        segment.set_position(gst::ClockTime::NONE);

        State {
            pre_queued_buffers: Vec::new(),
            utc_time_running_time_mapping: None,
            queued_frames: BTreeMap::new(),

            in_segment: segment.clone(),
            out_segment: segment.clone(),

            upstream_latency: None,
            configured_latency: gst::ClockTime::ZERO,
            last_flow_ret: Err(gst::FlowError::Flushing),
            clock_wait: None,
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(clock_wait) = self.clock_wait.take() {
            clock_wait.unschedule();
        }
    }
}

pub struct OnvifMetadataParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    cond: Condvar,
}

impl OnvifMetadataParse {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(
            CAT,
            obj = pad,
            "Handling buffer {:?} with UTC time {}",
            buffer,
            crate::lookup_reference_timestamp(&buffer).display()
        );

        let mut state = self.state.lock().unwrap();

        let pts = match buffer.pts() {
            Some(pts) => pts,
            None => {
                gst::error!(CAT, obj = pad, "Need buffers with PTS");
                return Err(gst::FlowError::Error);
            }
        };

        let running_time = state.in_segment.to_running_time_full(pts).unwrap();

        if state
            .in_segment
            .position()
            .is_none_or(|position| position < pts)
        {
            gst::trace!(CAT, imp = self, "Input position updated to {}", pts);
            state.in_segment.set_position(pts);
        }

        // First we need to get an UTC/running time mapping. We wait up to the latency
        // for that and otherwise error out.
        if state.utc_time_running_time_mapping.is_none() {
            let utc_time = crate::lookup_reference_timestamp(&buffer);
            if let Some(utc_time) = utc_time {
                let (initial_utc_time, initial_running_time) = loop {
                    let (idx, initial_running_time) = state
                        .pre_queued_buffers
                        .iter()
                        .enumerate()
                        .find_map(|(idx, o)| {
                            if let TimedBufferOrEvent::Buffer(running_time, _) = o {
                                Some((Some(idx), *running_time))
                            } else {
                                None
                            }
                        })
                        .unwrap_or((None, running_time));

                    let diff = match running_time.checked_sub(initial_running_time) {
                        Some(diff) => diff,
                        None => {
                            gst::error!(
                                CAT,
                                obj = pad,
                                "Too big running time difference between initial running time {:?} and current running time {:?}",
                                initial_running_time,
                                running_time,
                            );
                            return Err(gst::FlowError::Error);
                        }
                    };

                    use gst::Signed::*;
                    let initial_utc_time = match utc_time.into_positive().checked_sub(diff) {
                        Some(Positive(initial_utc_time)) => initial_utc_time,
                        Some(Negative(initial_utc_time)) => {
                            gst::warning!(
                                CAT,
                                obj = pad,
                                "Initial UTC time is negative: -{}, dropping buffer",
                                initial_utc_time
                            );

                            state.pre_queued_buffers.remove(idx.unwrap());
                            continue;
                        }
                        None => {
                            gst::warning!(
                                CAT,
                                obj = pad,
                                "Can't calculate initial UTC time, dropping buffer"
                            );
                            state.pre_queued_buffers.remove(idx.unwrap());
                            continue;
                        }
                    };

                    break (initial_utc_time, initial_running_time);
                };

                gst::info!(
                    CAT,
                    obj = pad,
                    "Calculated initial UTC/running time mapping: {}/{:?}",
                    initial_utc_time,
                    initial_running_time
                );
                state.utc_time_running_time_mapping =
                    Some((initial_utc_time, initial_running_time));
            } else {
                state
                    .pre_queued_buffers
                    .push(TimedBufferOrEvent::Buffer(running_time, buffer));

                if let Some((idx, front_running_time)) = state
                    .pre_queued_buffers
                    .iter()
                    .enumerate()
                    .find_map(|(idx, o)| {
                        if let TimedBufferOrEvent::Buffer(running_time, _) = o {
                            Some((idx, *running_time))
                        } else {
                            None
                        }
                    })
                {
                    if running_time.saturating_sub(front_running_time) >= state.configured_latency {
                        gst::warning!(
                            CAT,
                            obj = pad,
                            "Received no UTC time in the first {}",
                            state.configured_latency
                        );
                        state.pre_queued_buffers.remove(idx);
                    }
                }

                return Ok(gst::FlowSuccess::Ok);
            }
        }

        assert!(state.utc_time_running_time_mapping.is_some());
        self.queue(&mut state, buffer, running_time)?;
        let res = self.wake_up_output(state);

        gst::trace!(CAT, obj = pad, "Returning {:?}", res);

        res
    }

    fn queue(
        &self,
        state: &mut State,
        buffer: gst::Buffer,
        running_time: gst::Signed<gst::ClockTime>,
    ) -> Result<(), gst::FlowError> {
        let State {
            ref mut pre_queued_buffers,
            ref mut queued_frames,
            ref utc_time_running_time_mapping,
            ..
        } = &mut *state;

        let utc_time_running_time_mapping = utc_time_running_time_mapping.unwrap();

        for buffer_or_event in
            pre_queued_buffers
                .drain(..)
                .chain(std::iter::once(TimedBufferOrEvent::Buffer(
                    running_time,
                    buffer,
                )))
        {
            let (running_time, buffer) = match buffer_or_event {
                TimedBufferOrEvent::Event(running_time, event) => {
                    let current_utc_time =
                        running_time_to_utc_time(utc_time_running_time_mapping, running_time)
                            .unwrap_or(gst::ClockTime::ZERO);

                    let frame = queued_frames
                        .entry(current_utc_time)
                        .or_insert_with(Frame::default);
                    frame.events.push(event);

                    continue;
                }
                TimedBufferOrEvent::Buffer(running_time, buffer) => (running_time, buffer),
            };

            let root = crate::xml_from_buffer(&buffer).map_err(|err| {
                self.post_error_message(err);

                gst::FlowError::Error
            })?;

            for res in crate::iterate_video_analytics_frames(&root) {
                let (dt, el) = res.map_err(|err| {
                    self.post_error_message(err);

                    gst::FlowError::Error
                })?;

                let dt_unix_ns = dt
                    .timestamp_nanos_opt()
                    .and_then(|ns| u64::try_from(ns).ok())
                    .and_then(|ns| ns.nseconds().checked_add(crate::PRIME_EPOCH_OFFSET));

                let Some(dt_unix_ns) = dt_unix_ns else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Frame with unrepresentable UTC time {}",
                        dt,
                    );
                    continue;
                };

                gst::trace!(
                    CAT,
                    imp = self,
                    "Queueing frame with UTC time {}",
                    dt_unix_ns
                );

                let frame = queued_frames
                    .entry(dt_unix_ns)
                    .or_insert_with(Frame::default);

                frame
                    .video_analytics
                    .children
                    .push(xmltree::XMLNode::Element(el.clone()));
            }

            let utc_time = running_time_to_utc_time(utc_time_running_time_mapping, running_time)
                .unwrap_or(gst::ClockTime::ZERO);

            for child in root.children.iter().filter_map(|n| n.as_element()) {
                let frame = queued_frames.entry(utc_time).or_insert_with(Frame::default);

                if child.name == "VideoAnalytics"
                    && child.namespace.as_deref() == Some(crate::ONVIF_METADATA_SCHEMA)
                {
                    for subchild in child.children.iter().filter_map(|n| n.as_element()) {
                        if subchild.name == "Frame"
                            && subchild.namespace.as_deref() == Some(crate::ONVIF_METADATA_SCHEMA)
                        {
                            continue;
                        }

                        frame
                            .video_analytics
                            .children
                            .push(xmltree::XMLNode::Element(subchild.clone()));
                    }
                } else {
                    frame.other_elements.push(child.clone());
                }
            }
        }

        Ok(())
    }

    fn wake_up_output<'a>(
        &'a self,
        mut state: std::sync::MutexGuard<'a, State>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if state.upstream_latency.is_none() {
            drop(state);

            gst::debug!(CAT, imp = self, "Have no upstream latency yet, querying");
            let mut q = gst::query::Latency::new();
            let res = self.sinkpad.peer_query(&mut q);

            state = self.state.lock().unwrap();

            if res {
                let (live, min, max) = q.result();

                gst::debug!(
                    CAT,
                    imp = self,
                    "Latency query response: live {} min {} max {}",
                    live,
                    min,
                    max.display()
                );

                state.upstream_latency = Some((live, min));
            } else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Can't query upstream latency -- assuming non-live upstream for now"
                );
            }
        }

        // Consider waking up the source element thread
        if self.sinkpad.pad_flags().contains(gst::PadFlags::EOS) {
            gst::trace!(CAT, imp = self, "Scheduling immediate wakeup at EOS",);

            if let Some(clock_wait) = state.clock_wait.take() {
                clock_wait.unschedule();
            }
            self.cond.notify_all();
        } else if self.reschedule_clock_wait(&mut state) {
            self.cond.notify_all();
        } else {
            // Not live or have no clock

            // Wake up if between now and the earliest frame's running time more than the
            // configured latency has passed.
            let queued_time = self.calculate_queued_time(&state);

            if queued_time.is_some_and(|queued_time| queued_time >= state.configured_latency) {
                gst::trace!(
                    CAT,
                    imp = self,
                    "Scheduling immediate wakeup -- queued time {}",
                    queued_time.display()
                );

                if let Some(clock_wait) = state.clock_wait.take() {
                    clock_wait.unschedule();
                }
                self.cond.notify_all();
            }
        }

        state.last_flow_ret
    }

    fn calculate_queued_time(&self, state: &State) -> Option<gst::ClockTime> {
        let earliest_utc_time = match state.queued_frames.iter().next() {
            Some((&earliest_utc_time, _earliest_frame)) => earliest_utc_time,
            None => return None,
        };

        let earliest_running_time = utc_time_to_running_time(
            state.utc_time_running_time_mapping.unwrap(),
            earliest_utc_time,
        );

        let current_running_time = state
            .in_segment
            .to_running_time_full(state.in_segment.position());

        let queued_time = Option::zip(current_running_time, earliest_running_time)
            .and_then(|(current_running_time, earliest_running_time)| {
                current_running_time.checked_sub(earliest_running_time)
            })
            .and_then(|queued_time| queued_time.positive())
            .unwrap_or(gst::ClockTime::ZERO);

        gst::trace!(
            CAT,
            imp = self,
            "Currently queued {}",
            queued_time.display()
        );

        Some(queued_time)
    }

    fn reschedule_clock_wait(&self, state: &mut State) -> bool {
        let earliest_utc_time = match state.queued_frames.iter().next() {
            Some((&earliest_utc_time, _earliest_frame)) => earliest_utc_time,
            None => return false,
        };

        let min_latency = match state.upstream_latency {
            Some((true, min_latency)) => min_latency,
            _ => return false,
        };

        let earliest_running_time = utc_time_to_running_time(
            state.utc_time_running_time_mapping.unwrap(),
            earliest_utc_time,
        );

        let (clock, base_time) = match (self.obj().clock(), self.obj().base_time()) {
            (Some(clock), Some(base_time)) => (clock, base_time),
            _ => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Upstream is live but have no clock -- assuming non-live for now"
                );
                return false;
            }
        };

        // Update clock wait to the clock time of the earliest metadata to output plus
        // the configured latency
        if let Some(earliest_clock_time) = earliest_running_time
            .and_then(|earliest_running_time| {
                earliest_running_time
                    .checked_add_unsigned(base_time + min_latency + state.configured_latency)
            })
            .and_then(|earliest_clock_time| earliest_clock_time.positive())
        {
            if state
                .clock_wait
                .as_ref()
                .is_none_or(|clock_wait| clock_wait.time() != earliest_clock_time)
            {
                if let Some(clock_wait) = state.clock_wait.take() {
                    clock_wait.unschedule();
                }
                gst::trace!(
                    CAT,
                    imp = self,
                    "Scheduling timer for {} / running time {}, now {}",
                    earliest_clock_time,
                    earliest_running_time.unwrap().display(),
                    clock.time().display(),
                );
                state.clock_wait = Some(clock.new_single_shot_id(earliest_clock_time));
            }
        } else {
            // Wake up immediately if the metadata is before the segment
            if let Some(clock_wait) = state.clock_wait.take() {
                clock_wait.unschedule();
            }
            gst::trace!(CAT, imp = self, "Scheduling immediate wakeup");
        }

        true
    }

    fn drain(
        &self,
        state: &mut State,
        drain_utc_time: Option<gst::ClockTime>,
    ) -> Result<Vec<BufferOrEvent>, gst::FlowError> {
        let State {
            ref mut queued_frames,
            ref mut out_segment,
            utc_time_running_time_mapping,
            ..
        } = &mut *state;

        let utc_time_running_time_mapping = match utc_time_running_time_mapping {
            Some(utc_time_running_time_mapping) => *utc_time_running_time_mapping,
            None => return Ok(vec![]),
        };

        gst::log!(
            CAT,
            imp = self,
            "Draining up to UTC time {} / running time {} from current position {} / running time {}",
            drain_utc_time.display(),
            drain_utc_time
                .and_then(|drain_utc_time| utc_time_to_running_time(
                    utc_time_running_time_mapping,
                    drain_utc_time
                ))
                .and_then(|running_time| running_time.positive())
                .display(),
            out_segment.position().display(),
            out_segment
                .to_running_time(out_segment.position())
                .display(),
        );

        let mut data = Vec::new();

        while !queued_frames.is_empty() {
            let utc_time = *queued_frames.iter().next().unwrap().0;

            // Check if this frame should still be drained
            if drain_utc_time.is_some_and(|drain_utc_time| drain_utc_time < utc_time) {
                break;
            }

            // FIXME: Use pop_first() once stabilized
            let mut frame = queued_frames.remove(&utc_time).unwrap();

            let had_events = !frame.events.is_empty();
            let mut eos_events = vec![];

            for event in frame.events.drain(..) {
                match event.view() {
                    gst::EventView::Segment(ev) => {
                        let mut segment = ev
                            .segment()
                            .downcast_ref::<gst::ClockTime>()
                            .unwrap()
                            .clone();
                        let current_position = out_segment
                            .position()
                            .and_then(|position| out_segment.to_running_time(position))
                            .and_then(|running_time| {
                                segment.position_from_running_time(running_time)
                            });
                        segment.set_position(current_position);

                        gst::debug!(CAT, imp = self, "Configuring output segment {:?}", segment);

                        *out_segment = segment;

                        data.push(BufferOrEvent::Event(event));
                    }
                    gst::EventView::Caps(ev) => {
                        data.push(BufferOrEvent::Event(
                            gst::event::Caps::builder(&self.srcpad.pad_template_caps())
                                .seqnum(ev.seqnum())
                                .build(),
                        ));
                    }
                    gst::EventView::Gap(ev) => {
                        let (current_position, _duration) = ev.get();

                        if out_segment
                            .position()
                            .is_none_or(|position| position < current_position)
                        {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "Output position updated to {}",
                                current_position
                            );
                            out_segment.set_position(current_position);
                        }

                        data.push(BufferOrEvent::Event(event));
                    }
                    gst::EventView::Eos(_) => {
                        // Forward EOS events *after* any frame data
                        eos_events.push(event);
                    }
                    _ => {
                        data.push(BufferOrEvent::Event(event));
                    }
                }
            }

            let mut frame_pts =
                match utc_time_to_pts(out_segment, utc_time_running_time_mapping, utc_time) {
                    Some(frame_pts) => frame_pts,
                    None => {
                        gst::warning!(CAT, imp = self, "UTC time {} outside segment", utc_time);
                        gst::ClockTime::ZERO
                    }
                };

            if frame.video_analytics.children.is_empty() && frame.other_elements.is_empty() {
                // Generate a gap event if there's no actual data for this time
                if !had_events {
                    data.push(BufferOrEvent::Event(
                        gst::event::Gap::builder(frame_pts).build(),
                    ));
                }

                for event in eos_events {
                    data.push(BufferOrEvent::Event(event));
                }

                continue;
            }

            if let Some(position) = out_segment.position() {
                let settings = self.settings.lock().unwrap().clone();
                let diff = position.saturating_sub(frame_pts);
                if settings
                    .max_lateness
                    .is_some_and(|max_lateness| diff > max_lateness)
                {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Dropping frame with UTC time {} / PTS {} that is too late by {} at current position {}",
                        utc_time,
                        frame_pts,
                        diff,
                        position,
                    );

                    for event in eos_events {
                        data.push(BufferOrEvent::Event(event));
                    }

                    continue;
                } else if diff > gst::ClockTime::ZERO {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Frame in the past by {} with UTC time {} / PTS {} at current position {}",
                        diff,
                        utc_time,
                        frame_pts,
                        position,
                    );

                    frame_pts = position;
                }
            }

            if out_segment
                .position()
                .is_none_or(|position| position < frame_pts)
            {
                gst::trace!(CAT, imp = self, "Output position updated to {}", frame_pts);
                out_segment.set_position(frame_pts);
            }

            let Frame {
                video_analytics,
                other_elements,
                ..
            } = frame;

            gst::trace!(
                CAT,
                imp = self,
                "Producing frame with UTC time {} / PTS {}",
                utc_time,
                frame_pts
            );

            let mut xml = xmltree::Element::new("MetadataStream");
            xml.namespaces
                .get_or_insert_with(|| xmltree::Namespace(Default::default()))
                .put("tt", crate::ONVIF_METADATA_SCHEMA);
            xml.namespace = Some(String::from(crate::ONVIF_METADATA_SCHEMA));
            xml.prefix = Some(String::from("tt"));

            if !video_analytics.children.is_empty() {
                xml.children
                    .push(xmltree::XMLNode::Element(video_analytics));
            }
            for child in other_elements {
                xml.children.push(xmltree::XMLNode::Element(child));
            }

            let mut vec = Vec::new();
            if let Err(err) = xml.write_with_config(
                &mut vec,
                xmltree::EmitterConfig {
                    write_document_declaration: false,
                    perform_indent: true,
                    ..xmltree::EmitterConfig::default()
                },
            ) {
                gst::error!(CAT, imp = self, "Can't serialize XML element: {}", err);
                for event in eos_events {
                    data.push(BufferOrEvent::Event(event));
                }
                continue;
            }

            let mut buffer = gst::Buffer::from_mut_slice(vec);
            let buffer_ref = buffer.get_mut().unwrap();
            buffer_ref.set_pts(frame_pts);

            gst::ReferenceTimestampMeta::add(
                buffer_ref,
                &crate::NTP_CAPS,
                utc_time,
                gst::ClockTime::NONE,
            );

            data.push(BufferOrEvent::Buffer(buffer));

            for event in eos_events {
                data.push(BufferOrEvent::Event(event));
            }
        }

        gst::trace!(
            CAT,
            imp = self,
            "Position after draining {} / running time {} -- queued now {} / {} items",
            out_segment.position().display(),
            out_segment
                .to_running_time(out_segment.position())
                .display(),
            self.calculate_queued_time(state).display(),
            state.queued_frames.len(),
        );

        Ok(data)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            gst::EventView::FlushStart(_) => {
                let mut state = self.state.lock().unwrap();
                state.last_flow_ret = Err(gst::FlowError::Flushing);
                if let Some(ref clock_wait) = state.clock_wait.take() {
                    clock_wait.unschedule();
                }
                drop(state);
                self.cond.notify_all();

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::FlushStop(_) => {
                let _ = self.srcpad.stop_task();
                let mut state = self.state.lock().unwrap();
                state.pre_queued_buffers.clear();
                state.queued_frames.clear();
                state.utc_time_running_time_mapping = None;
                state.in_segment.reset();
                state.in_segment.set_position(gst::ClockTime::NONE);
                state.out_segment.reset();
                state.out_segment.set_position(gst::ClockTime::NONE);
                state.last_flow_ret = Ok(gst::FlowSuccess::Ok);
                drop(state);
                let mut res = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                if res {
                    res = self.src_start_task().is_ok();
                }
                res
            }
            ev if event.is_serialized() => {
                let mut state = self.state.lock().unwrap();

                match ev {
                    gst::EventView::StreamStart(_) => {
                        // Start task again if needed in case we previously went EOS and paused the
                        // task because of that.
                        let _ = self.src_start_task();
                    }
                    gst::EventView::Segment(ev) => {
                        match ev.segment().downcast_ref::<gst::ClockTime>().cloned() {
                            Some(mut segment) => {
                                let current_position = state
                                    .in_segment
                                    .position()
                                    .and_then(|position| state.in_segment.to_running_time(position))
                                    .and_then(|running_time| {
                                        segment.position_from_running_time(running_time)
                                    });
                                segment.set_position(current_position);

                                gst::debug!(
                                    CAT,
                                    obj = pad,
                                    "Configuring input segment {:?}",
                                    segment
                                );
                                state.in_segment = segment;
                            }
                            None => {
                                gst::error!(CAT, obj = pad, "Non-TIME segment");
                                return false;
                            }
                        }
                    }
                    gst::EventView::Caps(ev) => {
                        let settings = self.settings.lock().unwrap().clone();

                        let previous_latency = state.configured_latency;
                        let latency = if let Some(latency) = settings.latency {
                            latency
                        } else {
                            let caps = ev.caps();
                            let s = caps.structure(0).unwrap();
                            let parsed = Some(true) == s.get("parsed").ok();

                            if parsed {
                                gst::ClockTime::ZERO
                            } else {
                                6.seconds()
                            }
                        };
                        state.configured_latency = latency;
                        drop(state);

                        gst::debug!(CAT, obj = pad, "Configuring latency of {}", latency);
                        if previous_latency != latency {
                            let element = self.obj();
                            let _ = element.post_message(
                                gst::message::Latency::builder().src(&*element).build(),
                            );
                        }

                        state = self.state.lock().unwrap();
                    }
                    gst::EventView::Gap(ev) => {
                        let (mut current_position, duration) = ev.get();
                        if let Some(duration) = duration {
                            current_position += duration;
                        }

                        if state
                            .in_segment
                            .position()
                            .is_none_or(|position| position < current_position)
                        {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "Input position updated to {}",
                                current_position
                            );
                            state.in_segment.set_position(current_position);
                        }
                    }
                    _ => (),
                }

                let State {
                    utc_time_running_time_mapping,
                    ref in_segment,
                    ref mut queued_frames,
                    ref mut pre_queued_buffers,
                    ..
                } = &mut *state;

                if let Some(utc_time_running_time_mapping) = utc_time_running_time_mapping {
                    let frame = if matches!(ev, gst::EventView::Eos(_)) && !queued_frames.is_empty()
                    {
                        // FIXME: Use last_entry() once stabilized
                        let (&eos_utc_time, frame) = queued_frames.iter_mut().last().unwrap();

                        gst::trace!(
                            CAT,
                            imp = self,
                            "Queueing EOS event with UTC time {} / running time {}",
                            eos_utc_time,
                            utc_time_to_running_time(*utc_time_running_time_mapping, eos_utc_time)
                                .display(),
                        );

                        frame
                    } else {
                        let current_running_time = in_segment
                            .to_running_time_full(in_segment.position())
                            .unwrap_or(gst::ClockTime::MIN_SIGNED);
                        let current_utc_time = running_time_to_utc_time(
                            *utc_time_running_time_mapping,
                            current_running_time,
                        )
                        .unwrap_or(gst::ClockTime::ZERO);

                        gst::trace!(
                            CAT,
                            imp = self,
                            "Queueing event with UTC time {} / running time {}",
                            current_utc_time,
                            current_running_time.display(),
                        );

                        queued_frames
                            .entry(current_utc_time)
                            .or_insert_with(Frame::default)
                    };

                    frame.events.push(event);

                    self.wake_up_output(state).is_ok()
                } else {
                    if matches!(ev, gst::EventView::Eos(_)) {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Got EOS event before creating UTC/running time mapping"
                        );
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Failed,
                            ["Got EOS event before creating UTC/running time mapping"]
                        );
                        return gst::Pad::event_default(pad, Some(&*self.obj()), event);
                    }

                    let current_running_time = in_segment
                        .to_running_time_full(in_segment.position())
                        .unwrap_or(gst::ClockTime::MIN_SIGNED);

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Pre-queueing event with running time {}",
                        current_running_time.display()
                    );

                    pre_queued_buffers.push(TimedBufferOrEvent::Event(current_running_time, event));
                    true
                }
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            gst::QueryViewMut::Caps(q) => {
                let caps = pad.pad_template_caps();
                let res = if let Some(filter) = q.filter() {
                    filter.intersect_with_mode(&caps, gst::CapsIntersectMode::First)
                } else {
                    caps
                };

                q.set_result(&res);

                true
            }
            gst::QueryViewMut::AcceptCaps(q) => {
                let caps = q.caps();
                let res = caps.can_intersect(&pad.pad_template_caps());
                q.set_result(res);

                true
            }
            gst::QueryViewMut::Allocation(_) => {
                gst::fixme!(CAT, obj = pad, "Dropping allocation query");
                false
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            gst::EventView::FlushStart(_) => {
                let mut state = self.state.lock().unwrap();
                state.last_flow_ret = Err(gst::FlowError::Flushing);
                if let Some(ref clock_wait) = state.clock_wait.take() {
                    clock_wait.unschedule();
                }
                drop(state);
                self.cond.notify_all();

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::FlushStop(_) => {
                let _ = self.srcpad.stop_task();
                let mut state = self.state.lock().unwrap();
                state.pre_queued_buffers.clear();
                state.queued_frames.clear();
                state.utc_time_running_time_mapping = None;
                state.in_segment.reset();
                state.in_segment.set_position(gst::ClockTime::NONE);
                state.out_segment.reset();
                state.out_segment.set_position(gst::ClockTime::NONE);
                state.last_flow_ret = Ok(gst::FlowSuccess::Ok);
                drop(state);
                let mut res = gst::Pad::event_default(pad, Some(&*self.obj()), event);
                if res {
                    res = self.src_start_task().is_ok();
                }
                res
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            gst::QueryViewMut::Caps(q) => {
                let caps = pad.pad_template_caps();
                let res = if let Some(filter) = q.filter() {
                    filter.intersect_with_mode(&caps, gst::CapsIntersectMode::First)
                } else {
                    caps
                };

                q.set_result(&res);

                true
            }
            gst::QueryViewMut::AcceptCaps(q) => {
                let caps = q.caps();
                let res = caps.can_intersect(&pad.pad_template_caps());
                q.set_result(res);

                true
            }
            gst::QueryViewMut::Latency(q) => {
                let mut upstream_query = gst::query::Latency::new();
                let ret = self.sinkpad.peer_query(&mut upstream_query);

                if ret {
                    let (live, mut min, mut max) = upstream_query.result();

                    let mut state = self.state.lock().unwrap();
                    state.upstream_latency = Some((live, min));
                    min += state.configured_latency;
                    max = max.map(|max| max + state.configured_latency);

                    q.set(live, min, max);

                    gst::debug!(
                        CAT,
                        obj = pad,
                        "Latency query response: live {} min {} max {}",
                        live,
                        min,
                        max.display()
                    );

                    let _ = self.wake_up_output(state);
                }

                ret
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }

    fn src_start_task(&self) -> Result<(), gst::LoggableError> {
        let self_ = self.ref_counted();
        self.srcpad
            .start_task(move || {
                if let Err(err) = self_.src_loop() {
                    match err {
                        gst::FlowError::Flushing => {
                            gst::debug!(CAT, imp = self_, "Pausing after flow {:?}", err);
                        }
                        gst::FlowError::Eos => {
                            let _ = self_.srcpad.push_event(gst::event::Eos::builder().build());

                            gst::debug!(CAT, imp = self_, "Pausing after flow {:?}", err);
                        }
                        _ => {
                            let _ = self_.srcpad.push_event(gst::event::Eos::builder().build());

                            gst::error!(CAT, imp = self_, "Pausing after flow {:?}", err);

                            gst::element_imp_error!(
                                self_,
                                gst::StreamError::Failed,
                                ["Streaming stopped, reason: {:?}", err]
                            );
                        }
                    }

                    let _ = self_.srcpad.pause_task();
                }
            })
            .map_err(|err| gst::loggable_error!(CAT, "Failed to start pad task: {}", err))
    }

    fn src_activatemode(
        pad: &gst::Pad,
        mode: gst::PadMode,
        activate: bool,
    ) -> Result<(), gst::LoggableError> {
        if mode == gst::PadMode::Pull || activate && mode == gst::PadMode::None {
            return Err(gst::loggable_error!(CAT, "Invalid activation mode"));
        }

        if activate {
            let element = pad
                .parent()
                .map(|p| p.downcast::<super::OnvifMetadataParse>().unwrap())
                .ok_or_else(|| {
                    gst::loggable_error!(CAT, "Failed to start pad task: pad has no parent")
                })?;

            let self_ = element.imp();
            let mut state = self_.state.lock().unwrap();
            state.last_flow_ret = Ok(gst::FlowSuccess::Ok);
            drop(state);

            self_.src_start_task()?;
        } else {
            let element = pad
                .parent()
                .map(|p| p.downcast::<super::OnvifMetadataParse>().unwrap())
                .ok_or_else(|| {
                    gst::loggable_error!(CAT, "Failed to stop pad task: pad has no parent")
                })?;

            let self_ = element.imp();
            let mut state = self_.state.lock().unwrap();
            state.last_flow_ret = Err(gst::FlowError::Flushing);
            if let Some(ref clock_wait) = state.clock_wait.take() {
                clock_wait.unschedule();
            }
            drop(state);
            self_.cond.notify_all();

            pad.stop_task()
                .map_err(|err| gst::loggable_error!(CAT, "Failed to stop pad task: {}", err))?;
        }

        Ok(())
    }

    fn src_loop(&self) -> Result<(), gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        // Remember last clock wait time in case we got woken up slightly earlier than the timer
        // and a bit more than up to the current clock time should be drained.
        let mut last_clock_wait_time: Option<gst::ClockTime> = None;

        loop {
            // If flushing or any other error then just return here
            state.last_flow_ret?;

            // Calculate running time until which to drain now
            let mut drain_running_time = None;
            if self.sinkpad.pad_flags().contains(gst::PadFlags::EOS) {
                // Drain completely
                gst::debug!(CAT, imp = self, "Sink pad is EOS, draining");
            } else if let Some((true, min_latency)) = state.upstream_latency {
                // Drain until the current clock running time minus the configured latency when
                // live
                if let Some((now, base_time)) = Option::zip(
                    self.obj().clock().as_ref().map(gst::Clock::time),
                    self.obj().base_time(),
                ) {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Clock time now {}, last timer was at {} and current timer at {}",
                        now,
                        last_clock_wait_time.display(),
                        state
                            .clock_wait
                            .as_ref()
                            .map(|clock_wait| clock_wait.time())
                            .display(),
                    );

                    let now =
                        (now - base_time).saturating_sub(min_latency + state.configured_latency);
                    let now = if let Some(last_clock_wait_time) = last_clock_wait_time {
                        let last_clock_wait_time = (last_clock_wait_time - base_time)
                            .saturating_sub(min_latency + state.configured_latency);
                        std::cmp::max(now, last_clock_wait_time)
                    } else {
                        now
                    };

                    drain_running_time = Some(now.into_positive());
                }
            } else {
                // Otherwise if not live drain up to the last input running time minus the
                // configured latency, i.e. keep only up to the configured latency queued
                let current_running_time = state
                    .in_segment
                    .to_running_time_full(state.in_segment.position());
                drain_running_time = current_running_time.and_then(|current_running_time| {
                    current_running_time.checked_sub_unsigned(state.configured_latency)
                });
            }

            // And drain up to that running time now, or everything if EOS
            let data = if self.sinkpad.pad_flags().contains(gst::PadFlags::EOS) {
                self.drain(&mut state, None)?
            } else if let Some(drain_utc_time) =
                Option::zip(drain_running_time, state.utc_time_running_time_mapping).and_then(
                    |(drain_running_time, utc_time_running_time_mapping)| {
                        running_time_to_utc_time(utc_time_running_time_mapping, drain_running_time)
                    },
                )
            {
                self.drain(&mut state, Some(drain_utc_time))?
            } else {
                vec![]
            };

            // If there's nothing to drain currently then wait
            last_clock_wait_time = None;
            if data.is_empty() {
                if self.sinkpad.pad_flags().contains(gst::PadFlags::EOS) {
                    state.last_flow_ret = Err(gst::FlowError::Eos);
                    gst::debug!(CAT, imp = self, "EOS, waiting on cond");
                    state = self.cond.wait(state).unwrap();
                    gst::trace!(CAT, imp = self, "Woke up");
                } else if let Some(clock_wait) = state.clock_wait.clone() {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Waiting on timer with time {}, now {}",
                        clock_wait.time(),
                        clock_wait.clock().as_ref().map(gst::Clock::time).display(),
                    );

                    drop(state);
                    let res = clock_wait.wait();
                    state = self.state.lock().unwrap();

                    // Unset clock wait if it didn't change in the meantime: waiting again
                    // on it is going to return immediately and instead if there's nothing
                    // new to drain then we need to wait on the condvar
                    if state.clock_wait.as_ref() == Some(&clock_wait) {
                        state.clock_wait = None;
                    }

                    match res {
                        (Ok(_), jitter) => {
                            gst::trace!(CAT, imp = self, "Woke up after waiting for {}", jitter);
                            last_clock_wait_time = Some(clock_wait.time());
                        }
                        (Err(err), jitter) => {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "Woke up with error {:?} and jitter {}",
                                err,
                                jitter
                            );
                        }
                    }
                } else {
                    gst::debug!(CAT, imp = self, "Waiting on cond");
                    state = self.cond.wait(state).unwrap();
                    gst::trace!(CAT, imp = self, "Woke up");
                }

                // And retry if there's anything to drain now.
                continue;
            }

            drop(state);

            let mut res = Ok(());

            gst::trace!(CAT, imp = self, "Pushing {} items downstream", data.len());
            for data in data {
                match data {
                    BufferOrEvent::Event(event) => {
                        gst::trace!(CAT, imp = self, "Pushing event {:?}", event);
                        self.srcpad.push_event(event);
                    }
                    BufferOrEvent::Buffer(buffer) => {
                        gst::trace!(CAT, imp = self, "Pushing buffer {:?}", buffer);
                        if let Err(err) = self.srcpad.push(buffer) {
                            res = Err(err);
                            break;
                        }
                    }
                }
            }
            gst::trace!(CAT, imp = self, "Pushing returned {:?}", res);

            state = self.state.lock().unwrap();
            // If flushing or any other error then just return here
            state.last_flow_ret?;

            state.last_flow_ret = res.map(|_| gst::FlowSuccess::Ok);

            // Schedule a new clock wait now that data was drained in case we have to wait some
            // more time into the future on the next iteration
            self.reschedule_clock_wait(&mut state);

            // Loop and check if more data has to be drained now
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for OnvifMetadataParse {
    const NAME: &'static str = "GstOnvifMetadataParse";
    type Type = super::OnvifMetadataParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                OnvifMetadataParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                OnvifMetadataParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                OnvifMetadataParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_query(pad, query),
                )
            })
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                OnvifMetadataParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                OnvifMetadataParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.src_query(pad, query),
                )
            })
            .activatemode_function(|pad, _parent, mode, activate| {
                Self::src_activatemode(pad, mode, activate)
            })
            .flags(gst::PadFlags::PROXY_ALLOCATION)
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            settings: Mutex::default(),
            state: Mutex::default(),
            cond: Condvar::new(),
        }
    }
}

impl ObjectImpl for OnvifMetadataParse {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("latency")
                    .nick("Latency")
                    .blurb(
                        "Maximum latency to introduce for reordering metadata \
                     (max=auto: 6s if unparsed input, 0s if parsed input)",
                    )
                    .default_value(
                        Settings::default()
                            .latency
                            .map(|l| l.nseconds())
                            .unwrap_or(u64::MAX),
                    )
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("max-lateness")
                    .nick("Maximum Lateness")
                    .blurb("Drop metadata that delayed by more than this")
                    .default_value(
                        Settings::default()
                            .max_lateness
                            .map(|l| l.nseconds())
                            .unwrap_or(u64::MAX),
                    )
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "latency" => {
                self.settings.lock().unwrap().latency = value.get().expect("type checked upstream");

                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            "max-lateness" => {
                self.settings.lock().unwrap().max_lateness =
                    value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "latency" => self.settings.lock().unwrap().latency.to_value(),
            "max-lateness" => self.settings.lock().unwrap().max_lateness.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for OnvifMetadataParse {}

impl ElementImpl for OnvifMetadataParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIF Metadata Parser",
                "Metadata/Parser",
                "Parses ONVIF Timed XML Metadata",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-onvif-metadata")
                .field("parsed", true)
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::builder("application/x-onvif-metadata").build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
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

        if matches!(transition, gst::StateChange::ReadyToPaused) {
            let mut state = self.state.lock().unwrap();
            *state = State::default();
        }

        let res = self.parent_change_state(transition)?;

        if matches!(transition, gst::StateChange::PausedToReady) {
            let mut state = self.state.lock().unwrap();
            *state = State::default();
        }

        Ok(res)
    }
}
