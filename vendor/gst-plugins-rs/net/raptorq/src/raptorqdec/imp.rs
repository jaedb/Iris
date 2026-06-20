// Copyright (C) 2022 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use gst::{error_msg, glib};

use gst::prelude::*;
use gst::subclass::prelude::*;

use gst_rtp::RTPBuffer;

use std::sync::LazyLock;

use std::collections::BTreeMap;
use std::iter;
use std::ops::Range;
use std::sync::Mutex;

use raptorq::{EncodingPacket, ObjectTransmissionInformation, PayloadId, SourceBlockDecoder};

use crate::fecscheme::{self, DataUnitHeader, RepairPayloadId};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "raptorqdec",
        gst::DebugColorFlags::empty(),
        Some("RTP RaptorQ Decoder"),
    )
});

const DEFAULT_REPAIR_WINDOW_TOLERANCE: u32 = 500;
const DEFAULT_MEDIA_PACKETS_RESET_THRESHOLD: u32 = 5000;

#[derive(Debug, Clone, Copy)]
struct Settings {
    repair_window_tolerance: u32,
    media_packets_reset_threshold: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            repair_window_tolerance: DEFAULT_REPAIR_WINDOW_TOLERANCE,
            media_packets_reset_threshold: DEFAULT_MEDIA_PACKETS_RESET_THRESHOLD,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    recv: u64,
    lost: u64,
    recovered: u64,
}

#[derive(Debug, Clone, Copy)]
struct SourceBlockInfo {
    initial_seq: u64,
    symbols_per_block: u64,
    symbols_per_packet: u64,
}

impl SourceBlockInfo {
    fn seq_range(&self) -> Range<u64> {
        // RFC 6881, section 8.2.2
        let i = self.initial_seq;
        let lp = self.symbols_per_packet;
        let lb = self.symbols_per_block;

        i..i + lb / lp
    }

    fn packets_num(&self) -> usize {
        (self.symbols_per_block / self.symbols_per_packet) as usize
    }
}

#[derive(Debug, Clone)]
struct RepairPacketItem {
    payload_id: RepairPayloadId,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MediaPacketItem {
    header: DataUnitHeader,
    payload: Vec<u8>,
}

#[derive(Default)]
struct State {
    media_packets: BTreeMap<u64, MediaPacketItem>,
    repair_packets: BTreeMap<u64, Vec<RepairPacketItem>>,
    expirations: BTreeMap<u64, Option<gst::ClockTime>>,
    source_block_info: BTreeMap<u64, SourceBlockInfo>,
    extended_media_seq: Option<u64>,
    extended_repair_seq: Option<u64>,
    symbol_size: usize,
    media_packets_reset_threshold: usize,
    repair_window: Option<gst::ClockTime>,
    max_arrival_time: Option<gst::ClockTime>,
    stats: Stats,
}

impl State {
    fn drop_source_block(&mut self, seq: u64) {
        if let Some(info) = self.source_block_info.get(&seq) {
            let (seq_lo, seq_hi) = (info.seq_range().start, info.seq_range().end);

            self.media_packets.retain(|&k, _| k >= seq_hi);
            self.repair_packets.remove(&seq_lo);
            self.source_block_info.remove(&seq_lo);
            self.expirations.remove(&seq_lo);
        }
    }

