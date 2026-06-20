// SPDX-License-Identifier: MPL-2.0

use glib::{prelude::*, subclass::prelude::*};
use gst::{prelude::*, subclass::prelude::*};
use gst_base::{prelude::*, subclass::prelude::*};

use std::{
    cmp,
    collections::{BTreeMap, VecDeque},
    mem,
    sync::{LazyLock, Mutex},
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "analyticscombiner",
        gst::DebugColorFlags::empty(),
        Some("Analytics combiner / batcher element"),
    )
});

#[derive(Default)]
struct State {
    // Sorted by index
    sinkpads: Vec<super::AnalyticsCombinerSinkPad>,
}

#[derive(Clone)]
struct Settings {
    batch_duration: gst::ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            batch_duration: gst::ClockTime::from_mseconds(100),
        }
    }
}

#[derive(Default)]
pub struct AnalyticsCombiner {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

#[glib::object_subclass]
impl ObjectSubclass for AnalyticsCombiner {
    const NAME: &'static str = "GstAnalyticsCombiner";
    type Type = super::AnalyticsCombiner;
    type ParentType = gst_base::Aggregator;
    type Interfaces = (gst::ChildProxy,);
}

impl ObjectImpl for AnalyticsCombiner {
    fn constructed(&self) {
        self.parent_constructed();

        let settings = self.settings.lock().unwrap().clone();
        self.obj().set_latency(settings.batch_duration, None);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("batch-duration")
                    .nick("Batch Duration")
                    .blurb("Batch Duration")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("force-live")
                    .nick("Force Live")
                    .blurb(
                        "Always operate in live mode and aggregate on timeout regardless of \
                         whether any live sources are linked upstream",
                    )
                    .construct_only()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "batch-duration" => {
                let mut settings = self.settings.lock().unwrap();
                let batch_duration = value.get().unwrap();

                if batch_duration != settings.batch_duration {
                    settings.batch_duration = batch_duration;
                    drop(settings);
                    self.update_latency();
                }
            }
            "force-live" => {
                self.obj().set_force_live(value.get().unwrap());
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "batch-duration" => self.settings.lock().unwrap().batch_duration.to_value(),
            "force-live" => self.obj().is_force_live().to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for AnalyticsCombiner {}

impl ElementImpl for AnalyticsCombiner {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Analytics Combiner / Batcher",
                "Combiner/Analytics",
                "Analytics combiner / batcher element",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::with_gtype(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("multistream/x-analytics-batch")
                    .features([gst_analytics::CAPS_FEATURE_META_ANALYTICS_BATCH_META])
                    .build(),
                gst_base::AggregatorPad::static_type(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &gst::Caps::new_any(),
                super::AnalyticsCombinerSinkPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let pad = self.parent_request_new_pad(templ, name, caps)?;
        self.obj().child_added(&pad, &pad.name());
        Some(pad)
    }

    fn release_pad(&self, pad: &gst::Pad) {
        self.obj().child_removed(pad, &pad.name());
        self.parent_release_pad(pad);
    }
}

impl AggregatorImpl for AnalyticsCombiner {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state_guard = self.state.lock().unwrap();
        *state_guard = State::default();

        let mut sinkpads = self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|pad| pad.downcast::<super::AnalyticsCombinerSinkPad>().unwrap())
            .collect::<Vec<_>>();
        sinkpads.sort_by(|a, b| {
            let index_a = a.property::<u32>("index");
            let index_b = b.property::<u32>("index");
            index_a.cmp(&index_b).then_with(|| a.name().cmp(&b.name()))
        });

        if sinkpads.is_empty() {
            gst::error!(CAT, imp = self, "Can't start without sink pads");
            return Err(gst::error_msg!(
                gst::CoreError::StateChange,
                ("Can't start without sink pads")
            ));
        }

        for (idx, pad) in sinkpads.iter_mut().enumerate() {
            let index_pad = pad.property::<u32>("index");
            if idx as u32 != index_pad {
                gst::warning!(
                    CAT,
                    obj = pad,
                    "Updating pad from index {index_pad} to {idx}"
                );
                pad.set_property("index", idx as u32);
            } else {
                gst::debug!(CAT, obj = pad, "Using index {idx}");
            }
        }
        state_guard.sinkpads = sinkpads;

        gst::debug!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state_guard = self.state.lock().unwrap();

        for pad in &state_guard.sinkpads {
            let pad_imp = pad.imp();
            let mut pad_state = pad_imp.state.lock().unwrap();
            *pad_state = Default::default();
        }

        *state_guard = State::default();
        drop(state_guard);

        gst::debug!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn create_new_pad(
        &self,
        templ: &gst::PadTemplate,
        req_name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<gst_base::AggregatorPad> {
        let state_guard = self.state.lock().unwrap();
        if !state_guard.sinkpads.is_empty() {
            gst::warning!(CAT, imp = self, "Can't add new pads after started");
            return None;
        }
        drop(state_guard);

        self.parent_create_new_pad(templ, req_name, caps)
    }

    fn next_time(&self) -> Option<gst::ClockTime> {
        self.obj().simple_get_next_time()
    }

    fn aggregate(&self, timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Aggregate (timeout: {timeout})");

        let settings = self.settings.lock().unwrap().clone();

        let Some(segment) = self
            .obj()
            .src_pad()
            .segment()
            .downcast::<gst::ClockTime>()
            .ok()
        else {
            gst::error!(CAT, imp = self, "Non-TIME segment");
            gst::element_imp_error!(self, gst::CoreError::Clock, ["Received non-time segment"]);
            return Err(gst::FlowError::Error);
        };

        let start_position = segment.position().unwrap_or(segment.start().unwrap());
        let end_position = start_position + settings.batch_duration;
        gst::trace!(
            CAT,
            imp = self,
            "Collecting buffers for batch {start_position}-{end_position}"
        );

        let state_guard = self.state.lock().unwrap();
        self.fill_queues(
            &state_guard,
            &settings,
            timeout,
            start_position,
            end_position,
        )?;
        let buffer = self.drain(&state_guard, &settings, start_position, end_position);

        // Renegotiate if caps where processed
        self.negotiate(state_guard);

        gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}",);

        let res = self.obj().finish_buffer(buffer)?;

        self.obj().set_position(end_position);

        Ok(res)
    }

    fn sink_event(&self, pad: &gst_base::AggregatorPad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {event:?}");

        let pad = pad
            .downcast_ref::<super::AnalyticsCombinerSinkPad>()
            .unwrap();
        let pad_imp = pad.imp();

        if event.is_serialized() {
            let mut pad_state = pad_imp.state.lock().unwrap();

            pad_state.pending_serialized_events.push_back(event.clone());
        }

        self.parent_sink_event(pad.upcast_ref(), event)
    }

    #[allow(clippy::single_match)]
    fn sink_query(&self, pad: &gst_base::AggregatorPad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        let pad = pad
            .downcast_ref::<super::AnalyticsCombinerSinkPad>()
            .unwrap();

        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let filter = q.filter_owned();
                let peer_caps = self.obj().src_pad().peer_query_caps(None);

                if peer_caps.is_any() {
                    let res = filter.unwrap_or(peer_caps);
                    gst::log!(CAT, obj = pad, "Returning caps {res:?}");
                    q.set_result(&res);
                } else if peer_caps.is_empty() {
                    gst::log!(CAT, obj = pad, "Returning caps {peer_caps:?}");

                    q.set_result(&peer_caps);
                } else {
                    let state_guard = self.state.lock().unwrap();
                    let pad_index = state_guard
                        .sinkpads
                        .iter()
                        .enumerate()
                        .find_map(|(idx, p)| (pad == p).then_some(idx));
                    drop(state_guard);

                    // Shouldn't happen, pad not found!
                    let Some(pad_index) = pad_index else {
                        gst::warning!(CAT, obj = pad, "Unknown pad");
                        return false;
                    };

                    // return expected caps for this pad from downstream, if any
                    let mut res = gst::Caps::new_empty();
                    for s in peer_caps.iter() {
                        let Some(streams) = s.get::<gst::ArrayRef>("streams").ok() else {
                            continue;
                        };

                        let Some(stream_caps) = streams
                            .get(pad_index)
                            .and_then(|v| v.get::<Option<gst::Caps>>().unwrap())
                        else {
                            continue;
                        };

                        res.merge(stream_caps);
                    }

                    // No stream specific caps found, return ANY
                    let res = if res.is_empty() {
                        filter.unwrap_or(gst::Caps::new_any())
                    } else {
                        filter
                            .as_ref()
                            .map(|filter| {
                                filter.intersect_with_mode(&res, gst::CapsIntersectMode::First)
                            })
                            .unwrap_or(res)
                    };

                    gst::log!(CAT, obj = pad, "Returning caps {res:?}");

                    q.set_result(&res);
                }

                return true;
            }
            _ => (),
        }

        self.parent_sink_query(pad.upcast_ref(), query)
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let state_guard = self.state.lock().unwrap();
                let sinkpads = state_guard.sinkpads.clone();
                drop(state_guard);

                let streams = if sinkpads.is_empty() {
                    None
                } else {
                    Some(
                        sinkpads
                            .iter()
                            .map(|pad| pad.peer_query_caps(None))
                            .map(|caps| caps.to_send_value())
                            .collect::<gst::Array>(),
                    )
                };

                let res = gst::Caps::builder("multistream/x-analytics-batch")
                    .features([gst_analytics::CAPS_FEATURE_META_ANALYTICS_BATCH_META])
                    .field_if_some("streams", streams)
                    .build();

                let filter = q.filter();
                let res = &filter
                    .map(|filter| filter.intersect_with_mode(&res, gst::CapsIntersectMode::First))
                    .unwrap_or(res);
                q.set_result(res);

                gst::log!(CAT, imp = self, "Returning caps {res:?}");

                return true;
            }
            _ => (),
        }

