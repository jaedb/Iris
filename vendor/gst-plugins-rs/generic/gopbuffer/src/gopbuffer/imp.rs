// Copyright (C) 2023 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-gopbuffer
 *
 * #gopbuffer is an element that can be used to store a minimum duration of data delimited by
 * discrete GOPs (Group of Picture).  It does this in by differentiation on the DELTA_UNIT
 * flag on each input buffer.
 *
 * One example of the usefulness of #gopbuffer is its ability to store a backlog of data starting
 * on a key frame boundary if say the previous 10s seconds of a stream would like to be recorded to
 * disk.
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch videotestsrc ! vp8enc ! gopbuffer minimum-duration=10000000000 ! fakesink
 * ]|
 *
 * Since: plugins-rs-0.13.0
 */
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::collections::VecDeque;
use std::sync::Mutex;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gopbuffer",
        gst::DebugColorFlags::empty(),
        Some("GopBuffer Element"),
    )
});

const DEFAULT_MIN_TIME: gst::ClockTime = gst::ClockTime::from_seconds(1);
const DEFAULT_MAX_TIME: Option<gst::ClockTime> = None;

#[derive(Debug, Clone)]
struct Settings {
    min_time: gst::ClockTime,
    max_time: Option<gst::ClockTime>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_time: DEFAULT_MIN_TIME,
            max_time: DEFAULT_MAX_TIME,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum DeltaFrames {
    /// Only single completely decodable frames
    IntraOnly,
    /// Frames may depend on past frames
    PredictiveOnly,
    /// Frames may depend on past or future frames
    Bidirectional,
}

impl DeltaFrames {
    /// Whether dts is required to order buffers differently from presentation order
    pub(crate) fn requires_dts(&self) -> bool {
        matches!(self, Self::Bidirectional)
    }
    /// Whether this coding structure does not allow delta flags on buffers
    pub(crate) fn intra_only(&self) -> bool {
        matches!(self, Self::IntraOnly)
    }

    pub(crate) fn from_caps(caps: &gst::CapsRef) -> Option<Self> {
        let s = caps.structure(0)?;
        Some(match s.name().as_str() {
            "video/x-h264" | "video/x-h265" => DeltaFrames::Bidirectional,
            "video/x-vp8" | "video/x-vp9" | "video/x-av1" => DeltaFrames::PredictiveOnly,
            "image/jpeg" | "image/png" | "video/x-raw" => DeltaFrames::IntraOnly,
            _ => return None,
        })
    }
}

// TODO: add buffer list support
#[derive(Debug)]
enum GopItem {
    Buffer(gst::Buffer),
    Event(gst::Event),
}

struct Gop {
    // all times are in running time
    start_pts: gst::ClockTime,
    start_dts: Option<gst::Signed<gst::ClockTime>>,
    earliest_pts: gst::ClockTime,
    final_earliest_pts: bool,
    end_pts: gst::ClockTime,
    end_dts: Option<gst::Signed<gst::ClockTime>>,
    final_end_pts: bool,
    // Buffer or event
    data: VecDeque<GopItem>,
}

impl Gop {
    fn push_on_pad(mut self, pad: &gst::Pad) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut iter = self.data.iter().filter_map(|item| match item {
            GopItem::Buffer(buffer) => buffer.pts(),
            _ => None,
        });
        let first_pts = iter.next();
        let last_pts = iter.next_back();
        gst::debug!(
            CAT,
            "pushing gop with start pts {} end pts {}",
            first_pts.display(),
            last_pts.display(),
        );
        for item in self.data.drain(..) {
            match item {
                GopItem::Buffer(buffer) => {
                    pad.push(buffer)?;
                }
                GopItem::Event(event) => {
                    pad.push_event(event);
                }
            }
        }
        Ok(gst::FlowSuccess::Ok)
    }
}

struct Stream {
    sinkpad: gst::Pad,
    srcpad: gst::Pad,

    sink_segment: Option<gst::FormattedSegment<gst::ClockTime>>,

