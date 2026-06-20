//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::{cmp, collections::VecDeque, sync::Mutex};

use atomic_refcell::AtomicRefCell;
use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::{
    audio_discont::{AudioDiscont, AudioDiscontConfiguration},
    basepay::{PacketToBufferRelation, RtpBasePay2Ext, RtpBasePay2ImplExt, TimestampOffset},
};

#[derive(Clone)]
struct Settings {
    max_ptime: Option<gst::ClockTime>,
    min_ptime: gst::ClockTime,
    ptime_multiple: gst::ClockTime,
    audio_discont: AudioDiscontConfiguration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_ptime: None,
            min_ptime: gst::ClockTime::ZERO,
            ptime_multiple: gst::ClockTime::ZERO,
            audio_discont: AudioDiscontConfiguration::default(),
        }
    }
}

struct QueuedBuffer {
    /// ID of the buffer.
    id: u64,
    /// The mapped buffer itself.
    buffer: gst::MappedBuffer<gst::buffer::Readable>,
    /// Offset into the buffer that was not consumed yet.
    offset: usize,
}

#[derive(Default)]
struct State {
    /// Currently configured clock rate.
    clock_rate: Option<u32>,
    /// Number of bytes per frame.
    bpf: Option<usize>,

    /// Desired "packet time", i.e. packet duration, from the caps, if set.
    ptime: Option<gst::ClockTime>,
    max_ptime: Option<gst::ClockTime>,

    /// Currently queued buffers.
    queued_buffers: VecDeque<QueuedBuffer>,
    /// Currently queued number of bytes.
    queued_bytes: usize,

    audio_discont: AudioDiscont,
}

