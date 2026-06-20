// Copyright (C) 2024 Igalia S.L. <aboya@igalia.com>
// Copyright (C) 2024 Comcast <aboya@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;
use std::sync::Mutex;

use gst::prelude::{ElementExt, GstObjectExt, PadExt, PadExtManual};
use gst::subclass::{prelude::*, ElementMetadata};
use gst::{glib, Caps, GroupId, Pad, PadDirection, PadPresence};

use gst::{Element, PadTemplate};
use std::sync::LazyLock;

pub static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "streamgrouper",
        gst::DebugColorFlags::empty(),
        Some("Filter element that makes all the incoming streams share a group-id"),
    )
});

#[derive(Default)]
pub struct StreamGrouper {
    pub state: Mutex<State>,
}

pub struct State {
    pub group_id: GroupId,
    pub streams_by_number: BTreeMap<usize, Stream>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            group_id: GroupId::next(),
            streams_by_number: Default::default(),
        }
    }
}

impl State {
    fn find_unused_number(&self) -> usize {
        match self.streams_by_number.keys().last() {
            Some(n) => n + 1,
            None => 0,
        }
    }

    fn get_stream_with_number(&self, number: usize) -> Option<&Stream> {
        self.streams_by_number.get(&number)
    }

    fn get_stream_with_number_or_panic(&self, number: usize) -> &Stream {
        self.get_stream_with_number(number)
            .unwrap_or_else(|| panic!("Pad is associated with stream {number} which should exist"))
    }

    fn add_stream_or_panic(&mut self, number: usize, stream: Stream) -> &Stream {
        use std::collections::btree_map::Entry::{Occupied, Vacant};
        match self.streams_by_number.entry(number) {
            Occupied(_) => panic!("Stream {number} already exists!"),
            Vacant(entry) => entry.insert(stream),
        }
    }

    fn remove_stream_or_panic(&mut self, number: usize) {
        self.streams_by_number.remove(&number).or_else(|| {
            panic!("Attempted to delete stream number {number}, which does not exist");
        });
    }
}

pub struct Stream {
    pub stream_number: usize,
    pub sinkpad: Pad,
    pub srcpad: Pad,
}