        self.parent_src_query(query)
    }

    // This gets called too early, we want to wait until something has been
    // collected on every pad before doing the negotiation. So this class will
    // now trigger it by itself and implement its own private negotiate
    // method
    fn negotiate(&self) -> bool {
        true
    }
}

impl ChildProxyImpl for AnalyticsCombiner {
    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        self.obj()
            .sink_pads()
            .get(index as usize)
            .cloned()
            .map(|pad| pad.upcast())
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        self.obj()
            .sink_pads()
            .into_iter()
            .find(|pad| pad.name() == name)
            .map(|pad| pad.upcast())
    }

    fn children_count(&self) -> u32 {
        self.obj().num_sink_pads() as u32
    }
}

impl AnalyticsCombiner {
    fn update_latency(&self) {
        let mut latency = self.settings.lock().unwrap().batch_duration;
        let with_overlap = self.obj().sink_pads().into_iter().any(|pad| {
            pad.property::<super::BatchStrategy>("batch-strategy")
                == super::BatchStrategy::FirstInBatchWithOverlap
        });

        if with_overlap {
            latency += latency / 2;
        }

        self.obj().set_latency(latency, None);
    }

    fn fill_queues(
        &self,
        state: &State,
        settings: &Settings,
        timeout: bool,
        start_position: gst::ClockTime,
        end_position: gst::ClockTime,
    ) -> Result<(), gst::FlowError> {
        let mut all_eos = true;
        let mut all_done = true;

        'next_pad: for pad in &state.sinkpads {
            let pad_imp = pad.imp();
            let mut pad_state = pad_imp.state.lock().unwrap();
            let pad_settings = pad_imp.settings.lock().unwrap().clone();

            // Take any leftover previous buffer now if it's not too far in the past
            if pad_settings.batch_strategy == super::BatchStrategy::FirstInBatchWithOverlap {
                if let Some(buffer) = pad_state.previous_buffer.take() {
                    if buffer.running_time.is_none_or(|running_time| {
                        running_time >= start_position.saturating_sub(settings.batch_duration / 2)
                    }) {
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Taking previous buffer {:?} with running time {}",
                            buffer.buffer,
                            buffer.running_time.display(),
                        );
                        pad_state.pending_buffers.push_back(buffer);
                    } else {
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Dropping previous buffer {:?} with running time {}",
                            buffer.buffer,
                            buffer.running_time.display(),
                        );
                    }
                }
            }

            loop {
                all_eos = all_eos && pad.is_eos() && pad_state.pending_buffers.is_empty();
                if pad.is_eos() {
                    // This pad is done for this batch, no need to update all_done
                    gst::trace!(CAT, obj = pad, "Pad is EOS");
                    continue 'next_pad;
                }

                let Some(buffer) = pad.peek_buffer() else {
                    // Not EOS and no buffer, so we don't know if this pad still has buffers
                    // for this batch
                    all_done = false;
                    gst::trace!(CAT, obj = pad, "Pad has no pending buffer");
                    continue 'next_pad;
                };

                let Some(segment) = pad.segment().downcast::<gst::ClockTime>().ok() else {
                    gst::error!(CAT, obj = pad, "Non-TIME segment");
                    gst::element_imp_error!(
                        self,
                        gst::CoreError::Clock,
                        ["Received non-time segment"]
                    );
                    return Err(gst::FlowError::Error);
                };

                let Some(ts) = buffer.dts_or_pts() else {
                    // Buffers without PTS are immediately taken
                    gst::trace!(
                        CAT,
                        obj = pad,
                        "Taking buffer {buffer:?} without DTS or PTS"
                    );
                    let buffer = BatchBuffer {
                        running_time: None,
                        serialized_events: pad_state.pending_serialized_events.drain(..).collect(),
                        buffer: Some(buffer),
                    };
                    pad_state.pending_buffers.push_back(buffer);
                    let _ = pad.drop_buffer();
                    // Check next buffer
                    continue;
                };

                let running_time = segment.to_running_time_full(ts).unwrap();
                if running_time < end_position {
                    // Buffer is for this batch
                    gst::trace!(
                        CAT,
                        obj = pad,
                        "Taking buffer {buffer:?} with running time {running_time}"
                    );
                } else if pad_state.pending_buffers.is_empty()
                    && pad_settings.batch_strategy != super::BatchStrategy::FirstInBatchWithOverlap
                    && running_time < end_position + settings.batch_duration / 2
                {
                    // Buffer is still for this batch because of the batch strategy
                    gst::trace!(
                    CAT,
                    obj = pad,
                    "Taking future buffer {buffer:?} with running time {running_time} because of batch strategy"
                );
                } else {
                    // Buffer is for the next batch
                    gst::trace!(
                        CAT,
                        obj = pad,
                        "Keeping buffer {buffer:?} with running time {running_time} for next batch"
                    );
                    continue 'next_pad;
                }

                let buffer = BatchBuffer {
                    running_time: Some(running_time),
                    serialized_events: pad_state.pending_serialized_events.drain(..).collect(),
                    buffer: Some(buffer),
                };
                pad_state.pending_buffers.push_back(buffer);
                let _ = pad.drop_buffer();
                // Check next buffer
            }
        }

        if all_eos && !self.obj().is_force_live() {
            gst::debug!(CAT, imp = self, "All pads EOS");
            return Err(gst::FlowError::Eos);
        }

        if !all_done && !timeout {
            gst::trace!(CAT, imp = self, "Waiting for more data");
            return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
        }

        Ok(())
    }

    fn drain(
        &self,
        state: &State,
        settings: &Settings,
        start_position: gst::ClockTime,
        _end_position: gst::ClockTime,
    ) -> gst::Buffer {
        let mut buffer = gst::Buffer::new();
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(start_position);
            buffer.set_duration(settings.batch_duration);

            let mut meta = gst_analytics::AnalyticsBatchMeta::add(buffer);

            unsafe {
                let meta = &mut *(meta.as_mut_ptr());

                meta.streams = glib::ffi::g_malloc0_n(
                    state.sinkpads.len(),
                    mem::size_of::<gst_analytics::ffi::GstAnalyticsBatchStream>(),
                )
                    as *mut gst_analytics::ffi::GstAnalyticsBatchStream;
                meta.n_streams = state.sinkpads.len();
            }

            for pad in state.sinkpads.iter() {
                let pad_imp = pad.imp();
                let mut pad_state = pad_imp.state.lock().unwrap();
                let pad_settings = pad_imp.settings.lock().unwrap().clone();

                // Post-filter the pending buffers based on the batch strategy
                match pad_settings.batch_strategy {
                    crate::combiner::BatchStrategy::All => {
                        // Nothing to do here, take all buffers
                    }
                    crate::combiner::BatchStrategy::FirstInBatch => {
                        if pad_state.pending_buffers.len() > 1 {
                            // Drop all but the first buffer and store the serialized events
                            // for the next batch
                            let mut serialized_events = vec![];
                            for buffer in pad_state.pending_buffers.drain(1..) {
                                gst::trace!(
                                    CAT, obj = pad,
                                    "Dropping buffer {:?} with running time {} because of batch strategy",
                                    buffer.buffer,
                                    buffer.running_time.display(),
                                );
                                for event in buffer.serialized_events {
                                    serialized_events.push(event);
                                }
                            }

                            for event in serialized_events.into_iter().rev() {
                                pad_state.pending_serialized_events.push_front(event);
                            }
                        }
                    }
                    crate::combiner::BatchStrategy::LastInBatch => {
                        if pad_state.pending_buffers.len() > 1 {
                            // Drop all but the last buffer and store the serialized events
                            // for the first buffer
                            let mut selected_buffer = pad_state.pending_buffers.pop_back().unwrap();

                            let mut serialized_events = vec![];
                            for buffer in pad_state.pending_buffers.drain(..) {
                                gst::trace!(
                                    CAT, obj = pad,
                                    "Dropping buffer {:?} with running time {} because of batch strategy",
                                    buffer.buffer,
                                    buffer.running_time.display(),
                                );
                                for event in buffer.serialized_events {
                                    serialized_events.push(event);
                                }
                            }
                            for event in selected_buffer.serialized_events.drain(..) {
                                serialized_events.push(event);
                            }
                            selected_buffer.serialized_events = serialized_events;

                            // And store the selected buffer in the pending buffers for the meta
                            pad_state.pending_buffers.push_back(selected_buffer);
                        }
                    }
                    crate::combiner::BatchStrategy::FirstInBatchWithOverlap => {
                        if pad_state.pending_buffers.len() > 1 {
                            // Take the buffer closest to the batch start, and drop all others.
                            // Keep the last buffer around if there is any and store all serialized
                            // events of the dropped buffers.
                            let selected_buffer = {
                                let first_buffer = pad_state.pending_buffers.pop_front().unwrap();
                                // If the first buffer has no running time, select it directly
                                if first_buffer.running_time.is_none() {
                                    first_buffer
                                } else {
                                    let second_buffer = pad_state.pending_buffers.front().unwrap();

                                    if second_buffer.running_time.is_none() {
                                        // If the second buffer has no running time, select the first
                                        // buffer
                                        first_buffer
                                    } else {
                                        // If both have a running time, select the one closest to the
                                        // batch start
                                        let first_buffer_distance = first_buffer
                                            .running_time
                                            .map(|running_time| {
                                                (running_time - start_position).abs()
                                            })
                                            .unwrap();
                                        let second_buffer_distance = second_buffer
                                            .running_time
                                            .map(|running_time| {
                                                (running_time - start_position).abs()
                                            })
                                            .unwrap();

                                        if first_buffer_distance <= second_buffer_distance {
                                            first_buffer
                                        } else {
                                            pad_state.pending_buffers.pop_front().unwrap()
                                        }
                                    }
                                }
                            };

                            // Keep the last buffer around, if any
                            let last_buffer = pad_state.pending_buffers.pop_back();

                            // Drain all others and keep serialized events
                            let mut serialized_events = vec![];
                            for buffer in pad_state.pending_buffers.drain(..) {
                                gst::trace!(
                                    CAT, obj = pad,
                                    "Dropping buffer {:?} with running time {} because of batch strategy",
                                    buffer.buffer,
                                    buffer.running_time.display(),
                                );
                                for event in buffer.serialized_events {
                                    serialized_events.push(event);
                                }
                            }

                            if let Some(mut last_buffer) = last_buffer {
                                gst::trace!(
                                    CAT, obj = pad,
                                    "Keeping last buffer {:?} with running time {} for next batch because of batch strategy",
                                    last_buffer.buffer,
                                    last_buffer.running_time.display(),
                                );
                                for event in last_buffer.serialized_events.drain(..) {
                                    serialized_events.push(event);
                                }
                                last_buffer.serialized_events = serialized_events;
                                pad_state.previous_buffer = Some(last_buffer);
                            } else {
                                pad_state.previous_buffer = None;
                                for event in serialized_events.into_iter().rev() {
                                    pad_state.pending_serialized_events.push_front(event);
                                }
                            }

                            // And store the selected buffer in the pending buffers for the meta
                            pad_state.pending_buffers.push_back(selected_buffer);
                        }
                    }
                }

                // Include all pending events and all the serialized events if
                // there is no buffer for this pad at this time.
                if pad_state.pending_buffers.is_empty() {
                    let buffer = BatchBuffer {
                        running_time: None,
                        serialized_events: pad_state.pending_serialized_events.drain(..).collect(),
                        buffer: None,
                    };
                    pad_state.pending_buffers.push_back(buffer);
                } else if let Some(mut b) = pad_state.pending_buffers.pop_front() {
                    while !b.serialized_events.is_empty() {
                        let event = &mut b.serialized_events[0];
                        if !event.is_sticky() {
                            break;
                        }
                        if pad_state.store_sticky_event(event) {
                            self.obj().src_pad().mark_reconfigure();
                        }
                        b.serialized_events.remove(0);
                    }
                    pad_state.pending_buffers.push_front(b);
                }
            }

            for (idx, pad) in state.sinkpads.iter().enumerate() {
                let mut pad_state = pad.imp().state.lock().unwrap();

                // And finally fill the meta
                unsafe {
                    use glib::translate::IntoGlibPtr;

                    let meta = &mut *(meta.as_mut_ptr());
                    let stream = &mut *meta.streams.add(idx);
                    stream.index = idx as u32;

                    let sticky_events: Vec<_> = pad_state.sticky_events.values().collect();

                    stream.n_sticky_events = sticky_events.len();

                    stream.sticky_events = glib::ffi::g_malloc0_n(
                        stream.n_sticky_events,
                        mem::size_of::<*mut gst::ffi::GstEvent>(),
                    ) as *mut *mut gst::ffi::GstEvent;

                    for (event_id, event) in sticky_events.into_iter().enumerate() {
                        *stream.sticky_events.add(event_id) = event.clone().into_glib_ptr();
                    }

                    stream.n_objects = pad_state
                        .pending_buffers
                        .iter()
                        .map(|b| b.serialized_events.len() + b.buffer.is_some() as usize)
                        .sum();
                    stream.objects = glib::ffi::g_malloc0_n(
                        stream.n_objects,
                        mem::size_of::<*mut gst::ffi::GstMiniObject>(),
                    ) as *mut *mut gst::ffi::GstMiniObject;

                    let mut count = 0;
                    for buffer in pad_state
                        .pending_buffers
                        .drain(..)
                        .collect::<Vec<_>>()
                        .into_iter()
                    {
                        for ev in buffer.serialized_events.into_iter() {
                            if ev.is_sticky() && pad_state.store_sticky_event(&ev) {
                                self.obj().src_pad().mark_reconfigure();
                            }

                            *stream.objects.add(count) = ev.upcast().into_glib_ptr();
                            count += 1;
                        }

                        // Replace GAP buffers with a GAP event
                        if let Some(b) = buffer.buffer {
                            if b.flags().contains(gst::BufferFlags::GAP)
                                && b.flags().contains(gst::BufferFlags::DROPPABLE)
                                && b.size() == 0
                            {
                                let ev = gst::event::Gap::builder(b.pts().unwrap())
                                    .duration(b.duration())
                                    .build();
                                *stream.objects.add(count) = ev.upcast().into_glib_ptr();
                            } else {
                                *stream.objects.add(count) = b.upcast().into_glib_ptr();
                            }
                            count += 1;
                        }
                    }
                }
            }
        }

        buffer
    }

    fn negotiate(&self, state_guard: std::sync::MutexGuard<State>) {
        if !self.obj().src_pad().check_reconfigure() {
            return;
        }

        let mut streams = gst::Array::default();
        for pad in &state_guard.sinkpads {
            let pad_imp = pad.imp();
            let pad_state = pad_imp.state.lock().unwrap();

            let caps = pad_state.caps.clone().unwrap_or_else(|| {
                gst::warning!(CAT, obj = pad, "No caps for pad, using empty caps for now");
                gst::Caps::new_empty_simple("application/unknown")
            });

            streams.append(caps);
        }

        let caps = gst::Caps::builder("multistream/x-analytics-batch")
            .features([gst_analytics::CAPS_FEATURE_META_ANALYTICS_BATCH_META])
            .field("streams", streams)
            .build();

        gst::debug!(CAT, imp = self, "Configuring caps {caps:?}");

        drop(state_guard);

        self.obj().set_src_caps(&caps);
    }
}

