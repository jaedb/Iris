// Copyright (C) 2025 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-ts-rtpdtmfsrc
 * @title: ts-rtpdtmfsrc
 * @see_also: dtmfsrc, rtpdtmfdepay, rtpdtmfmux
 *
 * Thread-sharing RTP DTMF (RFC 2833) source.
 *
 * The `ts-rtpdtmfsrc` element generates RTP DTMF (RFC 2833) event packets on request
 * from application. The application communicates the beginning and end of a
 * DTMF event using custom upstream gstreamer events.
 *
 * Compated to `rtpdtmfsrc`, `ts-rtpdtmfsrc` takes advantage of the threadshare runtime,
 * allowing reduced number of threads and context switches when many elements are used.
 *
 * To report a DTMF event, an application must send an event of type `gst::event::CustomUpstream`,
 * having a structure of name "dtmf-event" with fields set according to the following
 * table:
 *
 * * `type` (`G_TYPE_INT` aka `i32`, 0-1): The application uses this field to specify which of
 *   the two methods specified in RFC 2833 to use. The value should be 0 for tones and 1 for
 *   named events. Tones are specified by their frequencies and events are specified
 *   by their number. This element can only take events as input. Do not confuse
 *   with "method" which specified the output.
 *
 * * `number` (`G_TYPE_INT`, 0-15): The event number.
 *
 * * `volume` (`G_TYPE_INT`, 0-36): This field describes the power level of the tone,
 *   expressed in dBm0 after dropping the sign. Power levels range from 0 to -63 dBm0. The range
 *   of valid DTMF is from 0 to -36 dBm0. Can be omitted if start is set to FALSE.
 *
 * * `start` (`G_TYPE_BOOLEAN` aka `bool`, True or False): Whether the event is starting or ending.
 *
 * * `method` (G_TYPE_INT, 1): The method used for sending event, this element will react if this
 *   field is absent or 1.
 *
 * For example, the following code informs the pipeline (and in turn, the
 * `ts-rtpdtmfsrc` element inside the pipeline) about the start of an RTP DTMF named
 * event '1' of volume -25 dBm0:
 *
 * |[
 * let dtmf_start_event = gst::event::CustomUpstream::builder(
 *     gst::Structure::builder("dtmf-event")
 *         .field("type", 1i32)
 *         .field("method", 1i32)
 *         .field("start", true)
 *         .field("number", 1i32)
 *         .field("volume", 25i32)
 *         .build(),
 * )
 * .build();
 *
 * pipeline.send_event(dtmf_start_event)
 * ]|
 *
 * When a DTMF tone actually starts or stop, a "dtmf-event-processed"
 * element #GstMessage with the same fields as the "dtmf-event"
 * #GstEvent that was used to request the event. Also, if any event
 * has not been processed when the element goes from the PAUSED to the
 * READY state, then a "dtmf-event-dropped" message is posted on the
 * #GstBus in the order that they were received.
 *
 * Since: plugins-rs-0.14.0
 */
use futures::channel::mpsc;
use futures::prelude::*;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use rand::prelude::*;
use rtp_types::RtpPacketBuilder;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{self, timer, PadSrc, Task};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-rtpdtmfsrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing RTP DTMF src"),
    )
});

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

const DEFAULT_CLOCK_RATE: u32 = 8_000;
const DEFAULT_PACKET_REDUNDANCY: u8 = MIN_PACKET_REDUNDANCY;
const DEFAULT_PT: u8 = 96;
const DEFAULT_PTIME: gst::ClockTime = gst::ClockTime::from_mseconds(40);
const DEFAULT_SEQNUM_OFFSET: Option<u16> = None;
const DEFAULT_SSRC: Option<u32> = None;
const DEFAULT_TIMESTAMP_OFFSET: Option<u32> = None;

const MIN_INTER_DIGIT_INTERVAL: gst::ClockTime = gst::ClockTime::from_mseconds(100);
const MIN_PACKET_REDUNDANCY: u8 = 1;
const MAX_PACKET_REDUNDANCY: u8 = 5;

const DEFAULT_DTMF_EVT_CHAN_CAPACITY: usize = 4;

static DEFAULT_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    gst::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", gst::IntRange::new(96, 127))
        .field("clock-rate", gst::IntRange::new(0, i32::MAX))
        .field("encoding-name", "TELEPHONE-EVENT")
        .build()
});

#[derive(Debug, Clone)]
struct Settings {
    context: String,
    context_wait: Duration,
    timestamp: u32,
    seqnum: u32,
    timestamp_offset: Option<u32>,
    seqnum_offset: Option<u16>,
    clock_rate: u32,
    ssrc: Option<u32>,
    pt: u8,
    packet_redundancy: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            timestamp: 0,
            seqnum: 0,
            timestamp_offset: DEFAULT_TIMESTAMP_OFFSET,
            seqnum_offset: DEFAULT_SEQNUM_OFFSET,
            clock_rate: DEFAULT_CLOCK_RATE,
            ssrc: DEFAULT_SSRC,
            pt: DEFAULT_PT,
            packet_redundancy: DEFAULT_PACKET_REDUNDANCY,
        }
    }
}

