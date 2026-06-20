// GStreamer SMPTE ST-2038 ancillary metadata demuxer
//
// Copyright (C) 2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-st3028ancdemux
 *
 * Splits SMPTE ST-2038 ancillary metadata (as received from `tsdemux`) into separate
 * streams per DID/SDID and line/horizontal_offset.
 *
 * Will add a sometimes pad with details for each ancillary stream. Also has an
 * always source pad that just outputs all ancillary streams for easy forwarding
 * or remuxing, in case none of the ancillary streams need to be modified or
 * dropped.
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::UniqueFlowCombiner;

use atomic_refcell::AtomicRefCell;

use std::{collections::HashMap, mem, sync::LazyLock};

use crate::st2038anc_utils::AncDataHeader;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "st2038ancdemux",
        gst::DebugColorFlags::empty(),
        Some("SMPTE ST-2038 ancillary metadata demuxer"),
    )
});

pub struct St2038AncDemux {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    streams: HashMap<AncDataId, AncStream>,
    flow_combiner: UniqueFlowCombiner,
    segment: gst::FormattedSegment<gst::ClockTime>,
    last_inactivity_check: Option<gst::ClockTime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AncDataId {
    c_not_y_channel_flag: bool,
    did: u8,
    sdid: u8,
    line_number: u16,
    horizontal_offset: u16,
}

impl From<AncDataHeader> for AncDataId {
    fn from(value: AncDataHeader) -> Self {
        AncDataId {
            c_not_y_channel_flag: value.c_not_y_channel_flag,
            did: value.did,
            sdid: value.sdid,
            line_number: value.line_number,
            horizontal_offset: value.horizontal_offset,
        }
    }
}

struct AncStream {
    pad: gst::Pad,
    last_used: Option<gst::ClockTime>,
}

impl St2038AncDemux {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        gst::trace!(CAT, imp = self, "Handling buffer {buffer:?}");

        let ts = buffer.dts_or_pts();
        let running_time = state.segment.to_running_time(ts);

        let Ok(map) = buffer.map_readable() else {
            gst::error!(CAT, imp = self, "Failed to map buffer",);

            // Just push it out on the combined pad and be done with it
            drop(state);
            let res = self.srcpad.push(buffer);
            state = self.state.borrow_mut();

            return state.flow_combiner.update_pad_flow(&self.srcpad, res);
        };

        let mut slice = map.as_slice();

        while !slice.is_empty() {
            // Stop on stuffing bytes
            if slice[0] == 0b1111_1111 {
                break;
            }

            let start_offset = map.len() - slice.len();
            let anc_hdr = match AncDataHeader::from_slice(slice) {
                Ok(anc_hdr) => anc_hdr,
                Err(err) => {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Failed to parse ancillary data header: {err:?}"
                    );
                    break;
                }
            };
            let end_offset = start_offset + anc_hdr.len;

            gst::trace!(CAT, imp = self, "Parsed ST2038 header {anc_hdr:?}");

            let anc_id = AncDataId::from(anc_hdr);

            let stream = match state.streams.get_mut(&anc_id) {
                Some(stream) => stream,
                None => {
                    let pad_name = format!(
                        "anc_{:02x}_{:02x}_at_{}_{}",
                        anc_id.did, anc_id.sdid, anc_id.line_number, anc_id.horizontal_offset
                    );

                    gst::info!(
                        CAT,
                        imp = self,
                        "New ancillary data stream {pad_name}: {anc_hdr:?}"
                    );

                    let anc_templ = self.obj().pad_template("anc_%02x_%02x_at_%u_%u").unwrap();
                    let anc_srcpad = gst::Pad::builder_from_template(&anc_templ)
                        .name(pad_name)
                        .build();

                    anc_srcpad.set_active(true).expect("set pad active");

                    // Forward sticky events from main source pad to new ancillary data source pad
                    // FIXME: do we want/need to modify the stream id here? caps?
                    self.srcpad.sticky_events_foreach(|event| {
                        let _ = anc_srcpad.store_sticky_event(event);
                        std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
                    });

                    drop(state);
                    self.obj().add_pad(&anc_srcpad).expect("add pad");
                    state = self.state.borrow_mut();

                    state.flow_combiner.add_pad(&anc_srcpad);

                    state.streams.insert(
                        anc_id,
                        AncStream {
                            pad: anc_srcpad,
                            last_used: running_time,
                        },
                    );

                    state.streams.get_mut(&anc_id).expect("stream")
                }
            };

