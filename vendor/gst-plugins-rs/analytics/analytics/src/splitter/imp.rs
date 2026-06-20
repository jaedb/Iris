// SPDX-License-Identifier: MPL-2.0

use gst::{prelude::*, subclass::prelude::*};
use std::{
    mem,
    sync::{LazyLock, Mutex},
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "analyticssplitter",
        gst::DebugColorFlags::empty(),
        Some("Analytics batch / splitter element"),
    )
});

struct Stream {
    pad: gst::Pad,
    caps: Option<gst::Caps>,
}

struct State {
    generation: usize,
    streams: Vec<Stream>,
    combiner: Option<gst_base::UniqueFlowCombiner>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            generation: 0,
            streams: Vec::new(),
            combiner: Some(gst_base::UniqueFlowCombiner::default()),
        }
    }
}

pub struct AnalyticsSplitter {
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for AnalyticsSplitter {
    const NAME: &'static str = "GstAnalyticsSplitter";
    type Type = super::AnalyticsSplitter;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                AnalyticsSplitter::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |self_| self_.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                AnalyticsSplitter::catch_panic_pad_function(
                    parent,
                    || false,
                    |self_| self_.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                AnalyticsSplitter::catch_panic_pad_function(
                    parent,
                    || false,
                    |self_| self_.sink_query(pad, query),
                )
            })
            .build();

        Self {
            sinkpad,
            state: Mutex::new(State::default()),
        }
    }
}

impl ObjectImpl for AnalyticsSplitter {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj().add_pad(&self.sinkpad).unwrap();
    }
}

impl GstObjectImpl for AnalyticsSplitter {}

impl ElementImpl for AnalyticsSplitter {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Analytics batch splitter element",
                "Demuxer/Analytics",
                "Analytics batch splitter element",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder("multistream/x-analytics-batch")
                    .features([gst_analytics::CAPS_FEATURE_META_ANALYTICS_BATCH_META])
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src_%u_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let res = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let mut state_guard = self.state.lock().unwrap();
                let streams = mem::take(&mut state_guard.streams);
                *state_guard = State::default();
                drop(state_guard);

                for stream in streams {
                    let _ = self.obj().remove_pad(&stream.pad);
                }
            }
            _ => (),
        }

        Ok(res)
    }
}