#[derive(Debug)]
struct RTPDTMFSrcTask {
    elem: super::RTPDTMFSrc,
    dtmf_evt_rx: mpsc::Receiver<DTMFEvent>,
    stream_start_pending: bool,
    segment_pending: bool,

    ptime: gst::ClockTime,

    last_stop: Option<gst::ClockTime>,
    timestamp: gst::ClockTime,
    dtmf_payload: Option<DTMFPayload>,

    next_wake_up: Option<timer::Oneshot>,
    rtp_ts_offset: u32,
    rtp_ts: u32,

    seqnum: u16,
    ssrc: u32,

    pt: u8,
    clock_rate: u32,

    packet_redundancy: u8,
    pending_msg: VecDeque<gst::Message>,
}

impl RTPDTMFSrcTask {
    fn new(elem: super::RTPDTMFSrc, dtmf_evt_rx: mpsc::Receiver<DTMFEvent>) -> Self {
        let imp = elem.imp();

        let ptime = *imp.ptime.lock().unwrap();

        let params = imp.get_configured_or_random_params();

        let settings = imp.settings.lock().unwrap();
        let clock_rate = settings.clock_rate;
        let pt = settings.pt;
        let packet_redundancy = settings.packet_redundancy;
        drop(settings);

        RTPDTMFSrcTask {
            elem,
            dtmf_evt_rx,
            stream_start_pending: true,
            segment_pending: true,

            last_stop: None,
            timestamp: gst::ClockTime::ZERO,
            dtmf_payload: None,

            next_wake_up: None,
            rtp_ts_offset: params.rtp_ts_offset,
            rtp_ts: 0,

            seqnum: params.seqnum_offset,
            ptime,

            clock_rate,
            ssrc: params.ssrc,
            pt,

            packet_redundancy,
            pending_msg: VecDeque::new(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum PacketPlace {
    First,
    Intermediate,
    Last,
}

#[derive(Debug)]
struct DTMFPayload {
    event_nb: u8,
    volume: u8,
    duration: u16,
    place: PacketPlace,
    redundancy_count: u8,
}

impl DTMFPayload {
    fn is_last(&self) -> bool {
        matches!(self.place, PacketPlace::Last)
    }
}

#[derive(Clone, Debug)]
enum DTMFEventStatus {
    Processed,
    Dropped,
}

/// DTMF event
impl RTPDTMFSrcTask {
    fn start_dtmf_payload(&mut self, event_nb: u8, volume: u8) {
        assert!(self.dtmf_payload.is_none());

        let start_timestamp = self.last_stop.unwrap_or_else(|| {
            self.elem
                .current_running_time()
                .expect("element in Playing state")
        });

        self.timestamp = gst::ClockTime::max(self.timestamp, start_timestamp);

        self.dtmf_payload = Some(DTMFPayload {
            event_nb,
            volume,
            duration: (self.ptime.nseconds())
                .mul_div_floor(self.clock_rate as u64, *gst::ClockTime::SECOND)
                .unwrap() as u16,
            place: PacketPlace::First,
            redundancy_count: self.packet_redundancy,
        });

        self.rtp_ts = self.rtp_ts_offset.wrapping_add(
            start_timestamp
                .nseconds()
                .mul_div_floor(self.clock_rate as u64, *gst::ClockTime::SECOND)
                .unwrap() as u32,
        );
    }

    fn create_next_rtp_packet(&mut self) -> Option<gst::Buffer> {
        let dtmf_pay = self.dtmf_payload.as_mut()?;

        let (is_first, end_mask) = match dtmf_pay.place {
            PacketPlace::First => {
                dtmf_pay.place = PacketPlace::Intermediate;
                (true, 0x00)
            }
            PacketPlace::Intermediate => (false, 0x00),
            PacketPlace::Last => (false, 1 << 7),
        };

        let mut rtp_packet = vec![0u8; 16];
        RtpPacketBuilder::new()
            .marker_bit(is_first)
            .payload_type(self.pt)
            .sequence_number(self.seqnum)
            .timestamp(self.rtp_ts)
            .ssrc(self.ssrc)
            .payload(
                // See RFC2833 § 3.5
                [
                    dtmf_pay.event_nb,
                    end_mask | dtmf_pay.volume,
                    (dtmf_pay.duration >> 8) as u8,
                    (dtmf_pay.duration & 0xff) as u8,
                ]
                .as_ref(),
            )
            .write_into(&mut rtp_packet)
            .unwrap_or_else(|err| {
                panic!("Failed to write RTP packet for {dtmf_pay:?}: {err}");
            });

        let mut rtp_buffer = gst::Buffer::from_mut_slice(rtp_packet);
        {
            let rtp_buffer = rtp_buffer.get_mut().unwrap();

            rtp_buffer.set_pts(self.timestamp);

            let duration = if dtmf_pay.redundancy_count > 1 {
                gst::ClockTime::ZERO
            } else if dtmf_pay.is_last() {
                let inter_digit_remainder = MIN_INTER_DIGIT_INTERVAL % self.ptime;
                if inter_digit_remainder.is_zero() {
                    self.ptime
                } else {
                    self.ptime + MIN_INTER_DIGIT_INTERVAL + self.ptime - inter_digit_remainder
                }
            } else {
                self.ptime
            };

            rtp_buffer.set_duration(duration);

            gst::log!(
                CAT,
                obj = self.elem,
                "Created buffer with DTMF event {} duration {duration} pts {} RTP ts {}",
                dtmf_pay.event_nb,
                self.timestamp,
                self.rtp_ts,
            );

            // Duration of DTMF payload for the NEXT packet
            // not updated for redundant packets.
            if dtmf_pay.redundancy_count <= 1 {
                dtmf_pay.duration += (self.ptime.nseconds())
                    .mul_div_floor(self.clock_rate as u64, *gst::ClockTime::SECOND)
                    .unwrap() as u16;
            }

            dtmf_pay.redundancy_count = dtmf_pay.redundancy_count.saturating_sub(1);
            if dtmf_pay.is_last() && dtmf_pay.redundancy_count == 0 {
                self.dtmf_payload = None;
            } else {
                self.timestamp.opt_add_assign(duration);
            }
        }

        self.seqnum = self.seqnum.wrapping_add(1);

        Some(rtp_buffer)
    }

    fn prepare_message(&self, dtmf_evt: &DTMFEvent, evt_status: DTMFEventStatus) -> gst::Message {
        use DTMFEventStatus::*;
        let struct_builder = gst::Structure::builder(match evt_status {
            Processed => "dtmf-event-processed",
            Dropped => "dtmf-event-dropped",
        })
        .field("type", 1i32)
        .field("method", 1i32);

        use DTMFEvent::*;
        gst::message::Element::builder(match *dtmf_evt {
            Start { number, volume, .. } => struct_builder
                .field("start", true)
                .field("number", number as i32)
                .field("volume", volume as i32)
                .build(),
            Stop { .. } => struct_builder.field("start", false).build(),
        })
        .src(&self.elem)
        .build()
    }

    fn dtmf_evt_to_item(&self, dtmf_evt: Option<DTMFEvent>) -> Result<TaskItem, gst::FlowError> {
        let Some(dtmf_evt) = dtmf_evt else {
            gst::error!(CAT, obj = self.elem, "DTMF event channel is broken");
            gst::element_error!(
                self.elem,
                gst::CoreError::Failed,
                ["DTMF event Queue is broken"]
            );
            return Err(gst::FlowError::Error);
        };

        Ok(TaskItem::Event(dtmf_evt))
    }
}

#[derive(Debug)]
enum TaskItem {
    Event(DTMFEvent),
    Timer,
}

impl TaskImpl for RTPDTMFSrcTask {
    type Item = TaskItem;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.elem
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.elem, "Starting Task");

        if self.stream_start_pending {
            gst::debug!(CAT, obj = self.elem, "Pushing initial events");

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            self.elem.imp().srcpad.push_event(stream_start).await;
            self.stream_start_pending = false;
        }

        self.negotiate().map_err(|err| {
            gst::error_msg!(
                gst::CoreError::Negotiation,
                ["Caps negotiation failed: {err}"]
            )
        })?;

        if self.segment_pending {
            let segment = gst::FormattedSegment::<gst::format::Time>::new();
            self.elem
                .imp()
                .srcpad
                .push_event(gst::event::Segment::new(&segment))
                .await;

            self.segment_pending = false;
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.elem, "Stopping Task");

        self.flush().await;
        self.stream_start_pending = true;
        self.segment_pending = true;

        Ok(())
    }

    async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.elem, "Starting task flush");

        self.flush().await;
        self.segment_pending = true;

        gst::log!(CAT, obj = self.elem, "Task flush started");
        Ok(())
    }