            if let Some(running_time) = running_time {
                if stream
                    .last_used
                    .is_none_or(|last_used| last_used < running_time)
                {
                    stream.last_used = Some(running_time);
                }
            }

            let Ok(mut sub_buffer) =
                buffer.copy_region(gst::BufferCopyFlags::MEMORY, start_offset..end_offset)
            else {
                gst::error!(CAT, imp = self, "Failed to create sub-buffer");
                break;
            };
            {
                let sub_buffer = sub_buffer.make_mut();
                let _ = buffer.copy_into(sub_buffer, gst::BUFFER_COPY_METADATA, ..);
            }

            let anc_pad = stream.pad.clone();

            drop(state);
            let anc_flow = anc_pad.push(sub_buffer.clone());
            state = self.state.borrow_mut();

            state.flow_combiner.update_pad_flow(&anc_pad, anc_flow)?;

            // TODO: Check every now and then if any ancillary streams haven't seen any data for a while
            if let Some((last_check, rt)) = Option::zip(state.last_inactivity_check, running_time) {
                if gst::ClockTime::absdiff(rt, last_check) >= gst::ClockTime::from_seconds(10) {
                    // gst::fixme!(CAT, imp = self, "Check ancillary streams for inactivity");
                    state.last_inactivity_check = running_time;
                }
            }

            let mut late_pads = Vec::new();
            if let Some(running_time) = running_time {
                let ts = ts.unwrap();

                let State {
                    ref mut streams,
                    ref segment,
                    ..
                } = &mut *state;

                for stream in streams.values_mut() {
                    let Some(last_used) = stream.last_used else {
                        continue;
                    };

                    if gst::ClockTime::absdiff(last_used, running_time)
                        > gst::ClockTime::from_mseconds(500)
                    {
                        let Some(timestamp) = segment.position_from_running_time_full(last_used)
                        else {
                            continue;
                        };

                        let timestamp = timestamp.positive().unwrap_or(ts);
                        let duration = ts.checked_sub(timestamp);

                        gst::trace!(
                            CAT,
                            obj = stream.pad,
                            "Advancing late stream from {last_used} to {running_time}"
                        );

                        late_pads.push((
                            stream.pad.clone(),
                            gst::event::Gap::builder(timestamp)
                                .duration(duration)
                                .build(),
                        ));
                        stream.last_used = Some(running_time);
                    }
                }
            }

            drop(state);
            let main_flow = self.srcpad.push(sub_buffer);
            state = self.state.borrow_mut();

            state
                .flow_combiner
                .update_pad_flow(&self.srcpad, main_flow)?;

            for (pad, event) in late_pads {
                let _ = pad.push_event(event);
            }