    delta_frames: DeltaFrames,

    queued_gops: VecDeque<Gop>,
}

impl Stream {
    fn queue_buffer(
        &mut self,
        buffer: gst::Buffer,
        segment: &gst::FormattedSegment<gst::ClockTime>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let pts_position = buffer.pts().unwrap();
        let end_pts_position = pts_position
            .opt_add(buffer.duration())
            .unwrap_or(pts_position);

        let pts = segment
            .to_running_time_full(pts_position)
            .ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = self.sinkpad,
                    "Couldn't convert PTS to running time"
                );
                gst::FlowError::Error
            })?
            .positive()
            .unwrap_or_else(|| {
                gst::warning!(CAT, obj = self.sinkpad, "Negative PTSs are not supported");
                gst::ClockTime::ZERO
            });
        let end_pts = segment
            .to_running_time_full(end_pts_position)
            .ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = self.sinkpad,
                    "Couldn't convert end PTS to running time"
                );
                gst::FlowError::Error
            })?
            .positive()
            .unwrap_or_else(|| {
                gst::warning!(CAT, obj = self.sinkpad, "Negative PTSs are not supported");
                gst::ClockTime::ZERO
            });

        let (dts, end_dts) = if !self.delta_frames.requires_dts() {
            (None, None)
        } else {
            let dts_position = buffer.dts().expect("No dts");
            let end_dts_position = buffer
                .duration()
                .opt_add(dts_position)
                .unwrap_or(dts_position);

            let dts = segment.to_running_time_full(dts_position).ok_or_else(|| {
                gst::error!(
                    CAT,
                    obj = self.sinkpad,
                    "Couldn't convert DTS to running time"
                );
                gst::FlowError::Error
            })?;

            let end_dts = segment
                .to_running_time_full(end_dts_position)
                .ok_or_else(|| {
                    gst::error!(
                        CAT,
                        obj = self.sinkpad,
                        "Couldn't convert end DTS to running time"
                    );
                    gst::FlowError::Error
                })?;

            let end_dts = std::cmp::max(end_dts, dts);

            (Some(dts), Some(end_dts))
        };

        if !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            gst::debug!(
                CAT,
                "New GOP detected with buffer pts {} dts {}",
                buffer.pts().display(),
                buffer.dts().display()
            );
            let gop = Gop {
                start_pts: pts,
                start_dts: dts,
                earliest_pts: pts,
                final_earliest_pts: false,
                end_pts: pts,
                end_dts,
                final_end_pts: false,
                data: VecDeque::from([GopItem::Buffer(buffer)]),
            };
            self.queued_gops.push_front(gop);
            if let Some(prev_gop) = self.queued_gops.get_mut(1) {
                gst::debug!(
                    CAT,
                    obj = self.sinkpad,
                    "Updating previous GOP starting at PTS {} to end PTS {}",
                    prev_gop.earliest_pts,
                    pts,
                );

                prev_gop.end_pts = std::cmp::max(prev_gop.end_pts, pts);
                prev_gop.end_dts = std::cmp::max(prev_gop.end_dts, dts);

                if !self.delta_frames.requires_dts() {
                    prev_gop.final_end_pts = true;
                }

                if !prev_gop.final_earliest_pts {
                    // Don't bother logging this for intra-only streams as it would be for every
                    // single buffer.
                    if self.delta_frames.requires_dts() {
                        gst::debug!(
                            CAT,
                            obj = self.sinkpad,
                            "Previous GOP has final earliest PTS at {}",
                            prev_gop.earliest_pts
                        );
                    }

                    prev_gop.final_earliest_pts = true;
                    if let Some(prev_prev_gop) = self.queued_gops.get_mut(2) {
                        prev_prev_gop.final_end_pts = true;
                    }
                }
            }
        } else if let Some(gop) = self.queued_gops.front_mut() {
            gop.end_pts = std::cmp::max(gop.end_pts, end_pts);
            gop.end_dts = gop.end_dts.opt_max(end_dts);
            gop.data.push_back(GopItem::Buffer(buffer));

            if self.delta_frames.requires_dts() {
                let dts = dts.unwrap();

                if gop.earliest_pts > pts && !gop.final_earliest_pts {
                    gst::debug!(
                        CAT,
                        obj = self.sinkpad,
                        "Updating current GOP earliest PTS from {} to {}",
                        gop.earliest_pts,
                        pts
                    );
                    gop.earliest_pts = pts;

                    if let Some(prev_gop) = self.queued_gops.get_mut(1) {
                        if prev_gop.end_pts < pts {
                            gst::debug!(
                                CAT,
                                obj = self.sinkpad,
                                "Updating previous GOP starting PTS {} end time from {} to {}",
                                pts,
                                prev_gop.end_pts,
                                pts
                            );
                            prev_gop.end_pts = pts;
                        }
                    }
                }
                let gop = self.queued_gops.front_mut().unwrap();

                // The earliest PTS is known when the current DTS is bigger or equal to the first
                // PTS that was observed in this GOP. If there was another frame later that had a
                // lower PTS then it wouldn't be possible to display it in time anymore, i.e. the
                // stream would be invalid.
                if gop.start_pts <= dts && !gop.final_earliest_pts {
                    gst::debug!(
                        CAT,
                        obj = self.sinkpad,
                        "GOP has final earliest PTS at {}",
                        gop.earliest_pts
                    );
                    gop.final_earliest_pts = true;

                    if let Some(prev_gop) = self.queued_gops.get_mut(1) {
                        prev_gop.final_end_pts = true;
                    }
                }
            }
        } else {
            gst::debug!(
                CAT,
                "dropping buffer before first GOP with pts {} dts {}",
                buffer.pts().display(),
                buffer.dts().display()
            );
        }

        if let Some((prev_gop, first_gop)) = Option::zip(
            self.queued_gops.iter().find(|gop| gop.final_end_pts),
            self.queued_gops.back(),
        ) {
            gst::debug!(
                CAT,
                obj = self.sinkpad,
                "Queued full GOPs duration updated to {}",
                prev_gop.end_pts.saturating_sub(first_gop.earliest_pts),
            );
        }

        gst::debug!(
            CAT,
            obj = self.sinkpad,
            "Queued duration updated to {}",
            Option::zip(self.queued_gops.front(), self.queued_gops.back())
                .map(|(end, start)| end.end_pts.saturating_sub(start.start_pts))
                .unwrap_or(gst::ClockTime::ZERO)
        );

        Ok(gst::FlowSuccess::Ok)
    }

    fn oldest_gop(&mut self) -> Option<Gop> {
        self.queued_gops.pop_back()
    }

    fn peek_oldest_gop(&self) -> Option<&Gop> {
        self.queued_gops.back()
    }

    fn peek_second_oldest_gop(&self) -> Option<&Gop> {
        if self.queued_gops.len() <= 1 {
            return None;
        }
        self.queued_gops.get(self.queued_gops.len() - 2)
    }

    fn drain_all(&mut self) -> impl Iterator<Item = Gop> + '_ {
        self.queued_gops.drain(..).rev()
    }

    fn flush(&mut self) {
        self.queued_gops.clear();
    }
}