    async fn try_next(&mut self) -> Result<TaskItem, gst::FlowError> {
        Ok(if let Some(ref mut next_wake_up) = self.next_wake_up {
            futures::select! {
                _ = next_wake_up.fuse() => TaskItem::Timer,
                dtmf_evt = self.dtmf_evt_rx.next() => {
                    self.dtmf_evt_to_item(dtmf_evt)?
                }
            }
        } else {
            let dtmf_evt = self.dtmf_evt_rx.next().await;
            self.dtmf_evt_to_item(dtmf_evt)?
        })
    }

    async fn handle_item(&mut self, item: TaskItem) -> Result<(), gst::FlowError> {
        gst::debug!(CAT, obj = self.elem, "Handling {item:?}");

        use DTMFEvent::*;
        use DTMFEventStatus::*;
        match item {
            TaskItem::Event(dtmf_evt) => match dtmf_evt {
                Start {
                    number,
                    volume,
                    last_stop,
                } => {
                    self.last_stop = last_stop;

                    if self.dtmf_payload.is_some() {
                        gst::warning!(
                            CAT,
                            obj = self.elem,
                            "Received two consecutive DTMF start events"
                        );
                        self.elem
                            .post_message(self.prepare_message(&dtmf_evt, Dropped))
                            .expect("element in Playing state");
                        return Ok(());
                    };

                    self.start_dtmf_payload(number, volume);
                    self.pending_msg
                        .push_back(self.prepare_message(&dtmf_evt, Processed));
                }
                Stop { last_stop } => {
                    self.last_stop = last_stop;

                    let Some(dtmf_payload) = self.dtmf_payload.as_mut() else {
                        gst::warning!(
                            CAT,
                            obj = self.elem,
                            "Received a DTMF stop event while already stopped"
                        );
                        self.elem
                            .post_message(self.prepare_message(&dtmf_evt, Dropped))
                            .expect("element in Playing state");
                        return Ok(());
                    };

                    dtmf_payload.place = PacketPlace::Last;
                    dtmf_payload.redundancy_count = self.packet_redundancy;
                    self.pending_msg
                        .push_back(self.prepare_message(&dtmf_evt, Processed));
                }
            },
            TaskItem::Timer => {
                self.next_wake_up = None;
                self.push_next_packet().await?;
            }
        }

        self.next_wake_up = self.dtmf_payload.as_ref().map(|_| {
            let now = self
                .elem
                .current_running_time()
                .expect("element in Playing state");

            gst::log!(
                CAT,
                obj = self.elem,
                "Setting next wake up for timestamp {}, now {now}",
                self.timestamp
            );

            timer::delay_for(Duration::from_nanos(*(self.timestamp.saturating_sub(now))))
        });

        Ok(())
    }
}