    fn expire_packets(&mut self) -> Vec<u64> {
        let expired = self
            .expirations
            .iter()
            .filter_map(|(&seq, &expiration)| {
                if self.max_arrival_time.opt_gt(expiration) == Some(true) {
                    Some(seq)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for seq in &expired {
            self.drop_source_block(*seq);
        }

        expired
    }
}

pub struct RaptorqDec {
    sinkpad: gst::Pad,
    srcpad: gst::Pad,
    sinkpad_fec: Mutex<Option<gst::Pad>>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl RaptorqDec {
    fn process_source_block(&self, state: &mut State) -> Result<gst::FlowSuccess, gst::FlowError> {
        // Pull the information about the current Source Block from sequence.
        // Data packets for current Source Block are in range: info.seq_range(),
        // Repair Packets on the other hand, for a given block share the same key
        // in the sequence, which is the lower seq bound of Data Packets.
        let source_block_info = state
            .source_block_info
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for info in source_block_info {
            let (seq_lo, seq_hi) = (info.seq_range().start, info.seq_range().end);
            let data_packets_num = state.media_packets.range(seq_lo..seq_hi).count();
            let n = info.packets_num();

            if data_packets_num == n {
                gst::trace!(
                    CAT,
                    imp = self,
                    "All packets ({}) received, dropping Source Block ({})",
                    data_packets_num,
                    seq_lo
                );

                state.drop_source_block(seq_lo);
                continue;
            }

            let repair_packets_num = state.repair_packets.entry(seq_lo).or_default().len();

            // Wait until we have enough Symbols to start decoding a Block
            if data_packets_num + repair_packets_num < n {
                continue;
            }

            // Build Source Block from received Data Packets and append
            // Repair Packets that have the same initial sequence number
            let mut source_block = Vec::with_capacity(
                (data_packets_num + repair_packets_num)
                    .checked_mul(state.symbol_size)
                    .ok_or(gst::FlowError::NotSupported)?
                    .checked_mul(info.symbols_per_packet as usize)
                    .ok_or(gst::FlowError::NotSupported)?,
            );

            source_block.extend(
                Iterator::chain(
                    state
                        .media_packets
                        .range(seq_lo..seq_hi)
                        .map(|(_, packet)| {
                            let si = info.symbols_per_packet as usize;
                            let mut data = vec![0; si * state.symbol_size];

                            assert!(data.len() >= packet.payload.len() + 3);

                            data[0..3].copy_from_slice(&packet.header.encode());
                            data[3..3 + packet.payload.len()].copy_from_slice(&packet.payload);
                            data
                        }),
                    state
                        .repair_packets
                        .entry(seq_lo)
                        .or_default()
                        .iter()
                        .map(|packet| packet.payload.to_owned()),
                )
                .flatten(),
            );

            // RFC 6881, section 8.2.2
            let esi = Iterator::chain(
                state
                    .media_packets
                    .range(seq_lo..seq_hi)
                    .flat_map(|(seq, _)| {
                        let i = (seq - seq_lo) * info.symbols_per_packet;
                        (i..i + info.symbols_per_packet).collect::<Vec<_>>()
                    }),
                state
                    .repair_packets
                    .entry(seq_lo)
                    .or_default()
                    .iter()
                    .flat_map(|packet| {
                        let i = packet.payload_id.encoding_symbol_id as u64;
                        (i..i + info.symbols_per_packet).collect::<Vec<_>>()
                    }),
            )
            .collect::<Vec<_>>();

            let symbolsz = state.symbol_size as u64;
            let blocksz = info.symbols_per_block * symbolsz;

            let config = ObjectTransmissionInformation::new(0, symbolsz as u16, 1, 1, 8);
            let mut decoder = SourceBlockDecoder::new(0, &config, blocksz);
            let mut result = None;

            for (esi, symbol) in
                Iterator::zip(esi.iter(), source_block.chunks_exact(state.symbol_size))
            {
                let payload_id = PayloadId::new(0, *esi as u32);
                let encoding_packet = EncodingPacket::new(payload_id, symbol.to_vec());

                result = decoder.decode(iter::once(encoding_packet));
                if result.is_some() {
                    break;
                }
            }

            if let Some(data) = result {
                // Find missing packets in the Source Block
                let missing_indices = (seq_lo..seq_hi)
                    .filter_map(|seq| match state.media_packets.contains_key(&seq) {
                        false => Some((seq - seq_lo) as usize),
                        true => None,
                    })
                    .collect::<Vec<_>>();

                let pktsz = (info.symbols_per_packet * symbolsz) as usize;

                let recovered_packets = missing_indices
                    .iter()
                    .filter_map(|i| {
                        let packet = &data[i * pktsz..];
                        let header = packet[0..3].try_into().ok()?;

                        let len = DataUnitHeader::decode(header).len_indication as usize;

                        // Length indication does not account for Unit Header and RTP header
                        if packet.len() >= len + 3 + 12 {
                            let data_unit = packet[3..len + 12 + 3].to_owned();
                            let mut buf = gst::Buffer::from_slice(data_unit);

                            let buf_mut = buf.get_mut().unwrap();
                            buf_mut.set_dts(state.max_arrival_time);

                            return Some(buf);
                        }

                        None
                    })
                    .collect::<Vec<_>>();

                state.drop_source_block(seq_lo);
                state.stats.lost += missing_indices.len() as u64;

                for packet in recovered_packets {
                    {
                        let rtpbuf = RTPBuffer::from_buffer_readable(&packet).unwrap();

                        gst::debug!(
                            CAT,
                            imp = self,
                            "Successfully recovered packet: seqnum: {}, len: {}, ts: {}",
                            rtpbuf.seq(),
                            rtpbuf.payload_size(),
                            rtpbuf.timestamp(),
                        );
                    }

                    state.stats.recovered += 1;
                    self.srcpad.push(packet)?;
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn store_media_packet(
        &self,
        state: &mut State,
        buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let this_seq = {
            let rtpbuf = RTPBuffer::from_buffer_readable(buffer).map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to map rtp buffer : {}", err);
                gst::FlowError::Error
            })?;

            gst::trace!(
                CAT,
                imp = self,
                "New data packet, seq {}, ts {}",
                rtpbuf.seq(),
                rtpbuf.timestamp()
            );

            // Expand cyclic sequence numbers to u64, start from u16::MAX so we
            // never overflow subtraction.
            let seq = rtpbuf.seq();
            let prev_seq = state.extended_media_seq.unwrap_or(65_535 + seq as u64);

            let delta = gst_rtp::compare_seqnum(prev_seq as u16, seq);

            match delta.is_negative() {
                true => prev_seq - delta.unsigned_abs() as u64,
                false => prev_seq + delta.unsigned_abs() as u64,
            }
        };

        let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

        // As defined in RFC6881, section 8.2.4, length indication
        // should be equal to UDP packet length without RTP header.
        let header = DataUnitHeader {
            flow_indication: 0,
            len_indication: buffer.size() as u16 - 12,
        };

        state.media_packets.insert(
            this_seq,
            MediaPacketItem {
                header,
                payload: map.to_vec(),
            },
        );

        state.stats.recv += 1;
        state.extended_media_seq = Some(this_seq);

        let now = buffer.dts_or_pts();
        state.max_arrival_time = state.max_arrival_time.opt_max(now).or(now);

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        self.store_media_packet(&mut state, &buffer)?;

        // Retire the packets that have been around for too long
        let expired = state.expire_packets();
        for seq in expired {
            gst::trace!(
                CAT,
                imp = self,
                "Source Block ({}) dropped, because max wait time has been exceeded",
                seq as u16
            );
        }

        // This is the fuse to make sure we are not growing RTP storage indefinitely.
        let thresh = state.media_packets_reset_threshold;
        if thresh > 0 && state.media_packets.len() >= thresh {
            gst::warning!(
                CAT,
                imp = self,
                "Too many buffered media packets, resetting decoder. This might \
                 be because we haven't received a repair packet for too long, or \
                 repair packets have no valid timestamps.",
            );

            self.reset();
        }

        self.process_source_block(&mut state)?;
        drop(state);

        self.srcpad.push(buffer)
    }

    fn fec_sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let rtpbuf = RTPBuffer::from_buffer_readable(&buffer).map_err(|err| {
            gst::error!(CAT, imp = self, "Failed to map rtp buffer : {}", err);
            gst::FlowError::Error
        })?;

        let payload = rtpbuf.payload().unwrap();
        let payload_id = payload[0..7].try_into().map_err(|err| {
            gst::error!(CAT, imp = self, "Unexpected rtp fec payload : {}", err);
            gst::FlowError::Error
        })?;

        let mut state = self.state.lock().unwrap();

        let id = RepairPayloadId::decode(payload_id);

        let i = id.initial_sequence_num;
        let lb = id.source_block_len as u64;
        let lp = ((payload.len() - payload_id.len()) / state.symbol_size) as u64;

        gst::trace!(
            CAT,
            imp = self,
            "New repair packet, I: {}, LP: {}, LB: {}",
            i,
            lp,
            lb,
        );

        // Expand cyclic sequence numbers to u64, start from u16::MAX so we
        // never overflow subtraction.
        let prev_seq = state.extended_repair_seq.unwrap_or(65_535 + i as u64);
        let delta = gst_rtp::compare_seqnum(prev_seq as u16, i);

        let this_seq = match delta.is_negative() {
            true => prev_seq - delta.unsigned_abs() as u64,
            false => prev_seq + delta.unsigned_abs() as u64,
        };

        state.extended_repair_seq = Some(this_seq);

        let expire_at = state.max_arrival_time.opt_add(state.repair_window);
        let scheduled = state.expirations.entry(this_seq).or_insert(expire_at);

        // Update already scheduled expiration if a new value happens to be earlier
        *scheduled = scheduled.opt_min(expire_at);

        state
            .source_block_info
            .entry(this_seq)
            .or_insert(SourceBlockInfo {
                initial_seq: this_seq,
                symbols_per_block: lb,
                symbols_per_packet: lp,
            });

        state
            .repair_packets
            .entry(this_seq)
            .or_default()
            .push(RepairPacketItem {
                payload_id: id,
                payload: payload[7..].to_vec(), // without PayloadId
            });

        assert_eq!(state.repair_packets.len(), state.source_block_info.len());
        assert_eq!(state.repair_packets.len(), state.expirations.len());

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::debug!(CAT, "Handling event {:?}", event);
        use gst::EventView;

        if let EventView::FlushStop(_) = event.view() {
            self.reset();
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    fn fec_sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::debug!(CAT, "Handling event {:?}", event);
        use gst::EventView;

        if let EventView::Caps(c) = event.view() {
            if let Err(err) = self.start(c.caps()) {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Event,
                    ["Failed to start raptorqdec {:?}", err]
                );

                return false;
            }
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    fn iterate_internal_links(&self, pad: &gst::Pad) -> gst::Iterator<gst::Pad> {
        if pad == &self.srcpad {
            gst::Iterator::from_vec(vec![self.sinkpad.clone()])
        } else if pad == &self.sinkpad {
            gst::Iterator::from_vec(vec![self.srcpad.clone()])
        } else {
            gst::Iterator::from_vec(vec![])
        }
    }

    fn start(&self, incaps: &gst::CapsRef) -> Result<(), gst::ErrorMessage> {
        let symbol_size = fmtp_param_from_caps::<usize>("t", incaps)?;

        if symbol_size > fecscheme::MAX_ENCODING_SYMBOL_SIZE {
            return Err(error_msg!(
                gst::CoreError::Failed,
                [
                    "Symbol size exceeds Maximum Encoding Symbol Size: {}",
                    fecscheme::MAX_ENCODING_SYMBOL_SIZE
                ]
            ));
        }

        let settings = self.settings.lock().unwrap();

        let tolerance = settings.repair_window_tolerance as u64;
        let repair_window = fmtp_param_from_caps::<u64>("repair-window", incaps)?;

        let tolerance = tolerance.mseconds();
        let repair_window = repair_window.useconds();
        let repair_window = Some(repair_window + tolerance);

        let media_packets_reset_threshold = settings.media_packets_reset_threshold as usize;

        gst::debug!(CAT, imp = self, "Configured for caps {}", incaps);

        let mut state = self.state.lock().unwrap();

        state.symbol_size = symbol_size;
        state.repair_window = repair_window;
        state.media_packets_reset_threshold = media_packets_reset_threshold;

        Ok(())
    }

    fn stop(&self) {
        self.reset();
    }

    fn reset(&self) {
        let mut state = self.state.lock().unwrap();

        state.media_packets.clear();
        state.repair_packets.clear();
        state.source_block_info.clear();
        state.expirations.clear();
        state.extended_media_seq = None;
        state.extended_repair_seq = None;
        state.max_arrival_time = gst::ClockTime::NONE;
        state.stats = Default::default();
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RaptorqDec {
    const NAME: &'static str = "GstRaptorqDec";
    type Type = super::RaptorqDec;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Self::catch_panic_pad_function(parent, || false, |this| this.sink_event(pad, event))
            })
            .iterate_internal_links_function(|pad, parent| {
                Self::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |this| this.iterate_internal_links(pad),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .iterate_internal_links_function(|pad, parent| {
                Self::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |this| this.iterate_internal_links(pad),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();

        Self {
            srcpad,
            sinkpad,
            sinkpad_fec: Mutex::new(None),
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for RaptorqDec {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("repair-window-tolerance")
                    .nick("Repair Window Tolerance (ms)")
                    .blurb("The amount of time to add to repair-window reported by RaptorQ encoder (in ms)")
                    .maximum(u32::MAX - 1)
                    .default_value(DEFAULT_REPAIR_WINDOW_TOLERANCE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("media-packets-reset-threshold")
                    .nick("Media Packets Reset Threshold")
                    .blurb("This is the maximum allowed number of buffered packets, before we reset the decoder. \
                     It can only be triggered if we don't receive repair packets for too long, or packets \
                     have no valid timestamps, (0 - disable)")
                    .maximum(u32::MAX - 1)
                    .default_value(DEFAULT_MEDIA_PACKETS_RESET_THRESHOLD)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Statistics")
                    .blurb("Various statistics")
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "repair-window-tolerance" => {
                let mut settings = self.settings.lock().unwrap();
                let val = value.get().expect("type checked upstream");
                settings.repair_window_tolerance = val;
            }

            "media-packets-reset-threshold" => {
                let mut settings = self.settings.lock().unwrap();
                let val = value.get().expect("type checked upstream");
                settings.media_packets_reset_threshold = val;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "repair-window-tolerance" => {
                let settings = self.settings.lock().unwrap();
                settings.repair_window_tolerance.to_value()
            }
            "media-packets-reset-threshold" => {
                let settings = self.settings.lock().unwrap();
                settings.media_packets_reset_threshold.to_value()
            }
            "stats" => {
                let state = self.state.lock().unwrap();
                let stats = state.stats;

                let (media_packets, repair_packets) = (
                    state.media_packets.len() as u64,
                    state
                        .repair_packets
                        .values()
                        .fold(0, |acc, x| acc + x.len() as u64),
                );

                let s = gst::Structure::builder("application/x-rtp-raptorqdec-stats")
                    .field("received-packets", stats.recv)
                    .field("lost-packets", stats.lost)
                    .field("recovered-packets", stats.recovered)
                    .field("buffered-media-packets", media_packets)
                    .field("buffered-repair-packets", repair_packets)
                    .build();

                s.to_value()
            }
            _ => unimplemented!(),
        }
    }
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for RaptorqDec {}

impl ElementImpl for RaptorqDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP RaptorQ FEC Decoder",
                "RTP RaptorQ FEC Decoding",
                "Performs FEC using RaptorQ (RFC6681, RFC6682)",
                "Tomasz Andrzejak <andreiltd@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("application/x-rtp").build();

            let srcpad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sinkpad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_fec_caps = gst::Caps::builder("application/x-rtp")
                .field("raptor-scheme-id", fecscheme::FEC_SCHEME_ID.to_string())
                // All fmtp parameters from SDP are string in caps, those are
                // required parameters that cannot be expressed as string:
                // .field("kmax", (string) [1, MAX_SOURCE_BLOCK_LEN])
                // .field("t", (string) [1, MAX_ENCODING_SYMBOL_SIZE])
                // .field("repair-window", (string) ANY)
                .build();

            let sinkpad_fec_template = gst::PadTemplate::new(
                "fec_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &sink_fec_caps,
            )
            .unwrap();

            vec![srcpad_template, sinkpad_template, sinkpad_fec_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                self.reset();
            }
            gst::StateChange::PausedToReady => {
                self.stop();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut sinkpad_fec_guard = self.sinkpad_fec.lock().unwrap();

        if sinkpad_fec_guard.is_some() {
            gst::element_imp_error!(
                self,
                gst::CoreError::Pad,
                ["Not accepting more than one FEC stream"]
            );

            return None;
        }

        let sinkpad_fec = gst::Pad::builder_from_template(templ)
            .name_if_some(name)
            .chain_function(|pad, parent, buffer| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.fec_sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Self::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.fec_sink_event(pad, event),
                )
            })
            .iterate_internal_links_function(|pad, parent| {
                Self::catch_panic_pad_function(
                    parent,
                    || gst::Iterator::from_vec(vec![]),
                    |this| this.iterate_internal_links(pad),
                )
            })
            .build();

        sinkpad_fec.set_active(true).unwrap();
        *sinkpad_fec_guard = Some(sinkpad_fec.clone());

        drop(sinkpad_fec_guard);

        self.obj().add_pad(&sinkpad_fec).unwrap();

        Some(sinkpad_fec)
    }

    fn release_pad(&self, _pad: &gst::Pad) {
        let mut pad_guard = self.sinkpad_fec.lock().unwrap();

        if let Some(pad) = pad_guard.take() {
            drop(pad_guard);
            pad.set_active(false).unwrap();
            self.obj().remove_pad(&pad).unwrap();
        }
    }
}

fn fmtp_param_from_caps<T: std::str::FromStr>(
    name: &str,
    caps: &gst::CapsRef,
) -> Result<T, gst::ErrorMessage>
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    caps.structure(0)
        .unwrap()
        .get::<String>(name)
        .map_err(|err| {
            error_msg!(
                gst::CoreError::Caps,
                [
                    "Could not get \"{}\" param from caps {:?}, err: {:?}",
                    name,
                    caps,
                    err
                ]
            )
        })?
        .parse::<T>()
        .map_err(|err| {
            error_msg!(
                gst::CoreError::Caps,
                [
                    "Could not parse \"{}\" param from caps {:?}, err: {:?}",
                    name,
                    caps,
                    err
                ]
            )
        })
}