#[derive(Default)]
struct State {
    streams: Vec<Stream>,
}

impl State {
    fn stream_from_sink_pad(&self, pad: &gst::Pad) -> Option<&Stream> {
        self.streams.iter().find(|stream| &stream.sinkpad == pad)
    }
    fn stream_from_sink_pad_mut(&mut self, pad: &gst::Pad) -> Option<&mut Stream> {
        self.streams
            .iter_mut()
            .find(|stream| &stream.sinkpad == pad)
    }
    fn stream_from_src_pad(&self, pad: &gst::Pad) -> Option<&Stream> {
        self.streams.iter().find(|stream| &stream.srcpad == pad)
    }
}

#[derive(Default)]
pub(crate) struct GopBuffer {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl GopBuffer {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let obj = self.obj();
        if buffer.pts().is_none() {
            gst::error!(CAT, obj = obj, "Require timestamped buffers!");
            return Err(gst::FlowError::Error);
        }

        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.lock().unwrap();
        let stream = state
            .stream_from_sink_pad_mut(pad)
            .expect("pad without an internal Stream");

        let Some(segment) = stream.sink_segment.clone() else {
            gst::element_imp_error!(self, gst::CoreError::Clock, ["Got buffer before segment"]);
            return Err(gst::FlowError::Error);
        };