#[derive(Debug, Clone, Copy)]
struct OrderedEventType(gst::EventType);

impl cmp::PartialEq for OrderedEventType {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(cmp::Ordering::Equal)
    }
}

impl cmp::Eq for OrderedEventType {}

impl cmp::PartialOrd for OrderedEventType {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for OrderedEventType {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or_else(|| {
            use glib::translate::IntoGlib;

            self.0.into_glib().cmp(&other.0.into_glib())
        })
    }
}

struct BatchBuffer {
    running_time: Option<gst::Signed<gst::ClockTime>>,
    serialized_events: Vec<gst::Event>,
    buffer: Option<gst::Buffer>,
}

#[derive(Default)]
struct PadState {
    sticky_events: BTreeMap<(OrderedEventType, Option<String>), gst::Event>,
    pending_serialized_events: VecDeque<gst::Event>,
    pending_buffers: VecDeque<BatchBuffer>,
    previous_buffer: Option<BatchBuffer>,
    caps: Option<gst::Caps>,
}

impl PadState {
    fn store_sticky_event(&mut self, event: &gst::Event) -> bool {
        use gst::EventView;
        let mut ret = false;

        match event.view() {
            EventView::Caps(caps) => {
                let caps = caps.caps_owned();

                gst::debug!(CAT, "Received new caps {caps:?}");
                self.caps = Some(caps);

                // Renegotiate before the next aggregate call
                ret = true;
            }

            EventView::StreamStart(ev) => {
                let stream_id_changed = self
                    .sticky_events
                    .get(&(OrderedEventType(gst::EventType::StreamStart), None))
                    .is_none_or(|old_ev| {
                        let new_stream_id = ev.stream_id();
                        let old_stream_id = {
                            let gst::EventView::StreamStart(old_ev) = old_ev.view() else {
                                unreachable!();
                            };
                            old_ev.stream_id()
                        };
                        new_stream_id != old_stream_id
                    });

                self.sticky_events
                    .remove(&(OrderedEventType(gst::EventType::Eos), None));
                self.sticky_events
                    .remove(&(OrderedEventType(gst::EventType::StreamGroupDone), None));

                if stream_id_changed {
                    self.sticky_events
                        .remove(&(OrderedEventType(gst::EventType::Tag), None));
                }
            }
            EventView::FlushStop(_) => {
                self.sticky_events
                    .remove(&(OrderedEventType(gst::EventType::Eos), None));
                self.sticky_events
                    .remove(&(OrderedEventType(gst::EventType::StreamGroupDone), None));
                self.sticky_events
                    .remove(&(OrderedEventType(gst::EventType::Segment), None));

                self.pending_buffers.clear();
                self.previous_buffer = None;
            }
            _ => (),
        }

        let key = if event.is_sticky_multi() {
            (
                OrderedEventType(event.type_()),
                event.structure().map(|s| s.name().to_string()),
            )
        } else {
            (OrderedEventType(event.type_()), None)
        };
        self.sticky_events.insert(key, event.clone());

        ret
    }
}