impl StreamGrouper {
    fn request_new_pad_with_number(&self, stream_number: Option<usize>) -> Option<Pad> {
        let mut state = self.state.lock().unwrap();
        let stream_number = stream_number.unwrap_or_else(|| state.find_unused_number());
        if state.get_stream_with_number(stream_number).is_some() {
            gst::error!(
                CAT,
                imp = self,
                "New pad with number {stream_number} was requested, but it already exists",
            );
            return None;
        }

        // Create the pads
        let srcpad = Pad::builder(PadDirection::Src)
            .name(format!("src_{stream_number}"))
            .query_function(move |pad, parent, query| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || false,
                    |streamgrouper| streamgrouper.src_query(pad, query, stream_number),
                )
            })
            .event_function(move |pad, parent, event| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || false,
                    |streamgrouper| streamgrouper.src_event(pad, event, stream_number),
                )
            })
            .iterate_internal_links_function(move |pad, parent| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |streamgrouper| streamgrouper.iterate_internal_links(pad, stream_number),
                )
            })
            .build();
        let sinkpad = Pad::builder(PadDirection::Sink)
            .name(format!("sink_{stream_number}"))
            .query_function(move |pad, parent, query| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || false,
                    |streamgrouper| streamgrouper.sink_query(pad, query, stream_number),
                )
            })
            .chain_function(move |pad, parent, buffer| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |streamgrouper| streamgrouper.sink_chain(pad, buffer, stream_number),
                )
            })
            .event_function(move |pad, parent, event| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || false,
                    |streamgrouper| streamgrouper.sink_event(pad, event, stream_number),
                )
            })
            .iterate_internal_links_function(move |pad, parent| {
                StreamGrouper::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |streamgrouper| streamgrouper.iterate_internal_links(pad, stream_number),
                )
            })
            .build();

        sinkpad.set_active(true).unwrap();
        srcpad.set_active(true).unwrap();

        // Add the stream
        let stream = Stream {
            stream_number,
            sinkpad: sinkpad.clone(),
            srcpad: srcpad.clone(),
        };
        state.add_stream_or_panic(stream_number, stream);

        drop(state);
        self.obj().add_pad(&srcpad).unwrap();
        self.obj().add_pad(&sinkpad).unwrap();

        Some(sinkpad)
    }

    fn src_query(
        &self,
        _srcpad: &gst::Pad,
        query: &mut gst::QueryRef,
        stream_number: usize,
    ) -> bool {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);
        let sinkpad = stream.sinkpad.clone();
        drop(state);
        sinkpad.peer_query(query) // Passthrough
    }

    fn sink_query(
        &self,
        _sinkpad: &gst::Pad,
        query: &mut gst::QueryRef,
        stream_number: usize,
    ) -> bool {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);
        let srcpad = stream.srcpad.clone();
        drop(state);
        srcpad.peer_query(query) // Passthrough
    }

    fn sink_event(&self, _sinkpad: &gst::Pad, mut event: gst::Event, stream_number: usize) -> bool {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);

        let target_group_id = state.group_id;
        let srcpad = stream.srcpad.clone();
        drop(state);

        if event.type_() != gst::EventType::StreamStart {
            return srcpad.push_event(event);
        }

        // Patch stream-start group-id
        match event.make_mut().view_mut() {
            gst::EventViewMut::StreamStart(stream_start) => {
                stream_start.set_group_id(target_group_id);
            }
            _ => unreachable!(),
        };
        srcpad.push_event(event)
    }

    fn src_event(&self, _srcpad: &gst::Pad, event: gst::Event, stream_number: usize) -> bool {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);

        let sinkpad = stream.sinkpad.clone();
        drop(state);
        sinkpad.push_event(event)
    }

    fn iterate_internal_links(
        &self,
        pad: &gst::Pad,
        stream_number: usize,
    ) -> gst::Iterator<gst::Pad> {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);

        if pad == &stream.sinkpad {
            gst::Iterator::from_vec(vec![stream.srcpad.clone()])
        } else {
            gst::Iterator::from_vec(vec![stream.sinkpad.clone()])
        }
    }

    fn sink_chain(
        &self,
        _pad: &Pad,
        buffer: gst::Buffer,
        stream_number: usize,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let state = self.state.lock().unwrap();
        let stream = state.get_stream_with_number_or_panic(stream_number);

        let srcpad = stream.srcpad.clone();
        drop(state);
        srcpad.push(buffer)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for StreamGrouper {
    const NAME: &'static str = "GstStreamGrouper";
    type Type = super::StreamGrouper;
    type ParentType = Element;
}

impl ObjectImpl for StreamGrouper {}

impl GstObjectImpl for StreamGrouper {}

impl ElementImpl for StreamGrouper {
    fn metadata() -> Option<&'static ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<ElementMetadata> = LazyLock::new(|| {
            ElementMetadata::new(
                "Stream Grouping Filter",
                "Generic",
                "Modifies all input streams to use the same group-id",
                "Alicia Boya García <aboya@igalia.com>",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if transition == gst::StateChange::PausedToReady {
            let mut state = self.state.lock().unwrap();
            let group_id = GroupId::next();
            gst::debug!(
                CAT,
                imp = self,
                "Invalidating previous group id: {:?} Next group id: {group_id:?}",
                state.group_id,
            );
            state.group_id = group_id;
        };
        self.parent_change_state(transition)
    }

    fn pad_templates() -> &'static [PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<PadTemplate>> = LazyLock::new(|| {
            // src side
            let src_pad_template = PadTemplate::new(
                "src_%u",
                PadDirection::Src,
                PadPresence::Sometimes,
                &Caps::new_any(),
            )
            .unwrap();

            // sink side
            let sink_pad_template = PadTemplate::new(
                "sink_%u",
                PadDirection::Sink,
                PadPresence::Request,
                &Caps::new_any(),
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
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        if templ.name_template() != "sink_%u" {
            gst::error!(
                CAT,
                imp = self,
                "Pad requested on extraneous template: {:?}",
                templ.name_template()
            );
            return None;
        }
        let stream_number = match name {
            None => None,
            Some(name) => {
                match name
                    .strip_prefix("sink_")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    Some(idx) => Some(idx),
                    None => {
                        gst::error!(CAT, imp = self, "Invalid pad name requested: {name:?}");
                        return None;
                    }
                }
            }
        };

        self.request_new_pad_with_number(stream_number)
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut state = self.state.lock().unwrap();
        let stream = match pad
            .name()
            .strip_prefix("sink_")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|stream_number| state.get_stream_with_number(stream_number))
        {
            Some(stream) => stream,
            None => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Requested to remove pad {}, which is not a request pad of this element",
                    pad.name()
                );
                return;
            }
        };
        let stream_number = stream.stream_number;
        let srcpad = stream.srcpad.clone();
        let sinkpad = stream.sinkpad.clone();
        state.remove_stream_or_panic(stream_number);
        drop(state);

        sinkpad.set_active(false).unwrap_or_else(|_| {
            gst::warning!(
                CAT,
                imp = self,
                "Failed to deactivate sinkpad for id {stream_number}",
            );
        });
        srcpad.set_active(false).unwrap_or_else(|_| {
            gst::warning!(
                CAT,
                imp = self,
                "Failed to deactivate srcpad for id {stream_number}",
            );
        });
        self.obj().remove_pad(&sinkpad).unwrap();
        self.obj().remove_pad(&srcpad).unwrap();
    }
}