            slice = &slice[anc_hdr.len..];
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        // Todo: clear last_seen times on ancillary src pads on stream start/flush?
        match event.view() {
            EventView::StreamStart(_) => {
                let mut state = self.state.borrow_mut();
                state.last_inactivity_check = gst::ClockTime::ZERO.into();
            }
            EventView::Segment(ev) => {
                let mut state = self.state.borrow_mut();
                state.segment = ev
                    .segment()
                    .clone()
                    .downcast::<gst::format::Time>()
                    .unwrap();
            }
            EventView::Caps(ev) => {
                let (alignment, framerate) = {
                    let caps = ev.caps();
                    let s = caps.structure(0).unwrap();

                    (
                        s.get::<&str>("alignment").ok(),
                        s.get::<gst::Fraction>("framerate").ok(),
                    )
                };

                // Don't forward the caps event directly but set the
                // alignment and frame rate.
                let src_caps = {
                    let mut caps = ev.caps_owned();
                    let caps = caps.make_mut();

                    if let Some(align) = alignment {
                        caps.set("alignment", align);
                    }
                    if let Some(framerate) = framerate {
                        caps.set("framerate", framerate);
                    }

                    caps.to_owned()
                };

                let src_event = gst::event::Caps::builder(&src_caps)
                    .seqnum(event.seqnum())
                    .build();

                let mut ret = self.srcpad.push_event(src_event.clone());

                let state = self.state.borrow_mut();
                let anc_pads = state
                    .streams
                    .values()
                    .map(|stream| stream.pad.clone())
                    .collect::<Vec<_>>();
                drop(state);

                for pad in anc_pads {
                    let mut templ_caps = pad.pad_template_caps();
                    let mut peer_caps = pad.peer_query_caps(Some(&templ_caps));
                    gst::debug!(CAT, obj = pad, "Downstream caps {peer_caps:?}");

                    let anc_caps = {
                        let caps_ref = templ_caps.make_mut();
                        if peer_caps.is_empty() {
                            caps_ref.set("alignment", "frame");
                        } else {
                            peer_caps.fixate();

                            let s = peer_caps.structure(0).unwrap();
                            match s.get::<&str>("alignment").ok() {
                                Some(align) => caps_ref.set("alignment", align),
                                None => caps_ref.set("alignment", "frame"),
                            }
                        }

                        templ_caps
                    };

                    let anc_event = gst::event::Caps::builder(&anc_caps)
                        .seqnum(event.seqnum())
                        .build();
                    ret |= pad.push_event(anc_event.clone());
                }

                return ret;
            }
            EventView::FlushStop(_) => {
                let mut state = self.state.borrow_mut();
                state.segment = gst::FormattedSegment::default();
                state.last_inactivity_check = None;
                state.flow_combiner.reset();
            }
            _ => {}
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}

impl GstObjectImpl for St2038AncDemux {}

impl ElementImpl for St2038AncDemux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "SMPTE ST-2038 ancillary metadata demuxer",
                "Metadata/Video/Demuxer",
                "Splits individual ancillary metadata streams from an SMPTE ST-2038 stream",
                "Tim-Philipp Müller <tim centricular com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("meta/x-st-2038").build();
            let caps_aligned = gst::Caps::builder("meta/x-st-2038")
                .field("alignment", "packet")
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            // One always pad that outputs the combined stream, and sometimes pads for each
            // ancillary data type, so people can only splice off the ones they actually need.
            let combined_src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps_aligned,
            )
            .unwrap();

            let individual_src_pad_template = gst::PadTemplate::new(
                "anc_%02x_%02x_at_%u_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::builder("meta/x-st-2038")
                    .field("alignment", gst::List::new(["frame", "line", "packet"]))
                    .build(),
            )
            .unwrap();

            vec![
                sink_pad_template,
                combined_src_pad_template,
                individual_src_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.borrow_mut();
                *state = State::default();
                state.flow_combiner.add_pad(&self.srcpad);
            }
            _ => (),
        }

        let res = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let old_state = mem::take(&mut *self.state.borrow_mut());
                for (_anc_id, stream) in old_state.streams {
                    let _ = self.obj().remove_pad(&stream.pad);
                }
            }
            _ => (),
        }

        Ok(res)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for St2038AncDemux {
    const NAME: &'static str = "GstSt2038AncDemux";
    type Type = super::St2038AncDemux;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                St2038AncDemux::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |enc| enc.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                St2038AncDemux::catch_panic_pad_function(
                    parent,
                    || false,
                    |enc| enc.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::from_template(&templ);

        Self {
            srcpad,
            sinkpad,
            state: State::default().into(),
        }
    }
}

impl ObjectImpl for St2038AncDemux {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}
