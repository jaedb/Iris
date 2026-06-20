// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::VecDeque, sync::Mutex};

use anyhow::anyhow;
/**
 * SECTION:element-rtpamrpay2
 * @see_also: rtpamrdepay2, rtpamrpay, rtpamrdepay, amrnbdec, amrnbenc, amrwbdec, voamrwbenc
 *
 * Payloads an AMR audio stream into RTP packets as per [RFC 3267][rfc-3267].
 *
 * [rfc-3267]: https://datatracker.ietf.org/doc/html/rfc3267
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc wave=ticks ! amrnbenc ! rtpamrpay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will encode an audio test signal as AMR NB audio and payload it as RTP and send it out
 * over UDP to localhost port 5004.
 *
 * Since: 0.14
 */
use atomic_refcell::AtomicRefCell;

use bitstream_io::{BigEndian, BitWrite as _, BitWriter, ByteWrite, ByteWriter};
use gst::{glib, prelude::*, subclass::prelude::*};
use smallvec::SmallVec;
use std::sync::LazyLock;

use crate::{
    amr::payload_header::{
        PayloadConfiguration, PayloadHeader, TocEntry, NB_FRAME_SIZES, NB_FRAME_SIZES_BYTES,
        WB_FRAME_SIZES, WB_FRAME_SIZES_BYTES,
    },
    audio_discont::{AudioDiscont, AudioDiscontConfiguration},
    basepay::{
        PacketToBufferRelation, RtpBasePay2Ext, RtpBasePay2Impl, RtpBasePay2ImplExt,
        TimestampOffset,
    },
};

struct QueuedBuffer {
    /// ID of the buffer.
    id: u64,
    /// The mapped buffer itself.
    buffer: gst::MappedBuffer<gst::buffer::Readable>,
    /// Number of frames in this buffer.
    num_frames: usize,
    /// Offset (in frames) into the buffer if some frames were consumed already.
    offset: usize,
}

#[derive(Default)]
struct State {
    /// AMR NB or WB?
    wide_band: bool,
    /// Whether octet-align is set or not
    bandwidth_efficient: bool,

    /// Currently queued buffers
    queued_buffers: VecDeque<QueuedBuffer>,
    /// Queued bytes
    queued_bytes: usize,
    /// Queued frames
    queued_frames: usize,
    /// Full queued frames, including already forwarded frames.
    full_queued_frames: usize,

    /// Desired "packet time", i.e. packet duration, from the caps, if set.
    ptime: Option<gst::ClockTime>,
    max_ptime: Option<gst::ClockTime>,

    audio_discont: AudioDiscont,
}

#[derive(Clone)]
struct Settings {
    max_ptime: Option<gst::ClockTime>,
    aggregate_mode: super::AggregateMode,
    audio_discont: AudioDiscontConfiguration,
}

