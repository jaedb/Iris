// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib::SignalHandlerId;
use gst::glib::{self, GString};
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::MutexGuard;
use std::{
    mem,
    sync::{Mutex, OnceLock},
};

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "fallbacksrc-custom-source",
        gst::DebugColorFlags::empty(),
        Some("Fallback Custom Source Bin"),
    )
});

struct Stream {
    source_pad: gst::Pad,
    ghost_pad: gst::GhostPad,
    stream: gst::Stream,

    // Used if source isn't stream-aware and we're handling the stream selection manually
    is_selected: bool,
}

impl Stream {
    // If source isn't stream-aware, we expose pads only after no-more-pads and READY->PAUSED
    fn is_exposed(&self) -> bool {
        self.ghost_pad.parent().is_some()
    }
}

#[derive(Default)]
struct State {
    stream_id_prefix: String,

    received_collection: Option<gst::StreamCollection>,
    pads: Vec<Stream>,
    num_audio: usize,
    num_video: usize,
    pad_added_sig_id: Option<SignalHandlerId>,
    pad_removed_sig_id: Option<SignalHandlerId>,
    no_more_pads_sig_id: Option<SignalHandlerId>,
    selection_seqnum: Option<gst::Seqnum>,

    // Signals either:
    // - ready->paused to post collection after no-more-pads was already called
    // - or no-more-pads to post collection because we're after ready->paused
    should_post_collection: bool,
}

impl State {
    /// If source sent us a collection, it's stream-aware
    /// and we can just forward the collection and selection events
    fn is_passthrough(&self) -> bool {
        self.received_collection.is_some()
    }

    fn our_collection(&self) -> gst::StreamCollection {
        let streams = self.pads.iter().map(|p| p.stream.clone());
        gst::StreamCollection::builder(None)
            .streams(streams)
            .build()
    }
}

#[derive(Default)]
pub struct CustomSource {
    source: OnceLock<gst::Element>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for CustomSource {
    const NAME: &'static str = "GstFallbackSrcCustomSource";
    type Type = super::CustomSource;
    type ParentType = gst::Bin;
}

impl ObjectImpl for CustomSource {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecObject::builder::<gst::Element>("source")
                .nick("Source")
                .blurb("Source")
                .write_only()
                .construct_only()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "source" => {
                let source = value.get::<gst::Element>().unwrap();
                self.source.set(source.clone()).unwrap();
                self.obj().add(&source).unwrap();
            }
            _ => unreachable!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SOURCE | gst::ElementFlags::SINK);
        obj.set_element_flags(gst::ElementFlags::SOURCE);
        obj.set_bin_flags(gst::BinFlags::STREAMS_AWARE);
    }
}

impl GstObjectImpl for CustomSource {}

impl ElementImpl for CustomSource {
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let audio_src_pad_template = gst::PadTemplate::new(
                "audio_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let video_src_pad_template = gst::PadTemplate::new(
                "video_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![audio_src_pad_template, video_src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                self.start()?;
            }
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.lock().unwrap();
                if !state.is_passthrough() {
                    if state.should_post_collection {
                        self.post_collection(state);
                    } else {
                        // Tells no-more-pads handler it can post the collection right away
                        state.should_post_collection = true;
                    }
                }
            }
            _ => (),
        }

        let res = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToNull | gst::StateChange::NullToNull => {
                self.stop();
            }
            _ => (),
        }

        Ok(res)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::SelectStreams(e) => {
                if self.state.lock().unwrap().is_passthrough() {
                    gst::debug!(CAT, imp = self, "Forwarding select streams event to source");
                    let streams = e.streams();
                    let event =
                        gst::event::SelectStreams::builder(streams.iter().map(|s| s.as_str()))
                            .build();
                    return self.source.get().unwrap().send_event(event);
                }

                gst::debug!(CAT, imp = self, "Handling select streams event");

                let stream_ids = e
                    .streams()
                    .into_iter()
                    .map(glib::GString::from)
                    .collect::<Vec<_>>();

                if let Some(message) = self.handle_stream_selection(stream_ids) {
                    let mut state = self.state.lock().unwrap();
                    state.selection_seqnum = Some(e.seqnum());
                    drop(state);

                    if let Err(err) = self.obj().post_message(message) {
                        gst::warning!(CAT, imp = self, "Failed to post message: {}", err);
                    }
                    return true;
                }

                false
            }
            _ => true,
        }
    }
}