impl RTPDTMFSrcTask {
    async fn push_next_packet(&mut self) -> Result<(), gst::FlowError> {
        while let Some(msg) = self.pending_msg.pop_front() {
            self.elem
                .post_message(msg)
                .expect("element in Playing state");
        }

        if let Some(rtp_buffer) = self.create_next_rtp_packet() {
            gst::debug!(CAT, obj = self.elem, "Pushing RTP packet {rtp_buffer:?}");
            self.elem.imp().srcpad.push(rtp_buffer).await?;
        }

        gst::log!(CAT, obj = self.elem, "Pushed RTP packet");

        Ok(())
    }

    async fn flush(&mut self) {
        if let Some(ref mut dtmf_payload) = self.dtmf_payload {
            dtmf_payload.place = PacketPlace::Last;
            dtmf_payload.redundancy_count = self.packet_redundancy;

            let _ = self.push_next_packet().await;

            while let Ok(Some(dtmf_evt)) = self.dtmf_evt_rx.try_next() {
                let _ = self
                    .elem
                    .post_message(self.prepare_message(&dtmf_evt, DTMFEventStatus::Dropped));
            }
        }

        let imp = self.elem.imp();

        imp.last_event_was_start.store(false, Ordering::SeqCst);

        self.next_wake_up = None;
        self.last_stop = None;
        self.timestamp = gst::ClockTime::ZERO;

        let params = imp.get_configured_or_random_params();
        self.rtp_ts_offset = params.rtp_ts_offset;
        self.rtp_ts = params.rtp_ts_offset;
        self.seqnum = params.seqnum_offset;
        self.ssrc = params.ssrc;
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
enum CapsNegotiationError {
    #[error("Could not intersect with peer caps {}", .0)]
    CapsIntersection(gst::Caps),

    #[error("Peer's '{field}' with value {value} is out of range")]
    OutOfRange { field: String, value: i64 },

    #[error("Failed to fixate '{field}' with {value} in {caps}")]
    Fixate {
        field: String,
        value: i64,
        caps: gst::Caps,
    },
}

/// Caps negotiation
impl RTPDTMFSrcTask {
    fn negotiate(&mut self) -> Result<(), CapsNegotiationError> {
        let imp = self.elem.imp();
        let pad = imp.srcpad.gst_pad();

        let src_tmpl_caps = pad.pad_template_caps();
        let peercaps = pad.peer_query_caps(Some(&src_tmpl_caps));
        gst::log!(CAT, imp = imp, "Peer returned {peercaps:?}");

        let newcaps = if peercaps.is_empty() {
            let srccaps = gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("payload", self.pt)
                .field("ssrc", self.ssrc)
                .field("timestamp-offset", self.rtp_ts_offset)
                .field("clock-rate", self.clock_rate)
                .field("seqnum-offset", self.seqnum as u32)
                .field("encoding-name", "TELEPHONE-EVENT")
                .build();

            gst::debug!(CAT, obj = self.elem, "No peer caps, using {srccaps}");
            srccaps
        } else {
            let mut inter =
                peercaps.intersect_with_mode(&src_tmpl_caps, gst::CapsIntersectMode::First);
            if inter.is_empty() {
                return Err(CapsNegotiationError::CapsIntersection(peercaps));
            }

            {
                let inter = inter.make_mut();
                let s = inter.structure_mut(0).expect("not empty");

                match s.get_optional::<i32>("payload") {
                    Ok(Some(pt)) => {
                        let pt = pt
                            .try_into()
                            .map_err(|_| CapsNegotiationError::OutOfRange {
                                field: "pt".to_string(),
                                value: pt as i64,
                            })?;

                        gst::log!(CAT, imp = imp, "Using peer pt {pt}");
                        self.pt = pt;
                    }
                    Ok(None) => {
                        s.set("payload", self.pt as i32);
                        gst::log!(CAT, imp = imp, "Using internal pt {}", self.pt);
                    }
                    Err(_) => {
                        if s.fixate_field_nearest_int("payload", self.pt as i32) {
                            self.pt = s.get::<i32>("payload").unwrap() as u8;

                            gst::log!(CAT, imp = imp, "Using fixated pt {}", self.pt);
                        } else {
                            return Err(CapsNegotiationError::Fixate {
                                field: "payload".to_string(),
                                value: self.pt as i64,
                                caps: peercaps,
                            });
                        }
                    }
                }

                match s.get_optional::<i32>("clock-rate") {
                    Ok(Some(clock_rate)) => {
                        let clock_rate = clock_rate.try_into().map_err(|_| {
                            CapsNegotiationError::OutOfRange {
                                field: "clock-rate".to_string(),
                                value: clock_rate as i64,
                            }
                        })?;

                        gst::log!(CAT, imp = imp, "Using peer clock-rate {clock_rate}");
                        self.clock_rate = clock_rate;
                    }
                    Ok(None) => {
                        s.set("clock-rate", self.clock_rate as i32);
                        gst::log!(
                            CAT,
                            imp = imp,
                            "Using internal clock-rate {}",
                            self.clock_rate
                        );
                    }
                    Err(_) => {
                        if s.fixate_field_nearest_int("clock-rate", self.clock_rate as i32) {
                            self.clock_rate = s.get::<i32>("clock-rate").unwrap() as u32;
                            gst::log!(
                                CAT,
                                imp = imp,
                                "Using fixated clock-rate {}",
                                self.clock_rate
                            );
                        } else {
                            return Err(CapsNegotiationError::Fixate {
                                field: "clock-rate".to_string(),
                                value: self.clock_rate as i64,
                                caps: peercaps,
                            });
                        }
                    }
                }

                match s.get_optional::<u32>("ssrc") {
                    Ok(Some(ssrc)) => {
                        gst::log!(CAT, imp = imp, "Using peer ssrc {ssrc:#08x}");
                        self.ssrc = ssrc;
                    }
                    other => {
                        if let Err(err) = other {
                            gst::warning!(
                                CAT,
                                imp = imp,
                                "Invalid type for peer 'ssrc' in {s}: {err}"
                            );
                        }
                        s.set("ssrc", self.ssrc);
                        gst::log!(CAT, imp = imp, "Using internal ssrc {}", self.ssrc);
                    }
                }

                match s.get_optional::<u32>("timestamp-offset") {
                    Ok(Some(timestamp_offset)) => {
                        gst::log!(
                            CAT,
                            imp = imp,
                            "Using peer timestamp-offset {timestamp_offset}"
                        );
                        self.rtp_ts_offset = timestamp_offset;
                    }
                    other => {
                        if let Err(err) = other {
                            // Would be cool to be able to fixate uint
                            gst::warning!(
                                CAT,
                                imp = imp,
                                "Invalid type for peer 'timestamp-offset' in {s}: {err}"
                            );
                        }
                        s.set("timestamp-offset", self.rtp_ts_offset);
                        gst::log!(
                            CAT,
                            imp = imp,
                            "Using internal timestamp-offset {}",
                            self.rtp_ts_offset
                        );
                    }
                }

                match s.get_optional::<u32>("seqnum-offset") {
                    Ok(Some(seqnum_offset)) => {
                        let seqnum_offset = seqnum_offset.try_into().map_err(|_| {
                            CapsNegotiationError::OutOfRange {
                                field: "seqnum-offset".to_string(),
                                value: seqnum_offset as i64,
                            }
                        })?;

                        gst::log!(CAT, imp = imp, "Using peer seqnum-offset {seqnum_offset}");
                        self.seqnum = seqnum_offset;
                    }
                    other => {
                        if let Err(err) = other {
                            // Would be cool to be able to fixate uint
                            gst::warning!(
                                CAT,
                                imp = imp,
                                "Invalid type for peer 'seqnum-offset' in {s}: {err}"
                            );
                        }
                        s.set("seqnum-offset", self.seqnum as u32);
                        gst::log!(
                            CAT,
                            imp = imp,
                            "Using internal seqnum-offset {}",
                            self.seqnum
                        );
                    }
                }

                if let Ok(Some(ptime)) = s.get_optional::<u32>("ptime") {
                    gst::log!(CAT, imp = imp, "Using peer ptime {ptime}");
                    self.ptime = gst::ClockTime::from_mseconds(ptime as u64);
                    *imp.ptime.lock().unwrap() = self.ptime;
                } else {
                    match s.get_optional::<u32>("maxptime") {
                        Ok(Some(maxptime)) => {
                            gst::log!(CAT, imp = imp, "Using peer maxptime {maxptime}");
                            self.ptime = gst::ClockTime::from_mseconds(maxptime as u64);
                            *imp.ptime.lock().unwrap() = self.ptime;
                        }
                        other => {
                            if let Err(err) = other {
                                // Would be cool to be able to fixate uint
                                gst::warning!(
                                    CAT,
                                    imp = imp,
                                    "Invalid type for peer 'ptime' / 'maxptime' in {s}: {err}"
                                );
                                s.remove_field("maxptime");
                            }
                            s.set("ptime", self.ptime);
                            gst::log!(CAT, imp = imp, "Using internal ptime {}", self.ptime);
                        }
                    }
                }
            }

            gst::debug!(CAT, obj = self.elem, "Processed peer caps => {inter}");
            inter
        };

        pad.push_event(gst::event::Caps::new(&newcaps));

        Ok(())
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
enum DTMFEventError {
    #[error("Not a DTMF event")]
    NotDTMFEvent,

    #[error("Unsupported DTMF event type {}", .0)]
    UnsupportedType(i32),

    #[error("Unsupported DTMF event method {}", .0)]
    UnsupportedMethod(i32),

    #[error("Field {field}: {err}")]
    FieldError { field: String, err: String },
}

#[derive(Clone, Debug)]
enum DTMFEvent {
    Start {
        number: u8,
        volume: u8,
        last_stop: Option<gst::ClockTime>,
    },
    Stop {
        last_stop: Option<gst::ClockTime>,
    },
}

impl DTMFEvent {
    fn try_parse(event: &gst::event::CustomUpstream) -> Result<DTMFEvent, DTMFEventError> {
        use DTMFEventError::*;

        let Some(s) = event.structure() else {
            return Err(NotDTMFEvent);
        };
        if s.name() != "dtmf-event" {
            return Err(NotDTMFEvent);
        }

        let evt_type = s.get::<i32>("type").map_err(|err| FieldError {
            field: "type".to_string(),
            err: err.to_string(),
        })?;
        if evt_type != 1i32 {
            return Err(UnsupportedType(evt_type));
        }

        let start = s.get::<bool>("start").map_err(|err| FieldError {
            field: "start".to_string(),
            err: err.to_string(),
        })?;

        let method = s.get_optional::<i32>("method").map_err(|err| FieldError {
            field: "method".to_string(),
            err: err.to_string(),
        })?;
        if method.is_some_and(|method| method != 1i32) {
            return Err(UnsupportedMethod(method.expect("checked above")));
        }

        let last_stop = s
            .get_optional::<gst::ClockTime>("last-stop")
            .map_err(|err| FieldError {
                field: "last-stop".to_string(),
                err: err.to_string(),
            })?;

        let dtmf_evt = if start {
            let number = s.get::<i32>("number").map_err(|err| FieldError {
                field: "number".to_string(),
                err: err.to_string(),
            })?;
            if !(0..=15).contains(&number) {
                return Err(FieldError {
                    field: "number".to_string(),
                    err: format!("{number} is out of range [0, 15]"),
                });
            }

            let volume = s.get::<i32>("volume").map_err(|err| FieldError {
                field: "volume".to_string(),
                err: err.to_string(),
            })?;
            if !(0..=36).contains(&volume) {
                return Err(FieldError {
                    field: "volume".to_string(),
                    err: format!("{volume} is out of range [0, 36]"),
                });
            }

            DTMFEvent::Start {
                number: number as u8,
                volume: volume as u8,
                last_stop,
            }
        } else {
            DTMFEvent::Stop { last_stop }
        };

        Ok(dtmf_evt)
    }

    fn is_start(&self) -> bool {
        matches!(*self, DTMFEvent::Start { .. })
    }
}

#[derive(Debug)]
pub struct ConfiguredOrRandomParams {
    rtp_ts_offset: u32,
    seqnum_offset: u16,
    ssrc: u32,
}

#[derive(Debug)]
pub struct RTPDTMFSrc {
    srcpad: PadSrc,
    task: Task,
    settings: Mutex<Settings>,
    last_event_was_start: AtomicBool,
    ptime: Mutex<gst::ClockTime>,
    dtmf_evt_tx: Mutex<Option<mpsc::Sender<DTMFEvent>>>,
}

impl RTPDTMFSrc {
    /// Handles the DTMF event
    ///
    /// Returns `true` if it could be handled, `false` otherwise.
    fn handle_maybe_dtmf_event(&self, event: &gst::Event) -> bool {
        gst::log!(CAT, imp = self, "Handling {event:?}");

        let gst::EventView::CustomUpstream(evt) = event.view() else {
            gst::log!(CAT, imp = self, "Not Handling unknown {event:?}");
            return false;
        };

        let dtmf_evt = match DTMFEvent::try_parse(evt) {
            Ok(dtmf_evt) => dtmf_evt,
            Err(DTMFEventError::NotDTMFEvent) => {
                return false;
            }
            Err(err) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to parse incoming DTMF event: {err}"
                );
                return false;
            }
        };

        let is_start = dtmf_evt.is_start();
        if is_start == self.last_event_was_start.load(Ordering::SeqCst) {
            gst::error!(
                CAT,
                imp = self,
                "Unexpected {} event",
                if is_start { "start" } else { "end" },
            );
            return false;
        }

        if let Err(err) = self
            .dtmf_evt_tx
            .lock()
            .unwrap()
            .as_mut()
            .expect("set in prepare")
            .try_send(dtmf_evt)
        {
            if err.is_full() {
                // This probably means the app is spamming us
                let dtmf_event = err.into_inner();
                gst::error!(
                    CAT,
                    imp = self,
                    "DTMF event channel is full => dropping {dtmf_event:?}"
                );
            } else {
                gst::error!(CAT, imp = self, "DTMF event channel is broken");
                gst::element_error!(
                    self.obj(),
                    gst::CoreError::Failed,
                    ["DTMF event Queue is broken"]
                );
            }

            return false;
        }

        self.last_event_was_start.store(is_start, Ordering::SeqCst);

        true
    }

    fn get_configured_or_random_params(&self) -> ConfiguredOrRandomParams {
        let mut rng = rand::rng();

        let settings = self.settings.lock().unwrap();

        ConfiguredOrRandomParams {
            rtp_ts_offset: settings
                .timestamp_offset
                .unwrap_or_else(|| rng.random::<u32>()),
            seqnum_offset: settings
                .seqnum_offset
                .unwrap_or_else(|| rng.random::<u16>()),
            ssrc: settings.ssrc.unwrap_or_else(|| rng.random::<u32>()),
        }
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap();
        let context =
            runtime::Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;
        drop(settings);

        let (dtmf_evt_tx, dtmf_evt_rx) = mpsc::channel(DEFAULT_DTMF_EVT_CHAN_CAPACITY);
        *self.dtmf_evt_tx.lock().unwrap() = Some(dtmf_evt_tx);
        self.task
            .prepare(
                RTPDTMFSrcTask::new(self.obj().clone(), dtmf_evt_rx),
                context,
            )
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");

        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                *elem.imp().dtmf_evt_tx.lock().unwrap() = None;

                gst::debug!(CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");

        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                {
                    let imp = elem.imp();
                    let mut settings = imp.settings.lock().unwrap();
                    settings.timestamp = 0;
                    settings.seqnum = 0;
                }

                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Started");
                }
            })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RTPDTMFSrc {
    const NAME: &'static str = "GstTsRTPDTMFSrc";
    type Type = super::RTPDTMFSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            srcpad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                RTPDTMFSrcPadHandler,
            ),
            task: Task::default(),
            settings: Default::default(),
            last_event_was_start: AtomicBool::new(false),
            ptime: Mutex::new(DEFAULT_PTIME),
            dtmf_evt_tx: Mutex::new(None),
        }
    }
}