        if stream.delta_frames.intra_only() && buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
        {
            gst::error!(CAT, obj = pad, "Intra-only stream with delta units");
            return Err(gst::FlowError::Error);
        }

        if stream.delta_frames.requires_dts() && buffer.dts().is_none() {
            gst::error!(CAT, obj = pad, "Require DTS for video streams");
            return Err(gst::FlowError::Error);
        }

        let srcpad = stream.srcpad.clone();
        stream.queue_buffer(buffer, &segment)?;
        let mut gops_to_push = vec![];

        let Some(newest_gop) = stream.queued_gops.front() else {
            return Ok(gst::FlowSuccess::Ok);
        };
        // we are looking for the latest pts value here (which should be the largest)
        let newest_ts = if stream.delta_frames.requires_dts() {
            newest_gop.end_dts.unwrap()
        } else {
            gst::Signed::Positive(newest_gop.end_pts)
        };

        loop {
            // check stored times as though the oldest GOP doesn't exist.
            let Some(second_oldest_gop) = stream.peek_second_oldest_gop() else {
                break;
            };
            // we are looking for the oldest pts here (with the largest value).  This is our potentially
            // new end time.
            let oldest_ts = if stream.delta_frames.requires_dts() {
                second_oldest_gop.start_dts.unwrap()
            } else {
                gst::Signed::Positive(second_oldest_gop.start_pts)
            };

            let stored_duration_without_oldest = newest_ts.saturating_sub(oldest_ts);
            gst::trace!(
                CAT,
                obj = obj,
                "newest_pts {}, second oldest_pts {}, stored_duration_without_oldest_gop {}, min-time {}",
                newest_ts.display(),
                oldest_ts.display(),
                stored_duration_without_oldest.display(),
                settings.min_time.display()
            );
            if stored_duration_without_oldest < settings.min_time {
                break;
            }
            gops_to_push.push(stream.oldest_gop().unwrap());
        }

        if let Some(max_time) = settings.max_time {
            while let Some(oldest_gop) = stream.peek_oldest_gop() {
                let oldest_ts = oldest_gop.data.iter().rev().find_map(|item| match item {
                    GopItem::Buffer(buffer) => {
                        if stream.delta_frames.requires_dts() {
                            Some(gst::Signed::Positive(buffer.dts().unwrap()))
                        } else {
                            Some(gst::Signed::Positive(buffer.pts().unwrap()))
                        }
                    }
                    _ => None,
                });
                if newest_ts
                    .opt_saturating_sub(oldest_ts)
                    .is_some_and(|diff| diff > gst::Signed::Positive(max_time))
                {
                    gst::warning!(CAT, obj = obj, "Stored data has overflowed the maximum allowed stored time {}, pushing oldest GOP", max_time.display());
                    gops_to_push.push(stream.oldest_gop().unwrap());
                } else {
                    break;
                }
            }
        }