#[derive(Default)]
pub struct RtpBaseAudioPay2 {
    settings: Mutex<Settings>,
    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpbaseaudiopay2",
        gst::DebugColorFlags::empty(),
        Some("Base RTP Audio Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpBaseAudioPay2 {
    const ABSTRACT: bool = true;
    const NAME: &'static str = "GstRtpBaseAudioPay2";
    type Type = super::RtpBaseAudioPay2;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpBaseAudioPay2 {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            let mut properties = vec![
                // Using same type/semantics as C payloaders
                glib::ParamSpecInt64::builder("max-ptime")
                    .nick("Maximum Packet Time")
                    .blurb("Maximum duration of the packet data in ns (-1 = unlimited up to MTU)")
                    .default_value(
                        Settings::default()
                            .max_ptime
                            .map(gst::ClockTime::nseconds)
                            .map(|x| x as i64)
                            .unwrap_or(-1),
                    )
                    .minimum(-1)
                    .maximum(i64::MAX)
                    .mutable_playing()
                    .build(),
                // Using same type/semantics as C payloaders
                glib::ParamSpecInt64::builder("min-ptime")
                    .nick("Minimum Packet Time")
                    .blurb("Minimum duration of the packet data in ns (can't go above MTU)")
                    .default_value(Settings::default().min_ptime.nseconds() as i64)
                    .minimum(0)
                    .maximum(i64::MAX)
                    .mutable_playing()
                    .build(),
                // Using same type/semantics as C payloaders
                glib::ParamSpecInt64::builder("ptime-multiple")
                    .nick("Packet Time Multiple")
                    .blurb("Force buffers to be multiples of this duration in ns (0 disables)")
                    .default_value(Settings::default().ptime_multiple.nseconds() as i64)
                    .minimum(0)
                    .maximum(i64::MAX)
                    .mutable_playing()
                    .build(),
            ];

            properties.extend_from_slice(&AudioDiscontConfiguration::create_pspecs());

            properties
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        if self
            .settings
            .lock()
            .unwrap()
            .audio_discont
            .set_property(value, pspec)
        {
            return;
        }

        match pspec.name() {
            "max-ptime" => {
                let v = value.get::<i64>().unwrap();
                self.settings.lock().unwrap().max_ptime =
                    (v != -1).then_some(gst::ClockTime::from_nseconds(v as u64));
            }
            "min-ptime" => {
                let v = gst::ClockTime::from_nseconds(value.get::<i64>().unwrap() as u64);

                let mut settings = self.settings.lock().unwrap();
                let changed = settings.min_ptime != v;
                settings.min_ptime = v;
                drop(settings);

                if changed {
                    let _ = self
                        .obj()
                        .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }
            "ptime-multiple" => {
                self.settings.lock().unwrap().ptime_multiple =
                    gst::ClockTime::from_nseconds(value.get::<i64>().unwrap() as u64);
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        if let Some(value) = self.settings.lock().unwrap().audio_discont.property(pspec) {
            return value;
        }

        match pspec.name() {
            "max-ptime" => (self
                .settings
                .lock()
                .unwrap()
                .max_ptime
                .map(gst::ClockTime::nseconds)
                .map(|x| x as i64)
                .unwrap_or(-1))
            .to_value(),
            "min-ptime" => (self.settings.lock().unwrap().min_ptime.nseconds() as i64).to_value(),
            "ptime-multiple" => {
                (self.settings.lock().unwrap().ptime_multiple.nseconds() as i64).to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpBaseAudioPay2 {}

impl ElementImpl for RtpBaseAudioPay2 {}

impl crate::basepay::RtpBasePay2Impl for RtpBaseAudioPay2 {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();
        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();
        Ok(())
    }

    fn negotiate(&self, mut src_caps: gst::Caps) {
        // Fixate here as a first step
        src_caps.fixate();

        let s = src_caps.structure(0).unwrap();

        // Negotiate ptime/maxptime with downstream and use them in combination with the
        // properties. See https://datatracker.ietf.org/doc/html/rfc4566#section-6
        let ptime = s
            .get::<u32>("ptime")
            .ok()
            .map(u64::from)
            .map(gst::ClockTime::from_mseconds);

        let max_ptime = s
            .get::<u32>("maxptime")
            .ok()
            .map(u64::from)
            .map(gst::ClockTime::from_mseconds);

        let clock_rate = match s.get::<i32>("clock-rate") {
            Ok(clock_rate) if clock_rate > 0 => clock_rate as u32,
            _ => {
                panic!("RTP caps {src_caps:?} without 'clock-rate'");
            }
        };

        self.parent_negotiate(src_caps);

        // Draining happened above if the clock rate has changed
        let mut state = self.state.borrow_mut();
        state.ptime = ptime;
        state.max_ptime = max_ptime;
        state.clock_rate = Some(clock_rate);
        drop(state);
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.borrow_mut();
        self.drain_packets(&settings, &mut state, true)
    }

    fn flush(&self) {
        let mut state = self.state.borrow_mut();
        state.queued_buffers.clear();
        state.queued_bytes = 0;
        state.audio_discont.reset();
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let buffer = buffer.clone().into_mapped_buffer_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;
        let pts = buffer.buffer().pts().unwrap();

        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.borrow_mut();

        let Some(bpf) = state.bpf else {
            return Err(gst::FlowError::NotNegotiated);
        };
        let Some(clock_rate) = state.clock_rate else {
            return Err(gst::FlowError::NotNegotiated);
        };
        let num_samples = buffer.size() / bpf;

        let discont = state.audio_discont.process_input(
            &settings.audio_discont,
            buffer.buffer().flags().contains(gst::BufferFlags::DISCONT)
                || buffer.buffer().flags().contains(gst::BufferFlags::RESYNC),
            clock_rate,
            pts,
            num_samples,
        );

        if discont {
            if state.audio_discont.base_pts().is_some() {
                gst::debug!(CAT, imp = self, "Draining because of discontinuity");
                self.drain_packets(&settings, &mut state, true)?;
            }

            state.audio_discont.resync(pts, num_samples);
        }

        state.queued_bytes += buffer.buffer().size();
        state.queued_buffers.push_back(QueuedBuffer {
            id,
            buffer,
            offset: 0,
        });

        self.drain_packets(&settings, &mut state, false)
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        let res = self.parent_src_query(query);
        if !res {
            return false;
        }

        match query.view_mut() {
            gst::QueryViewMut::Latency(query) => {
                let (is_live, mut min, mut max) = query.result();
                let min_ptime = self.settings.lock().unwrap().min_ptime;
                min += min_ptime;
                max.opt_add_assign(min_ptime);
                query.set(is_live, min, max);
            }
            _ => (),
        }

        true
    }
}

impl RtpBaseAudioPay2 {
    /// Returns the minimum, maximum and chunk/multiple packet sizes
    fn calculate_packet_sizes(
        &self,
        settings: &Settings,
        state: &State,
        clock_rate: u32,
        bpf: usize,
    ) -> (usize, usize, usize) {
        let min_pframes = settings
            .min_ptime
            .nseconds()
            .mul_div_ceil(clock_rate as u64, gst::ClockTime::SECOND.nseconds())
            .unwrap() as u32;
        let max_ptime = match (settings.max_ptime, state.max_ptime) {
            (Some(max_ptime), Some(caps_max_ptime)) => Some(cmp::min(max_ptime, caps_max_ptime)),
            (None, Some(max_ptime)) => Some(max_ptime),
            (Some(max_ptime), None) => Some(max_ptime),
            _ => None,
        };
        let max_pframes = max_ptime.map(|max_ptime| {
            max_ptime
                .nseconds()
                .mul_div_ceil(clock_rate as u64, gst::ClockTime::SECOND.nseconds())
                .unwrap() as u32
        });
        let pframes_multiple = cmp::max(
            1,
            settings
                .ptime_multiple
                .nseconds()
                .mul_div_ceil(clock_rate as u64, gst::ClockTime::SECOND.nseconds())
                .unwrap() as u32,
        );

        gst::trace!(
            CAT,
            imp = self,
            "min ptime {} (frames: {}), max ptime {} (frames: {}), ptime multiple {} (frames {})",
            settings.min_ptime,
            min_pframes,
            settings.max_ptime.display(),
            max_pframes.unwrap_or(0),
            settings.ptime_multiple,
            pframes_multiple,
        );

        let psize_multiple = pframes_multiple as usize * bpf;

        let mut max_packet_size = self.obj().max_payload_size() as usize;
        max_packet_size -= max_packet_size % psize_multiple;
        if let Some(max_pframes) = max_pframes {
            max_packet_size = cmp::min(max_pframes as usize * bpf, max_packet_size)
        }
        let mut min_packet_size = cmp::min(
            cmp::max(min_pframes as usize * bpf, psize_multiple),
            max_packet_size,
        );

        if let Some(ptime) = state.ptime {
            let pframes = ptime
                .nseconds()
                .mul_div_ceil(clock_rate as u64, gst::ClockTime::SECOND.nseconds())
                .unwrap() as u32;

            let psize = pframes as usize * bpf;
            min_packet_size = cmp::max(min_packet_size, psize);
            max_packet_size = cmp::min(max_packet_size, psize);
        }

        (min_packet_size, max_packet_size, psize_multiple)
    }

    fn drain_packets(
        &self,
        settings: &Settings,
        state: &mut State,
        force: bool,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // Always set when caps are set
        let Some(clock_rate) = state.clock_rate else {
            return Ok(gst::FlowSuccess::Ok);
        };
        let bpf = state.bpf.unwrap();

        let (min_packet_size, max_packet_size, psize_multiple) =
            self.calculate_packet_sizes(settings, state, clock_rate, bpf);

        gst::trace!(
            CAT,
            imp = self,
            "Currently {} bytes queued, min packet size {min_packet_size}, max packet size {max_packet_size}, force {force}",
            state.queued_bytes,
        );

        while state.queued_bytes >= min_packet_size || (force && state.queued_bytes > 0) {
            let packet_size = {
                let mut packet_size = cmp::min(max_packet_size, state.queued_bytes);
                packet_size -= packet_size % psize_multiple;
                packet_size
            };

            gst::trace!(
                CAT,
                imp = self,
                "Creating packet of size {packet_size} ({} frames), marker {}",
                packet_size / bpf,
                state.audio_discont.next_output_offset().is_none(),
            );

            // Set marker bit on the first packet after a discontinuity
            let mut packet_builder = rtp_types::RtpPacketBuilder::new()
                .marker_bit(state.audio_discont.next_output_offset().is_none());

            let front = state.queued_buffers.front().unwrap();
            let start_id = front.id;
            let mut end_id = front.id;

            // Fill payload from all relevant buffers and collect start/end ids that apply.
            let mut remaining_packet_size = packet_size;
            for buffer in state.queued_buffers.iter() {
                let this_buffer_payload_size =
                    cmp::min(buffer.buffer.size() - buffer.offset, remaining_packet_size);

                end_id = buffer.id;
                packet_builder = packet_builder
                    .payload(&buffer.buffer[buffer.offset..][..this_buffer_payload_size]);

                remaining_packet_size -= this_buffer_payload_size;
                if remaining_packet_size == 0 {
                    break;
                }
            }

            // Then create the packet.
            self.obj().queue_packet(
                PacketToBufferRelation::IdsWithOffset {
                    ids: start_id..=end_id,
                    timestamp_offset: {
                        if let Some(next_out_offset) = state.audio_discont.next_output_offset() {
                            TimestampOffset::Rtp(next_out_offset)
                        } else {
                            TimestampOffset::Pts(gst::ClockTime::ZERO)
                        }
                    },
                },
                packet_builder,
            )?;

            // And finally dequeue or update all currently queued buffers.
            let mut remaining_packet_size = packet_size;
            while remaining_packet_size > 0 {
                let buffer = state.queued_buffers.front_mut().unwrap();

                if buffer.buffer.size() - buffer.offset > remaining_packet_size {
                    buffer.offset += remaining_packet_size;
                    remaining_packet_size = 0;
                } else {
                    remaining_packet_size -= buffer.buffer.size() - buffer.offset;
                    let _ = state.queued_buffers.pop_front();
                }
            }
            state.queued_bytes -= packet_size;
            state.audio_discont.process_output(packet_size / bpf);
        }

        gst::trace!(
            CAT,
            imp = self,
            "Currently {} bytes / {} frames queued",
            state.queued_bytes,
            state.queued_bytes / bpf
        );

        Ok(gst::FlowSuccess::Ok)
    }
}

/// Wrapper functions for public API.
#[allow(dead_code)]
impl RtpBaseAudioPay2 {
    pub(super) fn set_bpf(&self, bpf: usize) {
        let mut state = self.state.borrow_mut();
        state.bpf = Some(bpf);
    }
}