#[derive(Default, Clone)]
struct PadSettings {
    index: u32,
    batch_strategy: super::BatchStrategy,
}

#[derive(Default)]
pub struct AnalyticsCombinerSinkPad {
    state: Mutex<PadState>,
    settings: Mutex<PadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for AnalyticsCombinerSinkPad {
    const NAME: &'static str = "GstAnalyticsCombinerSinkPad";
    type Type = super::AnalyticsCombinerSinkPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for AnalyticsCombinerSinkPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("index")
                    .nick("Index")
                    .blurb("Index, must be consecutive and starting at 0 and is fixed up otherwise")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder::<super::BatchStrategy>("batch-strategy")
                    .nick("Batch Strategy")
                    .blurb("Batching strategy to use for this stream")
                    .mutable_ready()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "index" => {
                self.settings.lock().unwrap().index = value.get().unwrap();
            }
            "batch-strategy" => {
                self.settings.lock().unwrap().batch_strategy = value.get().unwrap();
                if let Some(parent) = self
                    .obj()
                    .parent()
                    .map(|parent| parent.downcast::<super::AnalyticsCombiner>().unwrap())
                {
                    parent.imp().update_latency();
                }
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "index" => self.settings.lock().unwrap().index.to_value(),
            "batch-strategy" => self.settings.lock().unwrap().batch_strategy.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for AnalyticsCombinerSinkPad {}

impl PadImpl for AnalyticsCombinerSinkPad {}

impl AggregatorPadImpl for AnalyticsCombinerSinkPad {}
