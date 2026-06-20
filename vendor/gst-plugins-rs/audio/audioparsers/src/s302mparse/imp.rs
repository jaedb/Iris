// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MPL-2.0

use gst::{glib, prelude::*, subclass::prelude::*};

use std::{
    collections::VecDeque,
    mem,
    sync::{LazyLock, Mutex},
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "s302mparse",
        gst::DebugColorFlags::empty(),
        Some("S302M parser"),
    )
});

pub struct S302MParse {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

struct State {
    discont: bool,
    channels: Option<u8>,
    depth: Option<u8>,
    pending_events: VecDeque<gst::Event>,
    last_pts: Option<gst::ClockTime>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            discont: true,
            channels: None,
            depth: None,
            pending_events: VecDeque::default(),
            last_pts: None,
        }
    }
}

impl S302MParse {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let Ok(map) = buffer.map_readable() else {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");
            return Err(gst::FlowError::Error);
        };

        if map.size() < 4 {
            gst::warning!(CAT, obj = pad, "Buffer smaller than AES3 header");
            drop(map);
            state.discont = true;
            return Ok(gst::FlowSuccess::Ok);
        }

        // See S302M-2007 Table 1
        //
        // - 16 bit audio packet size (excluding 4 byte header)
        // - 2 bits number of channels (2, 4, 6, 8)
        // - 8 bits channel identification
        // - 2 bits bits-per-sample (16, 20, 24, reserved)
        // - 4 bits alignment/reserved
        let header = u32::from_be_bytes([map[0], map[1], map[2], map[3]]);
        let audio_packet_size = (header >> 16) as usize;
        let number_channels = match (header >> 14) & 0b11 {
            0b00 => 2,
            0b01 => 4,
            0b10 => 6,
            0b11 => 8,
            _ => unreachable!(),
        };
        let channel_identification = (header >> 8) & 0xff;
        let bits_per_sample = match (header >> 4) & 0b11 {
            0b00 => 16,
            0b01 => 20,
            0b10 => 24,
            0b11 => {
                gst::warning!(CAT, obj = pad, "Invalid bits-per-sample in AES3 header");
                drop(map);
                state.discont = true;
                return Ok(gst::FlowSuccess::Ok);
            }
            _ => unreachable!(),
        };
        let alignment_bits = header & 0b1111;
        if alignment_bits != 0 {
            gst::warning!(CAT, obj = pad, "Invalid alignment-bits in AES3 header");
        }

        gst::trace!(
            CAT,
            obj = pad,
            "AES3 header: audio_packet_size {audio_packet_size}, \
                number_channels {number_channels}, \
                channel_identification {channel_identification}, \
                bits_per_sample {bits_per_sample}",
        );

        // We require properly packetized packets from upstream
        if map.size() != 4 + audio_packet_size {
            gst::warning!(
                CAT,
                obj = pad,
                "Dropping short AES3 packet of wrong size: got {}, expected {}",
                map.size(),
                4 + audio_packet_size,
            );
            drop(map);
            state.discont = true;
            return Ok(gst::FlowSuccess::Ok);
        }
        drop(map);

        // Calculate number of samples
        //
        // See S302M-2007 Section 5
        let block_size = (bits_per_sample as usize + 4) / 4;
        let num_samples = 2 * audio_packet_size / (block_size * number_channels as usize);
        let duration = gst::ClockTime::SECOND
            .mul_div_ceil(num_samples as u64, 48_000)
            .unwrap();

        // Interpolate timestamp/duration as needed and set DISCONT flag
        if state.discont
            || (buffer.pts().is_none() && state.last_pts.is_some())
            || buffer.duration().is_none()
        {
            let buffer = buffer.make_mut();

            // Interpolate PTS
            if let Some(last_pts) = state.last_pts {
                if !state.discont && buffer.pts().is_none() {
                    buffer.set_pts(last_pts);
                }
            }

            // Calculate duration
            if buffer.duration().is_none() {
                buffer.set_duration(duration);
            }

            if state.discont {
                buffer.set_flags(gst::BufferFlags::DISCONT);
                state.discont = false;
            }
        }

        // Update last PTS. Buffer PTS might be the interpolated one from above which is then
        // advanced here to the expected start PTS of the next buffer.
        if let Some(pts) = buffer.pts() {
            state.last_pts = Some(pts + duration);
        }

        let mut pending_events = mem::take(&mut state.pending_events);
        if state
            .channels
            .is_none_or(|old_channels| old_channels != number_channels)
            || state
                .depth
                .is_none_or(|old_depth| old_depth != bits_per_sample)
        {
            state.channels = Some(number_channels);
            state.depth = Some(bits_per_sample);

            let caps = gst::Caps::builder("audio/x-smpte-302m")
                .field("parsed", true)
                .field("channels", number_channels as i32)
                .field("rate", 48_000i32)
                .field("depth", bits_per_sample as i32)
                .build();
            gst::debug!(CAT, obj = pad, "Sending new caps {caps:?}");
            pending_events.push_front(gst::event::Caps::builder(&caps).build());
        }

        drop(state);

        for event in pending_events {
            let _ = self.srcpad.push_event(event);
        }

        self.srcpad.push(buffer)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            gst::EventView::Caps(ev) => {
                let caps = ev.caps();
                gst::debug!(CAT, obj = pad, "Dropping caps event {caps:?}");
                return true;
            }
            gst::EventView::FlushStop(_) => {
                let mut state = self.state.lock().unwrap();
                state.last_pts = None;
                state.discont = true;
                // Drop all non-sticky pending events and the events that are reset by flush-stop
                state.pending_events.retain(|ev| {
                    ev.is_sticky()
                        && ![
                            gst::EventType::Eos,
                            gst::EventType::StreamGroupDone,
                            gst::EventType::Segment,
                        ]
                        .contains(&ev.type_())
                });
            }
            _ => (),
        }

        if event.is_serialized() {
            let mut state = self.state.lock().unwrap();
            // Delay serialized events > CAPS until caps are set
            if (state.channels.is_none() || state.depth.is_none())
                && event
                    .type_()
                    .partial_cmp(&gst::EventType::Caps)
                    .is_none_or(|ord| ord == std::cmp::Ordering::Greater)
            {
                state.pending_events.push_back(event);
                return true;
            }
        }

        self.srcpad.push_event(event)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for S302MParse {
    const NAME: &'static str = "GstS302MParse";
    type Type = super::S302MParse;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                S302MParse::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |parse| parse.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                S302MParse::catch_panic_pad_function(
                    parent,
                    || false,
                    |parse| parse.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::PROXY_ALLOCATION)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .flags(gst::PadFlags::FIXED_CAPS | gst::PadFlags::PROXY_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
        }
    }
}

impl ObjectImpl for S302MParse {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for S302MParse {}

impl ElementImpl for S302MParse {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "S302M audio stream parser",
                "Codec/Parser/Audio",
                "Extracts metadata from an S302M audio stream",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/x-smpte-302m")
                    .field("parsed", true)
                    .field("channels", gst::List::new([2i32, 4, 6, 8]))
                    .field("rate", 48_000i32)
                    .field("depth", gst::List::new([16i32, 20, 24]))
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::new_empty_simple("audio/x-smpte-302m"),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        let res = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                *self.state.lock().unwrap() = State::default();
            }
            _ => (),
        }

        Ok(res)
    }
}