impl ObjectImpl for RTPDTMFSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .readwrite()
                    .construct_only()
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .readwrite()
                    .construct_only()
                    .build(),
                glib::ParamSpecUInt::builder("timestamp")
                    .nick("Timestamp")
                    .blurb("The RTP timestamp of the last processed packet")
                    .minimum(0)
                    .maximum(u32::MAX)
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("seqnum")
                    .nick("Sequence number")
                    .blurb("The RTP Sequence number of the last processed packet")
                    .minimum(0)
                    .maximum(u32::MAX)
                    .read_only()
                    .build(),
                glib::ParamSpecInt64::builder("timestamp-offset")
                    .nick("Timestamp Offset")
                    .blurb("Offset to add to all outgoing timestamps (-1 = random)")
                    .minimum(-1)
                    .maximum(u32::MAX as i64)
                    .default_value(DEFAULT_TIMESTAMP_OFFSET.map_or(-1, |val| val as i64))
                    .build(),
                glib::ParamSpecInt::builder("seqnum-offset")
                    .nick("Sequence Number Offset")
                    .blurb("Offset to add to all outgoing seqnum (-1 => random)")
                    .minimum(-1)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_SEQNUM_OFFSET.map_or(-1i32, |val| val as i32))
                    .build(),
                glib::ParamSpecUInt::builder("clock-rate")
                    .nick("Clock-rate")
                    .blurb("The clock-rate at which to generate DTMF packets")
                    .minimum(0)
                    .maximum(u32::MAX)
                    .default_value(DEFAULT_CLOCK_RATE)
                    .build(),
                glib::ParamSpecInt64::builder("ssrc")
                    .nick("Synchronization Source (SSRC)")
                    .blurb("The SSRC of the packets (-1 => random)")
                    .minimum(-1)
                    .maximum(u32::MAX as i64)
                    .default_value(DEFAULT_SSRC.map_or(-1, |val| val as i64))
                    .build(),
                glib::ParamSpecUInt::builder("pt")
                    .nick("Payload Type")
                    .blurb("The payload type of the packets")
                    .minimum(0)
                    .maximum(0x80)
                    .default_value(DEFAULT_PT as u32)
                    .build(),
                glib::ParamSpecUInt::builder("packet-redundancy")
                    .nick("Packet Redundancy")
                    .blurb("Number of packets to send to indicate start and stop DTMF events")
                    .minimum(MIN_PACKET_REDUNDANCY as u32)
                    .maximum(MAX_PACKET_REDUNDANCY as u32)
                    .default_value(DEFAULT_PACKET_REDUNDANCY as u32)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .unwrap()
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(value.get::<u32>().unwrap().into());
            }
            "timestamp-offset" => {
                settings.timestamp_offset = value.get::<i64>().unwrap().try_into().ok();
            }
            "seqnum-offset" => {
                settings.seqnum_offset = value.get::<i32>().unwrap().try_into().ok();
            }
            "clock-rate" => {
                settings.clock_rate = value.get::<u32>().unwrap();
            }
            "ssrc" => {
                settings.ssrc = value.get::<i64>().unwrap().try_into().ok();
            }
            "pt" => {
                settings.pt = value.get::<u32>().unwrap().try_into().unwrap();
            }
            "packet-redundancy" => {
                settings.packet_redundancy = value.get::<u32>().unwrap().try_into().unwrap();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "timestamp" => settings.timestamp.to_value(),
            "seqnum" => settings.seqnum.to_value(),
            "timestamp-offset" => settings.timestamp_offset.map_or(-1, i64::from).to_value(),
            "seqnum-offset" => settings.seqnum_offset.map_or(-1i32, i32::from).to_value(),
            "clock-rate" => settings.clock_rate.to_value(),
            "ssrc" => settings.ssrc.map_or(-1, i64::from).to_value(),
            "pt" => (settings.pt as u32).to_value(),
            "packet-redundancy" => (settings.packet_redundancy as u32).to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.srcpad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for RTPDTMFSrc {}

impl ElementImpl for RTPDTMFSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing RTP DTMF source",
                "Source/Network/RTP",
                "Thread-sharing RTP DTMF packet (RFC2833) source",
                "François Laignel <francois@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        gst::log!(CAT, imp = self, "Got {event:?}");

        if self.handle_maybe_dtmf_event(&event) {
            true
        } else {
            self.parent_send_event(event)
        }
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &DEFAULT_CAPS,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {transition:?}");

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToPlaying => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }
}