        drop(state);
        for gop in gops_to_push.into_iter() {
            gop.push_on_pad(&srcpad)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        let obj = self.obj();
        let mut state = self.state.lock().unwrap();
        let stream = state
            .stream_from_sink_pad_mut(pad)
            .expect("pad without an internal Stream!");
        match event.view() {
            gst::EventView::Caps(caps) => {
                let Some(delta_frames) = DeltaFrames::from_caps(caps.caps()) else {
                    return false;
                };
                stream.delta_frames = delta_frames;
            }
            gst::EventView::FlushStop(_flush) => {
                gst::debug!(CAT, obj = obj, "flushing stored data");
                stream.flush();
            }
            gst::EventView::Eos(_eos) => {
                gst::debug!(CAT, obj = obj, "draining data at EOS");
                let gops = stream.drain_all().collect::<Vec<_>>();
                let srcpad = stream.srcpad.clone();
                drop(state);
                for gop in gops.into_iter() {
                    let _ = gop.push_on_pad(&srcpad);
                }
                // once we've pushed all the data, we can push the corresponding eos
                gst::Pad::event_default(pad, Some(&*obj), event);
                return true;
            }
            gst::EventView::Segment(segment) => {
                let Ok(segment) = segment.segment().clone().downcast::<gst::ClockTime>() else {
                    gst::error!(CAT, "Non TIME segments are not supported");
                    return false;
                };
                stream.sink_segment = Some(segment);
            }
            _ => (),
        };

        if event.is_serialized() {
            if stream.peek_oldest_gop().is_none() {
                // if there is nothing queued, the event can go straight through
                gst::trace!(
                    CAT,
                    obj = obj,
                    "nothing queued, event {:?} passthrough",
                    event.structure().map(|s| s.name().as_str())
                );
                drop(state);
                return gst::Pad::event_default(pad, Some(&*obj), event);
            }
            let gop = stream.queued_gops.front_mut().unwrap();
            gop.data.push_back(GopItem::Event(event));
            true
        } else {
            // non-serialized events can be pushed directly
            drop(state);
            gst::Pad::event_default(pad, Some(&*obj), event)
        }
    }

    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        let obj = self.obj();
        if query.is_serialized() {
            // TODO: serialized queries somehow?
            gst::warning!(
                CAT,
                obj = pad,
                "Serialized queries are currently not supported"
            );
            return false;
        }
        gst::Pad::query_default(pad, Some(&*obj), query)
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        let obj = self.obj();
        match query.view_mut() {
            gst::QueryViewMut::Latency(latency) => {
                let mut upstream_query = gst::query::Latency::new();
                let otherpad = {
                    let state = self.state.lock().unwrap();
                    let Some(stream) = state.stream_from_src_pad(pad) else {
                        return false;
                    };
                    stream.sinkpad.clone()
                };
                let ret = otherpad.peer_query(&mut upstream_query);

                if ret {
                    let (live, mut min, mut max) = upstream_query.result();

                    let settings = self.settings.lock().unwrap();
                    min += settings.max_time.unwrap_or(settings.min_time);
                    max = max.opt_max(settings.max_time);

                    latency.set(live, min, max);

                    gst::debug!(
                        CAT,
                        obj = pad,
                        "Latency query response: live {} min {} max {}",
                        live,
                        min,
                        max.display()
                    );
                }
                ret
            }
            _ => gst::Pad::query_default(pad, Some(&*obj), query),
        }
    }

