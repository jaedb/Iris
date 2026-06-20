// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::common::*;
use crate::quinnquicmeta::QuinnQuicMeta;
use crate::quinnquicquery::*;
use gst::{glib, prelude::*, subclass::prelude::*};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnquicdemux",
        gst::DebugColorFlags::empty(),
        Some("Quinn QUIC Demux"),
    )
});

#[derive(Default)]
struct Started {
    pads_map: HashMap<u64, gst::Pad>,
    datagram_pad_added: bool,
}

#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started(Started),
}

pub struct QuinnQuicDemux {
    state: Mutex<State>,
    sinkpad: gst::Pad,
    datagram_pad: gst::Pad,
}

impl GstObjectImpl for QuinnQuicDemux {}

impl ElementImpl for QuinnQuicDemux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn QUIC De-multiplexer",
                "Source/Network/QUIC",
                "Demultiplexes multiple streams and datagram for QUIC",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "stream_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let datagram_pad_template = gst::PadTemplate::new(
                "datagram",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![datagram_pad_template, src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let ret = self.parent_change_state(transition)?;

        if let gst::StateChange::NullToReady = transition {
            let mut state = self.state.lock().unwrap();
            *state = State::Started(Started::default());
        }

        Ok(ret)
    }
}

impl ObjectImpl for QuinnQuicDemux {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj()
            .add_pad(&self.sinkpad)
            .expect("Failed to add sink pad");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnQuicDemux {
    const NAME: &'static str = "GstQuinnQuicDemux";
    type Type = super::QuinnQuicDemux;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let sinkpad = gst::Pad::builder_from_template(&klass.pad_template("sink").unwrap())
            .chain_function(|_pad, parent, buffer| {
                QuinnQuicDemux::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |demux| demux.sink_chain(buffer),
                )
            })
            .event_function(|pad, parent, event| {
                QuinnQuicDemux::catch_panic_pad_function(
                    parent,
                    || false,
                    |demux| demux.sink_event(pad, event),
                )
            })
            .build();

        let datagram_pad =
            gst::Pad::builder_from_template(&klass.pad_template("datagram").unwrap()).build();

        Self {
            state: Mutex::new(State::default()),
            sinkpad,
            datagram_pad,
        }
    }
}

impl QuinnQuicDemux {
    fn handle_datagram(&self, buffer: gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut started) = *state {
            if !started.datagram_pad_added {
                self.datagram_pad.set_active(true).unwrap();

                let stream_start_evt = gst::event::StreamStart::builder(&0.to_string())
                    .group_id(gst::GroupId::next())
                    .build();
                self.datagram_pad.push_event(stream_start_evt);

                let segment_evt =
                    gst::event::Segment::new(&gst::FormattedSegment::<gst::ClockTime>::new());
                self.datagram_pad.push_event(segment_evt);

                self.obj()
                    .add_pad(&self.datagram_pad)
                    .expect("Failed to add datagram pad");

                started.datagram_pad_added = true;
            }

            gst::trace!(CAT, imp = self, "Pushing datagram buffer: {buffer:?}");

            return self.datagram_pad.push(buffer);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn handle_stream(
        &self,
        buffer: gst::Buffer,
        stream_id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut started) = *state {
            match started.pads_map.get(&stream_id) {
                Some(pad) => {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Pushing buffer: {buffer:?} with stream_id {stream_id}"
                    );
                    return pad.push(buffer);
                }
                None => {
                    let templ = self
                        .obj()
                        .element_class()
                        .pad_template("stream_%u")
                        .unwrap();
                    let stream_pad_name = format!("stream_{stream_id}");

                    let srcpad = gst::Pad::builder_from_template(&templ)
                        .name(stream_pad_name.clone())
                        .build();

                    srcpad.set_active(true).unwrap();

                    let stream_start_evt = gst::event::StreamStart::builder(&stream_id.to_string())
                        .group_id(gst::GroupId::next())
                        .build();
                    srcpad.push_event(stream_start_evt);

                    let segment_evt =
                        gst::event::Segment::new(&gst::FormattedSegment::<gst::ClockTime>::new());
                    srcpad.push_event(segment_evt);

                    self.obj().add_pad(&srcpad).expect("Failed to add pad");

                    gst::info!(
                        CAT,
                        imp = self,
                        "Added pad {stream_pad_name} for stream id {stream_id}"
                    );

                    started.pads_map.insert(stream_id, srcpad.clone());

                    return srcpad.push(buffer);
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn remove_pad(&self, stream_id: u64) -> bool {
        gst::debug!(CAT, imp = self, "Removing pad for stream id {stream_id}");

        let mut state = self.state.lock().unwrap();
        let stream_pad = if let State::Started(ref mut state) = *state {
            state.pads_map.remove(&stream_id)
        } else {
            None
        };
        drop(state);

        if let Some(pad) = stream_pad {
            let _ = pad.set_active(false);

            if let Err(err) = self.obj().remove_pad(&pad) {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to remove pad {} for stream id {stream_id}, error: {err:?}",
                    pad.name()
                );
                return false;
            } else {
                gst::log!(
                    CAT,
                    imp = self,
                    "Pad {} removed for stream id {stream_id}",
                    pad.name()
                );
                return true;
            }
        }

        false
    }

    fn sink_chain(&self, buffer: gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let meta = buffer.meta::<QuinnQuicMeta>();

        match meta {
            Some(m) => {
                if m.is_datagram() {
                    self.handle_datagram(buffer)
                } else {
                    let stream_id = m.stream_id();

                    self.handle_stream(buffer, stream_id)
                }
            }
            None => {
                gst::warning!(CAT, imp = self, "Buffer dropped, no metadata");
                Ok(gst::FlowSuccess::Ok)
            }
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling event {:?}", event);

        if let EventView::CustomDownstream(ev) = event.view() {
            if let Some(s) = ev.structure() {
                if s.name() == QUIC_STREAM_CLOSE_CUSTOMDOWNSTREAM_EVENT {
                    if let Ok(stream_id) = s.get::<u64>(QUIC_STREAM_ID) {
                        return self.remove_pad(stream_id);
                    }
                }
            }
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}