impl AnalyticsSplitter {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, imp = self, "Handling buffer {buffer:?}");

        let Some(meta) = buffer.meta::<gst_analytics::AnalyticsBatchMeta>() else {
            gst::error!(CAT, imp = self, "No batch meta");
            gst::element_imp_error!(self, gst::StreamError::Demux, ["No batch meta"]);
            return Err(gst::FlowError::Error);
        };

        let mut state_guard = self.state.lock().unwrap();

        if meta.streams().len() != state_guard.streams.len() {
            gst::error!(CAT, imp = self, "Wrong number of streams");
            gst::element_imp_error!(self, gst::StreamError::Demux, ["Wrong number of streams"]);
            return Err(gst::FlowError::NotNegotiated);
        }

        let pads = state_guard
            .streams
            .iter()
            .map(|s| &s.pad)
            .cloned()
            .collect::<Vec<_>>();
        // Temporarily take combiner out so we can release the lock, and later
        // store it again in the state.
        let mut combiner = state_guard.combiner.take().unwrap();
        drop(state_guard);

        let mut res = Ok(gst::FlowSuccess::Ok);
        'next_stream: for (pad, stream) in Iterator::zip(pads.into_iter(), meta.streams().iter()) {
            for event in stream.sticky_events() {
                gst::trace!(CAT, obj = pad, "Storing sticky event {event:?}");
                let _ = pad.store_sticky_event(event);
            }

            for object in stream.objects() {
                if let Some(event) = object.downcast_ref::<gst::Event>() {
                    gst::trace!(CAT, obj = pad, "Pushing serialized event {event:?}");
                    pad.push_event(event.clone());
                } else if let Some(buffer) = object.downcast_ref::<gst::Buffer>() {
                    gst::trace!(CAT, obj = pad, "Pushing buffer {buffer:?}");
                    let pad_res = pad.push(buffer.clone());
                    res = combiner.update_pad_flow(&pad, pad_res);
                } else if let Some(buffer_list) = object.downcast_ref::<gst::BufferList>() {
                    gst::trace!(CAT, obj = pad, "Pushing buffer list {buffer_list:?}");
                    let pad_res = pad.push_list(buffer_list.clone());
                    res = combiner.update_pad_flow(&pad, pad_res);
                }

                if let Err(err) = res {
                    gst::debug!(CAT, obj = pad, "Got flow error {err:?}");
                    break 'next_stream;
                }
            }
        }

        let mut state_guard = self.state.lock().unwrap();
        state_guard.combiner = Some(combiner);

        gst::trace!(CAT, imp = self, "Returning {res:?}");

        res
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {event:?}");
        match event.view() {
            EventView::Caps(ev) => {
                let caps = ev.caps();
                let s = caps.structure(0).unwrap();

                let Some(streams) = s.get::<gst::ArrayRef>("streams").ok() else {
                    return false;
                };
                let streams = streams
                    .iter()
                    .map(|v| v.get::<Option<gst::Caps>>().unwrap())
                    .collect::<Vec<_>>();

                let mut state_guard = self.state.lock().unwrap();
                let mut to_remove = vec![];
                let mut to_add = vec![];
                if streams.len() != state_guard.streams.len()
                    || !Iterator::zip(streams.iter(), state_guard.streams.iter().map(|s| &s.caps))
                        .all(|(a, b)| a == b)
                {
                    let templ = self.obj().class().pad_template("src_%u_%u").unwrap();
                    to_remove = state_guard.streams.drain(..).map(|s| s.pad).collect();
                    for pad in &to_remove {
                        state_guard.combiner.as_mut().unwrap().remove_pad(pad);
                    }

                    state_guard.combiner.as_mut().unwrap().reset();

                    for (idx, stream) in streams.iter().enumerate() {
                        let pad = gst::Pad::builder_from_template(&templ)
                            .name(format!("src_{}_{idx}", state_guard.generation))
                            .event_function(|pad, parent, event| {
                                AnalyticsSplitter::catch_panic_pad_function(
                                    parent,
                                    || false,
                                    |self_| self_.src_event(pad, event),
                                )
                            })
                            .query_function(|pad, parent, query| {
                                AnalyticsSplitter::catch_panic_pad_function(
                                    parent,
                                    || false,
                                    |self_| self_.src_query(pad, query),
                                )
                            })
                            .build();
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Creating pad {} with caps {stream:?}",
                            pad.name()
                        );
                        to_add.push(pad.clone());
                        state_guard.combiner.as_mut().unwrap().add_pad(&pad);
                        state_guard.streams.push(Stream {
                            pad,
                            caps: stream.clone(),
                        });
                    }

                    state_guard.generation += 1;
                }

                drop(state_guard);

                for pad in to_add {
                    let _ = pad.set_active(true);
                    let _ = self.obj().add_pad(&pad);
                }
                for pad in to_remove {
                    let _ = pad.set_active(false);
                    let _ = self.obj().remove_pad(&pad);
                }

                self.obj().no_more_pads();
            }
            // Pass through
            EventView::Eos(_) | EventView::FlushStart(_) | EventView::FlushStop(_) => (),
            // Drop all other events, we take them from the individual streams
            _ => return true,
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    #[allow(clippy::single_match)]
    fn sink_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {query:?}");
        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let state_guard = self.state.lock().unwrap();
                let sinkpads = state_guard
                    .streams
                    .iter()
                    .map(|s| s.pad.clone())
                    .collect::<Vec<_>>();
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
        gst::Pad::query_default(pad, Some(&*self.obj()), query)
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {event:?}");

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {query:?}");
        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let filter = q.filter_owned();
                let peer_caps = self.sinkpad.peer_query_caps(None);

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
                        .streams
                        .iter()
                        .enumerate()
                        .find_map(|(idx, p)| (pad == &p.pad).then_some(idx));
                    drop(state_guard);

                    // Shouldn't happen, pad not found!
                    let Some(pad_index) = pad_index else {
                        gst::warning!(CAT, obj = pad, "Unknown pad");
                        return false;
                    };

                    // return expected caps for this pad from upstream, if any
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
        gst::Pad::query_default(pad, Some(&*self.obj()), query)
    }
}