#[derive(Clone, Debug)]
struct RTPDTMFSrcPadHandler;

impl PadSrcHandler for RTPDTMFSrcPadHandler {
    type ElementImpl = RTPDTMFSrc;

    fn src_event(self, pad: &gst::Pad, imp: &Self::ElementImpl, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling {event:?}");

        use gst::EventView::*;
        match event.view() {
            FlushStart(..) => imp.task.flush_start().block_on_or_add_subtask(pad).is_ok(),
            FlushStop(..) => imp.task.flush_stop().block_on_or_add_subtask(pad).is_ok(),
            Reconfigure(..) => true,
            Latency(..) => true,
            _ => imp.handle_maybe_dtmf_event(&event),
        }
    }

    fn src_query(self, pad: &gst::Pad, imp: &Self::ElementImpl, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Received {query:?}");

        if let gst::QueryViewMut::Latency(q) = query.view_mut() {
            let latency = *imp.ptime.lock().unwrap()
                // timers can be up to 1/2 x context-wait late
                + gst::ClockTime::from_nseconds(
                    imp.settings.lock().unwrap().context_wait.as_nanos() as u64,
                ) / 2;

            gst::debug!(CAT, imp = imp, "Reporting latency of {latency}");
            q.set(true, latency, gst::ClockTime::NONE);

            return true;
        }

        gst::Pad::query_default(pad, Some(&*imp.obj()), query)
    }
}