    fn iterate_internal_links(&self, pad: &gst::Pad) -> gst::Iterator<gst::Pad> {
        let state = self.state.lock().unwrap();
        let otherpad = match pad.direction() {
            gst::PadDirection::Src => state
                .stream_from_src_pad(pad)
                .map(|stream| stream.sinkpad.clone()),
            gst::PadDirection::Sink => state
                .stream_from_sink_pad(pad)
                .map(|stream| stream.srcpad.clone()),
            _ => unreachable!(),
        };
        if let Some(otherpad) = otherpad {
            gst::Iterator::from_vec(vec![otherpad])
        } else {
            gst::Iterator::from_vec(vec![])
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for GopBuffer {
    const NAME: &'static str = "GstGopBuffer";
    type Type = super::GopBuffer;
    type ParentType = gst::Element;
}

impl ObjectImpl for GopBuffer {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("minimum-duration")
                    .nick("Minimum Duration")
                    .blurb("The minimum duration to store")
                    .default_value(DEFAULT_MIN_TIME.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("max-size-time")
                    .nick("Maximum Duration")
                    .blurb("The maximum duration to store (0=disable)")
                    .default_value(0)
                    .mutable_ready()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "minimum-duration" => {
                let mut settings = self.settings.lock().unwrap();
                let min_time = value.get().expect("type checked upstream");
                if settings.min_time != min_time {
                    settings.min_time = min_time;
                    drop(settings);
                    self.post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }
            "max-size-time" => {
                let mut settings = self.settings.lock().unwrap();
                let max_time = value
                    .get::<Option<gst::ClockTime>>()
                    .expect("type checked upstream");
                let max_time = if matches!(max_time, Some(gst::ClockTime::ZERO) | None) {
                    None
                } else {
                    max_time
                };
                if settings.max_time != max_time {
                    settings.max_time = max_time;
                    drop(settings);
                    self.post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }

            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "minimum-duration" => {
                let settings = self.settings.lock().unwrap();
                settings.min_time.to_value()
            }
            "max-size-time" => {
                let settings = self.settings.lock().unwrap();
                settings.max_time.unwrap_or(gst::ClockTime::ZERO).to_value()
            }

            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        let class = obj.class();
        let templ = class.pad_template("video_sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .name("video_sink")
            .chain_function(|pad, parent, buffer| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |gopbuffer| gopbuffer.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || false,
                    |gopbuffer| gopbuffer.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || false,
                    |gopbuffer| gopbuffer.sink_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || gst::Pad::iterate_internal_links_default(pad, parent),
                    |gopbuffer| gopbuffer.iterate_internal_links(pad),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();
        obj.add_pad(&sinkpad).unwrap();

        let templ = class.pad_template("video_src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .name("video_src")
            .query_function(|pad, parent, query| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || false,
                    |gopbuffer| gopbuffer.src_query(pad, query),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                GopBuffer::catch_panic_pad_function(
                    parent,
                    || gst::Pad::iterate_internal_links_default(pad, parent),
                    |gopbuffer| gopbuffer.iterate_internal_links(pad),
                )
            })
            .build();
        obj.add_pad(&srcpad).unwrap();

        let mut state = self.state.lock().unwrap();
        state.streams.push(Stream {
            sinkpad,
            srcpad,
            sink_segment: None,
            delta_frames: DeltaFrames::IntraOnly,
            queued_gops: VecDeque::new(),
        });
    }
}

impl GstObjectImpl for GopBuffer {}

impl ElementImpl for GopBuffer {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "GopBuffer",
                "Video",
                "GOP Buffer",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // This element is designed to implement multiple streams but it has not been
            // implemented.
            //
            // The things missing for multiple (audio or video) streams are:
            // 1. More pad templates
            // 2. Choosing a main stream to drive the timestamp logic between all input streams
            // 3. Allowing either the main stream to cause other streams to push data
            //    regardless of it's GOP state, or allow each stream to be individually delimited
            //    by GOP but all still within the minimum duration.
            let video_caps = [
                gst::Structure::builder("video/x-h264")
                    .field("stream-format", gst::List::new(["avc", "avc3"]))
                    .field("alignment", "au")
                    .build(),
                gst::Structure::builder("video/x-h265")
                    .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                    .field("alignment", "au")
                    .build(),
                gst::Structure::builder("video/x-vp8").build(),
                gst::Structure::builder("video/x-vp9").build(),
                gst::Structure::builder("video/x-av1")
                    .field("stream-format", "obu-stream")
                    .field("alignment", "tu")
                    .build(),
            ]
            .into_iter()
            .collect::<gst::Caps>();

            let src_pad_template = gst::PadTemplate::new(
                "video_src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &video_caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "video_sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &video_caps,
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
        #[allow(clippy::single_match)]
        match transition {
            gst::StateChange::NullToReady => {
                let settings = self.settings.lock().unwrap();
                if let Some(max_time) = settings.max_time {
                    if max_time < settings.min_time {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::StateChange,
                            ["Configured maximum time is less than the minimum time"]
                        );
                        return Err(gst::StateChangeError);
                    }
                }
            }
            _ => (),
        }

        self.parent_change_state(transition)?;

        Ok(gst::StateChangeSuccess::Success)
    }
}