#[derive(Default)]
pub struct RtpAmrPay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
    is_live: Mutex<Option<bool>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_ptime: None,
            aggregate_mode: super::AggregateMode::Auto,
            audio_discont: AudioDiscontConfiguration::default(),
        }
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpamrpay2",
        gst::DebugColorFlags::empty(),
        Some("RTP AMR Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpAmrPay {
    const NAME: &'static str = "GstRtpAmrPay2";
    type Type = super::RtpAmrPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpAmrPay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            let mut properties = vec![
                glib::ParamSpecEnum::builder_with_default("aggregate-mode", Settings::default().aggregate_mode)
                    .nick("Aggregate Mode")
                    .blurb("Whether to send out audio frames immediately or aggregate them until a packet is full.")
                    .build(),
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
            "aggregate-mode" => {
                self.settings.lock().unwrap().aggregate_mode = value.get().unwrap();
            }
            "max-ptime" => {
                let v = value.get::<i64>().unwrap();
                self.settings.lock().unwrap().max_ptime =
                    (v != -1).then_some(gst::ClockTime::from_nseconds(v as u64));
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        if let Some(value) = self.settings.lock().unwrap().audio_discont.property(pspec) {
            return value;
        }

        match pspec.name() {
            "aggregate-mode" => self.settings.lock().unwrap().aggregate_mode.to_value(),
            "max-ptime" => (self
                .settings
                .lock()
                .unwrap()
                .max_ptime
                .map(gst::ClockTime::nseconds)
                .map(|x| x as i64)
                .unwrap_or(-1))
            .to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpAmrPay {}

impl ElementImpl for RtpAmrPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP AMR Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an AMR audio stream into RTP packets (RFC 3267)",
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
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("audio/AMR")
                            .field("channels", 1i32)
                            .field("rate", 8_000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/AMR-WB")
                            .field("channels", 1i32)
                            .field("rate", 16_000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", "AMR")
                            .field("clock-rate", 8_000i32)
                            .field("encoding-params", "1")
                            .field("octet-align", gst::List::new(["0", "1"]))
                            .field("crc", "0")
                            .field("robust-sorting", "0")
                            .field("interleaving", "0")
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", "AMR-WB")
                            .field("clock-rate", 16_000i32)
                            .field("encoding-params", "1")
                            .field("octet-align", gst::List::new(["0", "1"]))
                            .field("crc", "0")
                            .field("robust-sorting", "0")
                            .field("interleaving", "0")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}
impl RtpBasePay2Impl for RtpAmrPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();
        let wide_band = s.name() == "audio/AMR-WB";

        let src_templ_caps = self.obj().src_pad().pad_template_caps();

        let src_caps = src_templ_caps
            .iter()
            .find(|s| {
                (s.get::<&str>("encoding-name") == Ok("AMR") && !wide_band)
                    || (s.get::<&str>("encoding-name") == Ok("AMR-WB") && wide_band)
            })
            .map(|s| gst::Caps::from(s.to_owned()))
            .unwrap();

        gst::debug!(CAT, imp = self, "Setting caps {src_caps:?}");

        self.obj().set_src_caps(&src_caps);

        let mut state = self.state.borrow_mut();
        state.wide_band = wide_band;

        true
    }

    fn negotiate(&self, mut src_caps: gst::Caps) {
        src_caps.truncate();

        // Prefer octet-aligned streams.
        {
            let src_caps = src_caps.make_mut();
            let s = src_caps.structure_mut(0).unwrap();
            s.fixate_field_str("octet-align", "1");
        }

        // Fixate as the first step
        src_caps.fixate();

        let s = src_caps.structure(0).unwrap();
        let bandwidth_efficient = s.get::<&str>("octet-align") != Ok("1");

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

        self.parent_negotiate(src_caps);

        let mut state = self.state.borrow_mut();
        state.bandwidth_efficient = bandwidth_efficient;
        state.ptime = ptime;
        state.max_ptime = max_ptime;
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
        state.queued_frames = 0;
        state.full_queued_frames = 0;

        state.audio_discont.reset();
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.borrow_mut();

        let buffer = buffer.clone().into_mapped_buffer_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        let pts = buffer.buffer().pts().unwrap();

        let mut num_frames = 0;
        let iter = AmrIter {
            data: &buffer,
            wide_band: state.wide_band,
        };
        for item in iter {
            if let Err(err) = item {
                gst::error!(CAT, imp = self, "Invalid AMR buffer: {err}");
                return Err(gst::FlowError::Error);
            }
            num_frames += 1;
        }

        let rate = if state.wide_band { 16_000 } else { 8_000 };
        let num_samples = num_frames * if state.wide_band { 320 } else { 160 };

        let discont = state.audio_discont.process_input(
            &settings.audio_discont,
            buffer.buffer().flags().contains(gst::BufferFlags::DISCONT),
            rate,
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
        state.queued_frames += num_frames;
        state.full_queued_frames += num_frames;
        state.queued_buffers.push_back(QueuedBuffer {
            id,
            buffer,
            num_frames,
            offset: 0,
        });

        // Make sure we have queried upstream liveness if needed
        if settings.aggregate_mode == super::AggregateMode::Auto {
            self.ensure_upstream_liveness(&settings);
        }

        self.drain_packets(&settings, &mut state, false)
    }

    #[allow(clippy::single_match)]
    fn sink_query(&self, query: &mut gst::QueryRef) -> bool {
        match query.view_mut() {
            gst::QueryViewMut::Caps(query) => {
                let src_tmpl_caps = self.obj().src_pad().pad_template_caps();

                let peer_caps = self.obj().src_pad().peer_query_caps(Some(&src_tmpl_caps));

                if peer_caps.is_empty() {
                    query.set_result(&peer_caps);
                    return true;
                }

                let rtp_amr_nb_caps = gst::Caps::builder("application/x-rtp")
                    .field("encoding-name", "AMR")
                    .build();
                let rtp_amr_wb_caps = gst::Caps::builder("application/x-rtp")
                    .field("encoding-name", "AMR-WB")
                    .build();

                let sink_templ_caps = self.obj().sink_pad().pad_template_caps();
                let amr_nb_supported = peer_caps.can_intersect(&rtp_amr_nb_caps);
                let amr_wb_supported = peer_caps.can_intersect(&rtp_amr_wb_caps);

                let mut ret_caps_builder = gst::Caps::builder_full();
                for s in sink_templ_caps.iter() {
                    if (s.name() == "audio/AMR" && amr_nb_supported)
                        || (s.name() == "audio/AMR-WB" && amr_wb_supported)
                    {
                        ret_caps_builder = ret_caps_builder.structure(s.to_owned());
                    }
                }

                let mut ret_caps = ret_caps_builder.build();
                if let Some(filter) = query.filter() {
                    ret_caps = ret_caps.intersect_with_mode(filter, gst::CapsIntersectMode::First);
                }

                query.set_result(&ret_caps);

                return true;
            }
            _ => (),
        }

        self.parent_sink_query(query)
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        let res = self.parent_src_query(query);
        if !res {
            return false;
        }

        match query.view_mut() {
            gst::QueryViewMut::Latency(query) => {
                let settings = self.settings.lock().unwrap();

                let (is_live, mut min, mut max) = query.result();

                {
                    let mut live_guard = self.is_live.lock().unwrap();

                    if Some(is_live) != *live_guard {
                        gst::info!(CAT, imp = self, "Upstream is live: {is_live}");
                        *live_guard = Some(is_live);
                    }
                }

                if self.effective_aggregate_mode(&settings) == super::AggregateMode::Aggregate {
                    if let Some(max_ptime) = settings.max_ptime {
                        min += max_ptime;
                        max.opt_add_assign(max_ptime);
                    } else if is_live {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Aggregating packets in live mode, but no max_ptime configured. \
                            Configured latency may be too low!"
                        );
                    }
                    query.set(is_live, min, max);
                }
            }
            _ => (),
        }

        true
    }
}

impl RtpAmrPay {
    fn drain_packets(
        &self,
        settings: &Settings,
        state: &mut State,
        drain: bool,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let agg_mode = self.effective_aggregate_mode(settings);

        let max_payload_size = self.obj().max_payload_size() as usize - 1;
        let max_ptime = self.calc_effective_max_ptime(settings, state);

        let payload_configuration = PayloadConfiguration {
            has_crc: false,
            wide_band: state.wide_band,
        };

        // Send out packets if there's enough data for one (or more), or if draining.
        while let Some(first) = state.queued_buffers.front() {
            let num_buffers = state.queued_buffers.len();
            let queued_bytes = state.queued_bytes;
            let queued_frames = state.queued_frames;
            let queued_duration =
                (gst::ClockTime::from_mseconds(20) * queued_frames as u64).nseconds();

            // We optimistically add average size/duration to send out packets as early as possible
            // if we estimate that the next buffer would likely overflow our accumulation limits.
            //
            // Th duration is based on the full buffer content as we'd have to wait not just 20ms until
            // the next buffer but the average number of frames per buffer times 20ms.
            let full_queued_frames = state.full_queued_frames;
            let full_queued_duration =
                (gst::ClockTime::from_mseconds(20) * full_queued_frames as u64).nseconds();
            let avg_bytes = queued_bytes / queued_frames;
            let avg_duration = full_queued_duration / num_buffers as u64;

            let is_ready = drain
                || agg_mode != super::AggregateMode::Aggregate
                || queued_bytes + avg_bytes > max_payload_size
                || (max_ptime.is_some_and(|max_ptime| queued_duration + avg_duration > max_ptime));

            gst::log!(
                CAT,
                imp = self,
                "Queued: bytes {}, duration ~{}ms, mode: {:?} + drain {} => ready: {}",
                queued_bytes,
                queued_duration / 1_000_000,
                agg_mode,
                drain,
                is_ready
            );

            if !is_ready {
                gst::log!(CAT, imp = self, "Not ready yet, waiting for more data");
                break;
            }

            gst::trace!(CAT, imp = self, "Creating packet..");

            let mut payload_header = PayloadHeader {
                cmr: 15,
                toc_entries: SmallVec::new(),
                crc: SmallVec::new(),
            };

            let mut frame_payloads = SmallVec::<[&[u8]; 16]>::new();

            let start_offset = first.offset;
            let start_id = first.id;
            let mut end_id = start_id;
            let mut acc_duration = 0;
            let mut acc_size = 0;
            let discont = state.audio_discont.next_output_offset().is_none();

            for buffer in &state.queued_buffers {
                let iter = AmrIter {
                    data: &buffer.buffer,
                    wide_band: state.wide_band,
                };

                for frame in iter.skip(buffer.offset) {
                    let (frame_type, frame_quality_indicator, frame_data) = frame.unwrap();
                    gst::trace!(
                        CAT,
                        imp = self,
                        "frame type {frame_type:?}, accumulated size {acc_size} duration ~{}ms",
                        acc_duration / 1_000_000
                    );

                    // If this frame would overflow the packet, bail out and send out what we have.
                    //
                    // Don't take into account the max_ptime for the first frame, since it could be
                    // lower than the frame duration in which case we would never payload anything.
                    //
                    // For the size check in bytes we know that the first frame will fit the mtu,
                    // because we already checked for the "audio frame bigger than mtu" scenario above.
                    if acc_size + frame_data.len() + 1 > max_payload_size
                        || (max_ptime.is_some()
                            && acc_duration > 0
                            && acc_duration + 20_000_000 > max_ptime.unwrap())
                    {
                        break;
                    }

                    // ... otherwise add frame to the TOC
                    payload_header.toc_entries.push(TocEntry {
                        last: false,
                        frame_type,
                        frame_quality_indicator,
                    });
                    frame_payloads.push(frame_data);
                    end_id = buffer.id;

                    acc_size += frame_data.len() + 1;
                    acc_duration += 20_000_000;

                    // .. otherwise check if there are more frames we can add to the packet
                }
            }

            assert!(!payload_header.toc_entries.is_empty());
            payload_header.toc_entries.last_mut().unwrap().last = true;

            let mut payload_buf = SmallVec::<[u8; 1500]>::new();
            let mut packet_builder;
            if state.bandwidth_efficient {
                let frame_sizes = if state.wide_band {
                    WB_FRAME_SIZES.as_slice()
                } else {
                    NB_FRAME_SIZES.as_slice()
                };

                payload_buf.reserve(1 + payload_header.toc_entries.len() + acc_size);

                let mut w = BitWriter::endian(&mut payload_buf, BigEndian);
                if let Err(err) = w.build_with(&payload_header, &payload_configuration) {
                    gst::error!(CAT, imp = self, "Failed writing payload header: {err}");
                    return Err(gst::FlowError::Error);
                }

                for (toc_entry, mut data) in
                    Iterator::zip(payload_header.toc_entries.iter(), frame_payloads)
                {
                    let mut num_bits =
                        *frame_sizes.get(toc_entry.frame_type as usize).unwrap_or(&0);

                    while num_bits > 8 {
                        if let Err(err) = w.write_from(data[0]) {
                            gst::error!(CAT, imp = self, "Failed writing payload: {err}");
                            return Err(gst::FlowError::Error);
                        }
                        data = &data[1..];
                        num_bits -= 8;
                    }

                    if num_bits > 0 {
                        if let Err(err) = w.write_var(num_bits as u32, data[0] >> (8 - num_bits)) {
                            gst::error!(CAT, imp = self, "Failed writing payload: {err}");
                            return Err(gst::FlowError::Error);
                        }
                    }
                }

                let _ = w.byte_align();

                packet_builder = rtp_types::RtpPacketBuilder::new()
                    .marker_bit(discont)
                    .payload(payload_buf.as_slice());
            } else {
                payload_buf.reserve(1 + payload_header.toc_entries.len());

                let mut w = ByteWriter::endian(&mut payload_buf, BigEndian);
                if let Err(err) = w.build_with(&payload_header, &payload_configuration) {
                    gst::error!(CAT, imp = self, "Failed writing payload header: {err}");
                    return Err(gst::FlowError::Error);
                }

                packet_builder = rtp_types::RtpPacketBuilder::new()
                    .marker_bit(discont)
                    .payload(payload_buf.as_slice());

                for data in frame_payloads {
                    packet_builder = packet_builder.payload(data);
                }
            }

            self.obj().queue_packet(
                PacketToBufferRelation::IdsWithOffset {
                    ids: start_id..=end_id,
                    timestamp_offset: {
                        if let Some(next_out_offset) = state.audio_discont.next_output_offset() {
                            TimestampOffset::Rtp(next_out_offset)
                        } else {
                            TimestampOffset::Pts(
                                gst::ClockTime::from_mseconds(20) * start_offset as u64,
                            )
                        }
                    },
                },
                packet_builder,
            )?;

            let mut remaining_frames = payload_header.toc_entries.len();
            while remaining_frames > 0 {
                let first = state.queued_buffers.front_mut().unwrap();

                if remaining_frames >= first.num_frames - first.offset {
                    remaining_frames -= first.num_frames - first.offset;
                    let _ = state.queued_buffers.pop_front();
                } else {
                    first.offset += remaining_frames;
                    remaining_frames = 0;
                }
            }

            state.queued_bytes -= acc_size;
            state.queued_frames -= payload_header.toc_entries.len();
            state.full_queued_frames -= payload_header.toc_entries.len();
            let acc_samples =
                payload_header.toc_entries.len() * if state.wide_band { 320 } else { 160 };
            state.audio_discont.process_output(acc_samples);
        }

        gst::log!(
            CAT,
            imp = self,
            "All done for now, {} buffer / {} frames queued",
            state.queued_buffers.len(),
            state.queued_frames,
        );

        if drain {
            self.obj().finish_pending_packets()?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn effective_aggregate_mode(&self, settings: &Settings) -> super::AggregateMode {
        match settings.aggregate_mode {
            super::AggregateMode::Auto => match self.is_live() {
                Some(true) => super::AggregateMode::ZeroLatency,
                Some(false) => super::AggregateMode::Aggregate,
                None => super::AggregateMode::ZeroLatency,
            },
            mode => mode,
        }
    }

    fn is_live(&self) -> Option<bool> {
        *self.is_live.lock().unwrap()
    }

    // Query upstream live-ness if needed, in case of aggregate-mode=auto
    fn ensure_upstream_liveness(&self, settings: &Settings) {
        if settings.aggregate_mode != super::AggregateMode::Auto || self.is_live().is_some() {
            return;
        }

        let mut q = gst::query::Latency::new();
        let is_live = if self.obj().sink_pad().peer_query(&mut q) {
            let (is_live, _, _) = q.result();
            is_live
        } else {
            false
        };

        *self.is_live.lock().unwrap() = Some(is_live);

        gst::info!(CAT, imp = self, "Upstream is live: {is_live}");
    }

    // We can get max ptime or ptime recommendations/restrictions from multiple places, e.g. the
    // "max-ptime" property, but also from "maxptime" or "ptime" values from downstream / an SDP.
    //
    // Here we look at the various values and decide on an effective max ptime value.
    //
    // We'll just return the lowest of any set values.
    //
    fn calc_effective_max_ptime(&self, settings: &Settings, state: &State) -> Option<u64> {
        [settings.max_ptime, state.max_ptime, state.ptime]
            .into_iter()
            .filter(|v| v.is_some())
            .min()
            .map(|t| t.unwrap().nseconds())
    }
}

struct AmrIter<'a> {
    data: &'a [u8],
    wide_band: bool,
}

impl<'a> Iterator for AmrIter<'a> {
    type Item = Result<(u8, bool, &'a [u8]), anyhow::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        let frame_sizes = if self.wide_band {
            WB_FRAME_SIZES_BYTES.as_slice()
        } else {
            NB_FRAME_SIZES_BYTES.as_slice()
        };

        let frame_type = (self.data[0] & 0b0111_1000) >> 3;
        let frame_quality_indicator = (self.data[0] & 0b0000_0100) != 0;
        if !self.wide_band && (9..=14).contains(&frame_type) {
            self.data = &[];
            return Some(Err(anyhow!("Invalid AMR frame type {frame_type}")));
        }
        if self.wide_band && (10..=13).contains(&frame_type) {
            self.data = &[];
            return Some(Err(anyhow!("Invalid AMR-WB frame type {frame_type}")));
        }

        // Empty frames
        if frame_type > 10 {
            self.data = &self.data[1..];
            return Some(Ok((frame_type, frame_quality_indicator, &[])));
        }

        let frame_size = *frame_sizes
            .get(frame_type as usize)
            .expect("Invalid frame type") as usize;
        if self.data.len() < frame_size + 1 {
            self.data = &[];
            return Some(Err(anyhow!("Not enough data")));
        }

        let res_data = &self.data[1..][..frame_size];
        self.data = &self.data[(frame_size + 1)..];

        Some(Ok((frame_type, frame_quality_indicator, res_data)))
    }
}