impl BinImpl for CustomSource {
    #[allow(clippy::single_match)]
    fn handle_message(&self, msg: gst::Message) {
        use gst::MessageView;

        match msg.view() {
            MessageView::StreamCollection(collection) => {
                // Receiving a stream collection indicates we can be in passthrough mode
                // Otherwise if no collection is received, we generate our own one and handle selection etc.

                gst::debug!(
                    CAT,
                    imp = self,
                    "Forwarding stream collection message from source: {:?}",
                    collection.stream_collection()
                );

                let mut state = self.state.lock().unwrap();
                state.received_collection = Some(collection.stream_collection().clone());
                drop(state);

                let message =
                    gst::message::StreamCollection::builder(&collection.stream_collection())
                        .src(&*self.obj())
                        .build();

                if let Err(err) = self.obj().post_message(message) {
                    gst::warning!(CAT, imp = self, "Failed to post message: {}", err);
                }
            }
            MessageView::StreamsSelected(selected) => {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Forwarding streams-selected from source: {:?}",
                    selected.streams()
                );

                let message = gst::message::StreamsSelected::builder(&selected.stream_collection())
                    .streams(selected.streams())
                    .src(&*self.obj())
                    .build();

                if let Err(err) = self.obj().post_message(message) {
                    gst::warning!(CAT, imp = self, "Failed to post message: {}", err);
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}

impl CustomSource {
    fn start(&self) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::debug!(CAT, imp = self, "Starting");
        let source = self.source.get().unwrap();

        let mut state = self.state.lock().unwrap();
        state.stream_id_prefix = format!("{:016x}", rand::random::<u64>());
        drop(state);

        let templates = source.pad_template_list();

        if templates
            .iter()
            .any(|templ| templ.presence() == gst::PadPresence::Request)
        {
            gst::error!(CAT, imp = self, "Request pads not supported");
            gst::element_imp_error!(
                self,
                gst::LibraryError::Settings,
                ["Request pads not supported"]
            );
            return Err(gst::StateChangeError);
        }

        let has_sometimes_pads = templates
            .iter()
            .any(|templ| templ.presence() == gst::PadPresence::Sometimes);

        // Handle all source pads that already exist
        for pad in source.src_pads() {
            if let Err(msg) = self.handle_source_pad_added(&pad) {
                self.post_error_message(msg);
                return Err(gst::StateChangeError);
            }
        }

        if !has_sometimes_pads {
            self.handle_source_no_more_pads();
        } else {
            gst::debug!(CAT, imp = self, "Found sometimes pads");

            let pad_added_sig_id = source.connect_pad_added(move |source, pad| {
                let element = match source
                    .parent()
                    .and_then(|p| p.downcast::<super::CustomSource>().ok())
                {
                    Some(element) => element,
                    None => return,
                };
                let src = element.imp();

                if let Err(msg) = src.handle_source_pad_added(pad) {
                    element.post_error_message(msg);
                }
            });
            let pad_removed_sig_id = source.connect_pad_removed(move |source, pad| {
                let element = match source
                    .parent()
                    .and_then(|p| p.downcast::<super::CustomSource>().ok())
                {
                    Some(element) => element,
                    None => return,
                };
                let src = element.imp();

                src.handle_source_pad_removed(pad);
            });

            let no_more_pads_sig_id = source.connect_no_more_pads(move |source| {
                let element = match source
                    .parent()
                    .and_then(|p| p.downcast::<super::CustomSource>().ok())
                {
                    Some(element) => element,
                    None => return,
                };
                let src = element.imp();

                src.handle_source_no_more_pads();
            });

            let mut state = self.state.lock().unwrap();
            state.pad_added_sig_id = Some(pad_added_sig_id);
            state.pad_removed_sig_id = Some(pad_removed_sig_id);
            state.no_more_pads_sig_id = Some(no_more_pads_sig_id);
        }

        Ok(gst::StateChangeSuccess::Success)
    }

    fn handle_source_pad_added(&self, pad: &gst::Pad) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Source added pad {}", pad.name());

        let mut state = self.state.lock().unwrap();

        let (mut stream_type, mut stream_id) = (None, None);

        // Take stream type from stream-start event if we can
        if let Some(ev) = pad.sticky_event::<gst::event::StreamStart>(0) {
            stream_type = ev.stream().map(|s| s.stream_type());
            stream_id = ev.stream().and_then(|s| s.stream_id());
        }

        // Otherwise from the caps
        if stream_type.is_none() {
            let caps = match pad.current_caps().unwrap_or_else(|| pad.query_caps(None)) {
                caps if !caps.is_any() && !caps.is_empty() => caps,
                _ => {
                    gst::error!(CAT, imp = self, "Pad {} had no caps", pad.name());
                    return Err(gst::error_msg!(
                        gst::CoreError::Negotiation,
                        ["Pad had no caps"]
                    ));
                }
            };

            let s = caps.structure(0).unwrap();

            if s.name().starts_with("audio/") {
                stream_type = Some(gst::StreamType::AUDIO);
            } else if s.name().starts_with("video/") {
                stream_type = Some(gst::StreamType::VIDEO);
            } else {
                return Ok(());
            }
        }

        let stream_type = stream_type.unwrap();

        let (templ, name) = if stream_type.contains(gst::StreamType::AUDIO) {
            let name = format!("audio_{}", state.num_audio);
            state.num_audio += 1;
            (self.obj().pad_template("audio_%u").unwrap(), name)
        } else {
            let name = format!("video_{}", state.num_video);
            state.num_video += 1;
            (self.obj().pad_template("video_%u").unwrap(), name)
        };

        let ghost_pad = gst::GhostPad::builder_from_template_with_target(&templ, pad)
            .unwrap()
            .name(name)
            .build();
        ghost_pad.set_active(true).unwrap();

        // If source posted a stream collection, we can(?) assume that the stream has an ID
        // Otherwise we create our own simple collection
        if !state.is_passthrough() {
            stream_id = if stream_type.contains(gst::StreamType::AUDIO) {
                Some(format!("{}/audio/{}", state.stream_id_prefix, state.num_audio - 1).into())
            } else {
                Some(format!("{}/video/{}", state.stream_id_prefix, state.num_video - 1).into())
            };
        } else {
            assert!(stream_id.is_some());
        }

        let expose_pad = state.is_passthrough();
        let gst_stream = gst::Stream::new(
            Some(stream_id.as_ref().unwrap()),
            None,
            stream_type,
            gst::StreamFlags::empty(),
        );
        let stream = Stream {
            source_pad: pad.clone(),
            ghost_pad: ghost_pad.clone().upcast(),
            stream: gst_stream.clone(),
            is_selected: true,
        };

        state.pads.push(stream);
        drop(state);

        if expose_pad {
            let stream_start_event = gst::event::StreamStart::builder(&stream_id.unwrap())
                .stream(gst_stream)
                .build();
            ghost_pad.store_sticky_event(&stream_start_event).unwrap();
            self.obj().add_pad(&ghost_pad).unwrap();
        }

        Ok(())
    }

    fn handle_source_pad_removed(&self, pad: &gst::Pad) {
        gst::debug!(CAT, imp = self, "Source removed pad {}", pad.name());

        let mut state = self.state.lock().unwrap();
        let (i, stream) = match state
            .pads
            .iter()
            .enumerate()
            .find(|(_i, p)| &p.source_pad == pad)
        {
            None => return,
            Some(v) => v,
        };

        // If we're in streams-aware mode (have a collection from source)
        // then this is fine, probably happens because streams were de-selected.
        // Otherwise if the source is not stream-aware, this means the stream disappeared
        // and we need to remove it from our proxy collection.

        let (ghost_pad, is_exposed) = (stream.ghost_pad.clone(), stream.is_exposed());
        state.pads.remove(i);

        if !state.is_passthrough() {
            let our_collection = state.our_collection();
            let our_seqnum = gst::Seqnum::next();
            state.selection_seqnum = Some(our_seqnum);
            drop(state);

            let _ = self.obj().post_message(
                gst::message::StreamsSelected::builder(&our_collection)
                    .src(&*self.obj())
                    .build(),
            );

            let state = self.state.lock().unwrap();
            if state.selection_seqnum == Some(our_seqnum) {
                let selected_ids = state
                    .pads
                    .iter()
                    .filter(|p| p.is_selected)
                    .map(|p| p.stream.stream_id().unwrap())
                    .collect::<Vec<_>>();
                drop(state);
                if let Some(message) = self.handle_stream_selection(selected_ids) {
                    let _ = self.obj().post_message(message);
                }
            }
        } else {
            drop(state);
        }

        if is_exposed {
            ghost_pad.set_active(false).unwrap();
            let _ = ghost_pad.set_target(None::<&gst::Pad>);
            let _ = self.obj().remove_pad(&ghost_pad);
        }
    }

    fn handle_source_no_more_pads(&self) {
        gst::debug!(CAT, imp = self, "Source signalled no-more-pads");

        let mut state = self.state.lock().unwrap();

        // Make sure this isn't happening if a source posted a stream collection
        assert!(!state.is_passthrough());

        // Tells ready->paused handler to post collection and handle selection there
        if !state.should_post_collection {
            state.should_post_collection = true;
            return;
        }

        self.post_collection(state);
    }

    fn post_collection(&self, mut state: MutexGuard<State>) {
        let collection = state.our_collection();
        let our_seqnum = gst::Seqnum::next();
        state.selection_seqnum = Some(our_seqnum);
        state.should_post_collection = false;
        drop(state);

        let _ = self.obj().post_message(
            gst::message::StreamCollection::builder(&collection)
                .src(&*self.obj())
                .build(),
        );

        let state = self.state.lock().unwrap();
        if state.selection_seqnum == Some(our_seqnum) {
            // Exposes all available pads by default
            let selected_ids = state
                .pads
                .iter()
                .map(|p| p.stream.stream_id().unwrap())
                .collect::<Vec<_>>();
            drop(state);
            if let Some(message) = self.handle_stream_selection(selected_ids) {
                let _ = self.obj().post_message(message);
            }
        }
    }

    fn handle_stream_selection(&self, stream_ids: Vec<GString>) -> Option<gst::Message> {
        let mut state_guard = self.state.lock().unwrap();
        let state = &mut *state_guard;

        for id in stream_ids.iter() {
            if !state
                .pads
                .iter()
                .any(|p| p.stream.stream_id().unwrap() == *id)
            {
                gst::error!(CAT, imp = self, "Stream with ID {} not found!", id);
                return None;
            }
        }

        let mut selected_streams = vec![];
        for stream in state.pads.iter_mut() {
            if stream_ids.contains(&stream.stream.stream_id().unwrap()) {
                stream.is_selected = true;
                selected_streams.push(stream.stream.clone());
                gst::log!(
                    CAT,
                    imp = self,
                    "Stream {} selected",
                    stream.stream.stream_id().unwrap()
                );
            } else {
                stream.is_selected = false;
                gst::log!(
                    CAT,
                    imp = self,
                    "Stream {} not selected",
                    stream.stream.stream_id().unwrap()
                );
            }
        }

        let our_collection = state.our_collection();
        let message = gst::message::StreamsSelected::builder(&our_collection)
            .streams(selected_streams)
            .src(&*self.obj())
            .build();

        self.expose_only_selected_streams(state_guard);
        Some(message)
    }

    fn expose_only_selected_streams(&self, state: MutexGuard<State>) {
        let mut to_add = vec![];
        let mut to_remove = vec![];

        for stream in state.pads.iter() {
            if stream.is_selected && !stream.is_exposed() {
                let event = gst::event::StreamStart::builder(&stream.stream.stream_id().unwrap())
                    .stream(stream.stream.clone())
                    .build();
                stream.ghost_pad.store_sticky_event(&event).unwrap();
                to_add.push(stream.ghost_pad.clone());
            } else if !stream.is_selected && stream.is_exposed() {
                let _ = stream.ghost_pad.set_target(None::<&gst::Pad>);
                to_remove.push(stream.ghost_pad.clone());
            }
        }
        drop(state);

        for pad in to_add {
            self.obj().add_pad(&pad).unwrap();
        }

        for pad in to_remove {
            let _ = self.obj().remove_pad(&pad);
        }
    }

    fn stop(&self) {
        gst::debug!(CAT, imp = self, "Stopping");

        let mut state = self.state.lock().unwrap();
        let source = self.source.get().unwrap();
        if let Some(id) = state.pad_added_sig_id.take() {
            source.disconnect(id)
        }

        if let Some(id) = state.pad_removed_sig_id.take() {
            source.disconnect(id)
        }

        if let Some(id) = state.no_more_pads_sig_id.take() {
            source.disconnect(id)
        }

        let pads = mem::take(&mut state.pads);
        state.num_audio = 0;
        state.num_video = 0;
        state.received_collection = None;
        drop(state);

        for pad in pads.iter().filter(|s| s.is_exposed()) {
            let _ = pad.ghost_pad.set_target(None::<&gst::Pad>);
            let _ = self.obj().remove_pad(&pad.ghost_pad);
        }
    }
}
