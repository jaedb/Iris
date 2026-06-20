// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::{Duration, Instant, SystemTime},
};

use rtcp_types::{ReportBlock, ReportBlockBuilder};

use crate::utils::ExtendedSeqnum;

use super::{
    session::KeyUnitRequestType,
    time::{system_time_to_ntp_time_u64, NtpTime},
};

use gst::prelude::MulDiv;

pub const DEFAULT_PROBATION_N_PACKETS: usize = 2;
pub const DEFAULT_MAX_DROPOUT: u32 = 3000;
pub const DEFAULT_MAX_MISORDER: u32 = 100;

const BITRATE_WINDOW: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy)]
pub struct Rb {
    ssrc: u32,
    /// fraction out of 256 of packets lost since the last Rb
    fraction_lost: u8,
    /// signed 24-bit number of expected packets - received packets (including duplicates and late
    /// packets)
    cumulative_lost: u32,
    extended_sequence_number: u32,
    /// jitter in clock rate units
    jitter: u32,
    /// 16.16 fixed point ntp time
    last_sr: u32,
    /// 16.16 fixed point ntp duration
    delay_since_last_sr: u32,
}

impl Rb {
    pub fn fraction_lost(&self) -> u8 {
        self.fraction_lost
    }

    pub fn cumulative_lost(&self) -> i32 {
        if self.cumulative_lost & 0x800000 > 0 {
            -((self.cumulative_lost & 0x7fffff) as i32)
        } else {
            self.cumulative_lost as i32
        }
    }

    pub fn extended_sequence_number(&self) -> u32 {
        self.extended_sequence_number
    }

    pub fn jitter(&self) -> u32 {
        self.jitter
    }

    pub fn last_sr_ntp_time(&self) -> u32 {
        self.last_sr
    }

    pub fn delay_since_last_sr(&self) -> u32 {
        self.delay_since_last_sr
    }
}

impl From<ReportBlock<'_>> for Rb {
    fn from(value: ReportBlock) -> Self {
        Self {
            ssrc: value.ssrc(),
            fraction_lost: value.fraction_lost(),
            cumulative_lost: value.cumulative_lost(),
            extended_sequence_number: value.extended_sequence_number(),
            jitter: value.interarrival_jitter(),
            last_sr: value.last_sender_report_timestamp(),
            delay_since_last_sr: value.delay_since_last_sender_report_timestamp(),
        }
    }
}

impl From<Rb> for ReportBlockBuilder {
    fn from(value: Rb) -> Self {
        ReportBlock::builder(value.ssrc)
            .fraction_lost(value.fraction_lost)
            .cumulative_lost(value.cumulative_lost)
            .extended_sequence_number(value.extended_sequence_number)
            .interarrival_jitter(value.jitter)
            .last_sender_report_timestamp(value.last_sr)
            .delay_since_last_sender_report_timestamp(value.delay_since_last_sr)
    }
}

#[derive(Debug)]
pub(crate) struct Source {
    ssrc: u32,
    state: SourceState,
    sdes: HashMap<u8, String>,
    last_activity: Instant,
    payload_type: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    Probation(usize),
    Normal,
    Bye,
}

impl Source {
    fn new(ssrc: u32) -> Self {
        Self {
            ssrc,
            state: SourceState::Probation(DEFAULT_PROBATION_N_PACKETS),
            sdes: HashMap::new(),
            last_activity: Instant::now(),
            payload_type: None,
        }
    }

    fn set_state(&mut self, state: SourceState) {
        self.state = state;
    }
}

#[derive(Debug)]
pub struct ReceivedRb {
    pub rb: Rb,
    #[allow(unused)]
    pub receive_time: Instant,
    pub receive_ntp_time: NtpTime,
}

impl ReceivedRb {
    #[allow(unused)]
    fn round_trip_time(&self) -> Duration {
        let rb_send_ntp_time = self.rb.last_sr as u64 + self.rb.delay_since_last_sr as u64;

        // Can't calculate any round trip time
        if rb_send_ntp_time == 0 {
            return Duration::ZERO;
        }

        let mut rb_recv_ntp_time = self.receive_ntp_time.as_u32() as u64;

        if rb_send_ntp_time > rb_recv_ntp_time {
            // 16.16 bit fixed point NTP time wrapped around
            if rb_send_ntp_time - rb_recv_ntp_time > 0x7fff_ffff {
                rb_recv_ntp_time += u32::MAX as u64;
            }
        }

        let diff = rb_recv_ntp_time.saturating_sub(rb_send_ntp_time);
        // Bogus RTT of more than 2*5 seconds, return 1s as a fallback
        if (diff >> 16) > 5 {
            return Duration::from_secs(1);
        }

        let rtt = 2 * diff;
        let rtt_ns = rtt * 1_000_000_000 / 65_536;
        Duration::from_nanos(rtt_ns)
    }
}

#[derive(Debug)]
pub struct LocalSendSource {
    source: Source,
    ext_seqnum: ExtendedSeqnum,
    last_rtp_sent: Option<(u32, Instant)>,
    sent_bytes: u64,
    sent_packets: u64,
    bitrate: Bitrate,
    bye_sent_time: Option<Instant>,
    bye_reason: Option<String>,
    last_sent_sr: Option<Sr>,
    last_received_rb: HashMap<u32, ReceivedRb>,
}

impl LocalSendSource {
    pub(crate) fn new(ssrc: u32) -> Self {
        Self {
            source: Source::new(ssrc),
            ext_seqnum: ExtendedSeqnum::default(),
            last_rtp_sent: None,
            sent_bytes: 0,
            sent_packets: 0,
            bitrate: Bitrate::new(BITRATE_WINDOW),
            bye_sent_time: None,
            bye_reason: None,
            last_sent_sr: None,
            last_received_rb: HashMap::new(),
        }
    }

    pub(crate) fn set_state(&mut self, state: SourceState) {
        self.source.set_state(state);
    }

    pub(crate) fn state(&self) -> SourceState {
        self.source.state
    }

    pub(crate) fn sent_packet(
        &mut self,
        bytes: usize,
        time: Instant,
        seqnum: u16,
        rtp_time: u32,
        payload_type: u8,
    ) {
        self.bitrate.add_entry(bytes, time);

        let _ext_seqnum = self.ext_seqnum.next(seqnum);

        self.source.payload_type = Some(payload_type);

        self.sent_bytes = self.sent_bytes.wrapping_add(bytes as u64);
        self.sent_packets += 1;
        self.last_rtp_sent = Some((rtp_time, time));
    }

    /// Retrieve the last rtp timestamp (and time) that data was sent for this source
    pub fn last_rtp_sent_timestamp(&self) -> Option<(u32, Instant)> {
        self.last_rtp_sent
    }

    /// Retrieve the last seen payload type for this source
    pub fn payload_type(&self) -> Option<u8> {
        self.source.payload_type
    }

    pub(crate) fn bitrate(&self) -> usize {
        self.bitrate.bitrate()
    }

    pub(crate) fn packet_count(&self) -> u64 {
        self.sent_packets
    }

    pub(crate) fn octet_count(&self) -> u64 {
        self.sent_bytes
    }

    /// Retrieve the ssrc for this source
    pub fn ssrc(&self) -> u32 {
        self.source.ssrc
    }

    /// Set an sdes item for this source
    pub fn set_sdes_item(&mut self, type_: u8, value: &[u8]) {
        if let Ok(s) = std::str::from_utf8(value) {
            self.source.sdes.insert(type_, s.to_owned());
        }
    }

    /// Retrieve the sdes for this source
    pub fn sdes(&self) -> &HashMap<u8, String> {
        &self.source.sdes
    }

    /// Set the last time when activity was seen for this source
    pub fn set_last_activity(&mut self, time: Instant) {
        self.source.last_activity = time;
    }

    /// The last time when activity was seen for this source
    pub fn last_activity(&self) -> Instant {
        self.source.last_activity
    }

    pub(crate) fn take_sr_snapshot(
        &mut self,
        ntp_now: SystemTime,
        ntp_time: NtpTime,
        rtp_timestamp: u32,
    ) {
        self.last_sent_sr = Some(Sr {
            local_time: ntp_now,
            remote_time: ntp_time,
            rtp_time: rtp_timestamp,
            octet_count: (self.sent_bytes & 0xffff_ffff) as u32,
            packet_count: (self.sent_packets & 0xffff_ffff) as u32,
        });
    }

    pub fn last_sent_sr(&self) -> Option<Sr> {
        self.last_sent_sr
    }

    pub(crate) fn bye_sent_at(&mut self, time: Instant) {
        self.bye_sent_time = Some(time);
    }

    pub(crate) fn bye_sent_time(&self) -> Option<Instant> {
        self.bye_sent_time
    }

    pub(crate) fn mark_bye(&mut self, reason: &str) {
        if self.source.state == SourceState::Bye {
            return;
        }
        self.set_state(SourceState::Bye);
        self.bye_reason = Some(reason.to_string());
    }

    pub(crate) fn bye_reason(&self) -> Option<&String> {
        self.bye_reason.as_ref()
    }

    pub(crate) fn into_receive(self) -> LocalReceiveSource {
        LocalReceiveSource {
            source: self.source,
            bye_sent_time: self.bye_sent_time,
            bye_reason: self.bye_reason,
        }
    }

    pub fn add_last_rb(
        &mut self,
        sender_ssrc: u32,
        rb: ReportBlock<'_>,
        now: Instant,
        ntp_now: SystemTime,
    ) {
        let ntp_now = system_time_to_ntp_time_u64(ntp_now);
        let owned_rb = rb.into();
        self.last_received_rb
            .entry(sender_ssrc)
            .and_modify(|entry| {
                *entry = ReceivedRb {
                    rb: owned_rb,
                    receive_time: now,
                    receive_ntp_time: ntp_now,
                }
            })
            .or_insert_with(|| ReceivedRb {
                rb: owned_rb,
                receive_time: now,
                receive_ntp_time: ntp_now,
            });
    }

    pub fn received_report_blocks(&self) -> impl Iterator<Item = (u32, &ReceivedRb)> + '_ {
        self.last_received_rb.iter().map(|(&k, v)| (k, v))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRecvReply {
    /// hold this buffer for later and give it the relevant id.  The id will be used in a Drop, or
    /// Forward return value
    Hold(usize),
    /// drop a buffer by id. Should continue calling with the same input until not Drop or Forward
    Drop(usize),
    /// forward a held buffer by id. Should continue calling with the same input until not Drop or Forward.
    Forward(usize),
    /// forward the input buffer
    Passthrough,
    /// Ignore this buffer and do not passthrough
    Ignore,
}

#[derive(Debug)]
struct HeldRecvBuffer {
    id: usize,
    time: Instant,
    seqnum: u16,
    bytes: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Sr {
    local_time: SystemTime,
    remote_time: NtpTime,
    rtp_time: u32,
    octet_count: u32,
    packet_count: u32,
}

impl Sr {
    pub fn ntp_timestamp(&self) -> NtpTime {
        self.remote_time
    }

    pub fn rtp_timestamp(&self) -> u32 {
        self.rtp_time
    }

    pub fn octet_count(&self) -> u32 {
        self.octet_count
    }

    pub fn packet_count(&self) -> u32 {
        self.packet_count
    }
}

#[derive(Debug)]
pub struct RemoteSendSource {
    source: Source,
    probation_packets: usize,
    last_received_sr: Option<Sr>,
    rtp_from: Option<SocketAddr>,
    rtcp_from: Option<SocketAddr>,
    initial_seqnum: Option<u64>,
    ext_seqnum: ExtendedSeqnum,
    recv_bytes: u64,
    recv_packets: u64,
    recv_packets_at_last_rtcp: u64,
    ext_seqnum_at_last_rtcp: u64,
    jitter: u32,
    transit: Option<u32>,
    // any held buffers. Used when source is on probation.
    held_buffers: VecDeque<HeldRecvBuffer>,
    bitrate: Bitrate,
    last_sent_rb: Option<Rb>,
    last_received_rb: HashMap<u32, ReceivedRb>,
    last_request_key_unit: HashMap<u32, Instant>,

    // If a NACK/PLI is pending with the next RTCP packet
    send_pli: bool,
    // If a FIR is pending with the next RTCP packet
    send_fir: bool,
    // Sequence number of the next FIR
    send_fir_seqnum: u8,
    // Count from the ForceKeyUnitEvent to de-duplicate FIR
    send_fir_count: Option<u32>,
}

// The first time we recev a packet for jitter calculations
static INITIAL_RECV_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

impl RemoteSendSource {
    pub fn new(ssrc: u32) -> Self {
        Self {
            source: Source::new(ssrc),
            probation_packets: DEFAULT_PROBATION_N_PACKETS,
            last_received_sr: None,
            rtp_from: None,
            rtcp_from: None,
            initial_seqnum: None,
            ext_seqnum: ExtendedSeqnum::default(),
            recv_bytes: 0,
            recv_packets: 0,
            recv_packets_at_last_rtcp: 0,
            ext_seqnum_at_last_rtcp: 0,
            held_buffers: VecDeque::new(),
            jitter: 0,
            transit: None,
            bitrate: Bitrate::new(BITRATE_WINDOW),
            last_sent_rb: None,
            last_received_rb: HashMap::new(),
            last_request_key_unit: HashMap::new(),
            send_pli: false,
            send_fir: false,
            send_fir_seqnum: 0,
            send_fir_count: None,
        }
    }

    /// Retrieve the ssrc for this source
    pub fn ssrc(&self) -> u32 {
        self.source.ssrc
    }

    pub(crate) fn set_state(&mut self, state: SourceState) {
        self.source.set_state(state);
    }

    pub(crate) fn state(&self) -> SourceState {
        self.source.state
    }

    pub(crate) fn set_rtp_from(&mut self, from: Option<SocketAddr>) {
        self.rtp_from = from;
    }

    pub(crate) fn rtp_from(&self) -> Option<SocketAddr> {
        self.rtp_from
    }

    pub(crate) fn set_rtcp_from(&mut self, from: Option<SocketAddr>) {
        self.rtcp_from = from;
    }

    pub(crate) fn rtcp_from(&self) -> Option<SocketAddr> {
        self.rtcp_from
    }

    pub(crate) fn set_last_received_sr(
        &mut self,
        ntp_time: SystemTime,
        remote_time: NtpTime,
        rtp_time: u32,
        octet_count: u32,
        packet_count: u32,
    ) {
        self.last_received_sr = Some(Sr {
            local_time: ntp_time,
            remote_time,
            rtp_time,
            octet_count,
            packet_count,
        });
    }

    /// Retrieve the last received Sr for this source
    pub fn last_received_sr(&self) -> Option<Sr> {
        self.last_received_sr
    }

    /// Get the last sent RTCP report block for this source
    pub fn last_sent_rb(&self) -> Option<Rb> {
        self.last_sent_rb
    }

    fn init_sequence(&mut self, seqnum: u16) {
        self.last_received_sr = None;
        self.recv_bytes = 0;
        self.recv_packets = 0;
        self.recv_packets_at_last_rtcp = 0;
        self.initial_seqnum = self.ext_seqnum.current();
        self.ext_seqnum_at_last_rtcp = match self.ext_seqnum.current() {
            Some(ext) => ext,
            None => 0x10000 + seqnum as u64,
        };
        self.bitrate.reset();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn recv_packet(
        &mut self,
        bytes: u32,
        time: Instant,
        seqnum: u16,
        rtp_timestamp: u32,
        payload_type: u8,
        clock_rate: Option<u32>,
        hold_buffer_id: usize,
    ) -> SourceRecvReply {
        let initial_time = *INITIAL_RECV_TIME.get_or_init(|| time);

        if matches!(self.state(), SourceState::Bye) {
            return SourceRecvReply::Ignore;
        }

        let previous_seqnum = self.ext_seqnum.current();
        let ext_seqnum = self.ext_seqnum.next(seqnum);
        trace!(
            "source {} previous {previous_seqnum:?}, ext_seqnum {ext_seqnum}",
            self.ssrc()
        );

        let diff = match previous_seqnum {
            Some(ext) => (ext_seqnum as i64).wrapping_sub(ext as i64),
            None => 0,
        };

        trace!("source {} in state {:?} received seqnum {seqnum} with a difference of {diff} from the previous seqnum", self.ssrc(), self.state());

        let ret = if let SourceState::Probation(n_probation) = self.state() {
            // consecutive packets are good
            if diff == 1 {
                if (0..=1).contains(&n_probation) {
                    info!("source {} leaving probation", self.ssrc());
                    self.init_sequence(seqnum);
                    self.set_state(SourceState::Normal);
                    SourceRecvReply::Passthrough
                } else {
                    debug!(
                        "source {} holding seqnum {seqnum} on probation",
                        self.ssrc()
                    );
                    self.held_buffers.push_front(HeldRecvBuffer {
                        id: hold_buffer_id,
                        seqnum,
                        bytes,
                        time,
                    });
                    while self.held_buffers.len() > self.probation_packets {
                        if let Some(held) = self.held_buffers.pop_back() {
                            debug!(
                                "source {} dropping seqnum {seqnum} on probation",
                                self.ssrc()
                            );
                            return SourceRecvReply::Drop(held.id);
                        }
                    }
                    self.set_state(SourceState::Probation(n_probation - 1));
                    SourceRecvReply::Hold(hold_buffer_id)
                }
            } else if self.probation_packets > 0 {
                debug!(
                    "source {} resetting probation counter to {} at seqnum {seqnum}",
                    self.ssrc(),
                    self.probation_packets
                );
                self.set_state(SourceState::Probation(self.probation_packets - 1));
                if let Some(held) = self.held_buffers.pop_back() {
                    return SourceRecvReply::Drop(held.id);
                }
                self.held_buffers.push_front(HeldRecvBuffer {
                    id: hold_buffer_id,
                    seqnum,
                    bytes,
                    time,
                });
                while self.held_buffers.len() > self.probation_packets {
                    if let Some(held) = self.held_buffers.pop_back() {
                        return SourceRecvReply::Drop(held.id);
                    }
                }
                SourceRecvReply::Hold(hold_buffer_id)
            } else {
                info!(
                    "source {} leaving probation (no probation configured)",
                    self.ssrc()
                );
                self.init_sequence(seqnum);
                self.set_state(SourceState::Normal);
                SourceRecvReply::Passthrough
            }
        } else if diff >= 1 && diff < DEFAULT_MAX_DROPOUT as i64 {
            SourceRecvReply::Passthrough
        } else if diff < -(DEFAULT_MAX_MISORDER as i64) || diff >= DEFAULT_MAX_DROPOUT as i64 {
            debug!("non-consecutive packet outside of configured limits, dropping");

            // TODO: we will want to perform a few tasks here that the C jitterbuffer
            // used to be taking care of:
            //
            // - We probably want to operate in the time domain rather than the sequence domain,
            //   that means implementing a utility similar to RTPPacketRateCtx
            // - We should update our late / lost stats when a packet does get ignored
            // - We should perform the equivalent of the big gap handling in the C jitterbuffer,
            //   possibly holding gap packets for a while before deciding that we indeed have an
            //   actual gap, then propagating a new "resync" receive reply before releasing the
            //   gap packets in order to let other components (eg jitterbuffer) reset themselves
            //   when needed.

            // FIXME: should be a harder error?
            return SourceRecvReply::Ignore;
        } else {
            // duplicate or reordered packet
            // downstream jitterbuffer will deal with this
            SourceRecvReply::Passthrough
        };

        if matches!(ret, SourceRecvReply::Passthrough) {
            if let Some(held) = self.held_buffers.pop_back() {
                info!(
                    "source {} pushing stored seqnum {}",
                    self.ssrc(),
                    held.seqnum
                );
                self.recv_packet_add_to_stats(
                    rtp_timestamp,
                    held.time,
                    initial_time,
                    payload_type,
                    clock_rate,
                    ext_seqnum,
                    held.bytes,
                );
                return SourceRecvReply::Forward(held.id);
            }
        }

        trace!("setting ext seqnum to {ext_seqnum}");
        self.recv_packet_add_to_stats(
            rtp_timestamp,
            time,
            initial_time,
            payload_type,
            clock_rate,
            ext_seqnum,
            bytes,
        );

        ret
    }

    #[allow(clippy::too_many_arguments)]
    fn recv_packet_add_to_stats(
        &mut self,
        rtp_timestamp: u32,
        now: Instant,
        initial_time: Instant,
        payload_type: u8,
        clock_rate: Option<u32>,
        ext_seqnum: u64,
        bytes: u32,
    ) {
        /* calculate jitter */
        if let Some(clock_rate) = clock_rate {
            let rtparrival =
                ((now.duration_since(initial_time).as_micros() & 0xffff_ffff_ffff_ffff) as u32)
                    .mul_div_round(clock_rate, 1_000_000)
                    .unwrap();
            let transit = rtparrival.wrapping_sub(rtp_timestamp);
            let diff = if let Some(existing_transit) = self.transit {
                existing_transit.abs_diff(transit)
            } else {
                0
            };
            self.transit = Some(transit);
            trace!("jitter {} diff {diff}", self.jitter);
            self.jitter = self
                .jitter
                .saturating_add(diff.saturating_sub((self.jitter.saturating_add(8)) >> 4));
        }
        self.source.payload_type = Some(payload_type);

        if self.initial_seqnum.is_none() {
            self.initial_seqnum = Some(ext_seqnum);
        }

        self.bitrate.add_entry(bytes as usize, now);
        self.recv_bytes = self.recv_bytes.wrapping_add(bytes as u64);
        self.recv_packets += 1;
    }

    pub(crate) fn received_sdes(&mut self, type_: u8, value: &[u8]) {
        if let Ok(s) = std::str::from_utf8(value) {
            self.source.sdes.insert(type_, s.to_owned());
        }
    }

    /// Retrieve the SDES items currently received for this remote sender
    pub fn sdes(&self) -> &HashMap<u8, String> {
        &self.source.sdes
    }

    pub(crate) fn set_last_activity(&mut self, time: Instant) {
        self.source.last_activity = time;
    }

    /// Retrieve the last time that activity was seen on this source
    pub fn last_activity(&self) -> Instant {
        self.source.last_activity
    }

    pub(crate) fn bitrate(&self) -> usize {
        self.bitrate.bitrate()
    }

    pub fn payload_type(&self) -> Option<u8> {
        self.source.payload_type
    }

    fn extended_sequence_number(&self) -> u32 {
        (self.ext_seqnum.current().unwrap_or(0) & 0xffff_ffff) as u32
    }

    pub(crate) fn generate_report_block(&self, ntp_time: SystemTime) -> Rb {
        let (last_sr, delay_since_last_sr) = self
            .last_received_sr
            .as_ref()
            .map(|t| {
                (
                    t.remote_time,
                    NtpTime::from_duration(
                        ntp_time
                            .duration_since(t.local_time)
                            .unwrap_or(Duration::from_secs(0)),
                    ),
                )
            })
            .unwrap_or((
                NtpTime::from_duration(Duration::from_secs(0)),
                NtpTime::from_duration(Duration::from_secs(0)),
            ));

        let lost = self.packets_lost();

        let expected_since_last_rtcp = self
            .ext_seqnum
            .current()
            .unwrap_or(0)
            .saturating_sub(self.ext_seqnum_at_last_rtcp);
        let recv_packets_since_last_rtcp = self.recv_packets - self.recv_packets_at_last_rtcp;
        let lost_packets_since_last_rtcp =
            expected_since_last_rtcp as i64 - recv_packets_since_last_rtcp as i64;
        let fraction_lost = if expected_since_last_rtcp == 0 || lost_packets_since_last_rtcp <= 0 {
            0
        } else {
            (((lost_packets_since_last_rtcp as u64) << 8) / expected_since_last_rtcp) as u8
        };
        let cumulative_lost = if lost < 0 {
            0x800000 | (lost & 0x7fffff) as u32
        } else {
            (lost & 0x7ffffff) as u32
        };

        trace!(
            "ssrc {} current packet counts ext_seqnum {:?} recv_packets {}",
            self.source.ssrc,
            self.ext_seqnum,
            self.recv_packets
        );
        trace!(
            "ssrc {} previous rtcp values ext_seqnum {:?} recv_packets {}",
            self.source.ssrc,
            self.ext_seqnum_at_last_rtcp,
            self.recv_packets_at_last_rtcp
        );
        trace!("ssrc {} fraction expected {expected_since_last_rtcp} lost {lost_packets_since_last_rtcp} fraction lost {fraction_lost}", self.source.ssrc);

        Rb {
            ssrc: self.source.ssrc,
            fraction_lost,
            cumulative_lost,
            extended_sequence_number: self.extended_sequence_number(),
            jitter: self.jitter >> 4,
            last_sr: last_sr.as_u32(),
            delay_since_last_sr: delay_since_last_sr.as_u32(),
        }
    }

    pub(crate) fn update_last_rtcp(&mut self) {
        self.recv_packets_at_last_rtcp = self.recv_packets;
        if let Some(ext) = self.ext_seqnum.current() {
            self.ext_seqnum_at_last_rtcp = ext;
        }
    }

    /// The amount of jitter (in clock-rate units)
    pub fn jitter(&self) -> u32 {
        self.jitter >> 4
    }

    /// The total number of packets lost over the lifetime of this source
    pub fn packets_lost(&self) -> i64 {
        let expected =
            self.ext_seqnum.current().unwrap_or(0) - self.initial_seqnum.unwrap_or(0) + 1;
        expected as i64 - self.recv_packets as i64
    }

    #[cfg(test)]
    /// Set the number of probation packets before validating this source
    pub fn set_probation_packets(&mut self, n_packets: usize) {
        info!("source {} setting probation to {n_packets}", self.ssrc());
        self.probation_packets = n_packets;
        match self.state() {
            SourceState::Bye | SourceState::Normal => (),
            SourceState::Probation(existing) => {
                if n_packets < existing {
                    self.set_state(SourceState::Probation(n_packets));
                }
            }
        }
    }

    pub fn packet_count(&self) -> u64 {
        self.recv_packets
    }

    pub fn octet_count(&self) -> u64 {
        self.recv_bytes
    }

    pub fn add_last_rb(
        &mut self,
        sender_ssrc: u32,
        rb: ReportBlock<'_>,
        now: Instant,
        ntp_now: SystemTime,
    ) {
        let ntp_now = system_time_to_ntp_time_u64(ntp_now);
        let owned_rb = rb.into();
        self.last_received_rb
            .entry(sender_ssrc)
            .and_modify(|entry| {
                *entry = ReceivedRb {
                    rb: owned_rb,
                    receive_time: now,
                    receive_ntp_time: ntp_now,
                }
            })
            .or_insert_with(|| ReceivedRb {
                rb: owned_rb,
                receive_time: now,
                receive_ntp_time: ntp_now,
            });
    }

    pub fn received_report_blocks(&self) -> impl Iterator<Item = (u32, &ReceivedRb)> + '_ {
        self.last_received_rb.iter().map(|(&k, v)| (k, v))
    }

    pub(crate) fn into_receive(self) -> RemoteReceiveSource {
        RemoteReceiveSource {
            source: self.source,
            rtcp_from: self.rtcp_from,
            last_request_key_unit: self.last_request_key_unit,
        }
    }

    pub(crate) fn remote_request_key_unit_allowed(
        &mut self,
        now: Instant,
        rb: &ReceivedRb,
    ) -> bool {
        let rtt = rb.round_trip_time();

        // Allow up to one key-unit request per RTT and SSRC.
        let mut allowed = false;
        self.last_request_key_unit
            .entry(rb.rb.ssrc)
            .and_modify(|previous| {
                allowed = now.duration_since(*previous) >= rtt;
                *previous = now;
            })
            .or_insert_with(|| now);

        allowed
    }

    pub(crate) fn request_remote_key_unit(&mut self, _now: Instant, typ: KeyUnitRequestType) {
        match typ {
            KeyUnitRequestType::Fir(count) => {
                if self.send_fir_count != Some(count) {
                    self.send_fir_seqnum = self.send_fir_seqnum.wrapping_add(1);
                }
                self.send_fir = true;
                self.send_fir_count = Some(count);
            }
            KeyUnitRequestType::Pli if !self.send_fir => {
                self.send_pli = true;
            }
            _ => {}
        }
    }

    pub(crate) fn generate_pli(&mut self) -> Option<rtcp_types::PliBuilder> {
        if self.send_pli {
            self.send_pli = false;
            Some(rtcp_types::Pli::builder())
        } else {
            None
        }
    }

    pub(crate) fn generate_fir(
        &mut self,
        fir: rtcp_types::FirBuilder,
        added: &mut bool,
    ) -> rtcp_types::FirBuilder {
        if self.send_fir {
            self.send_fir = false;
            *added = true;
            fir.add_ssrc(self.ssrc(), self.send_fir_seqnum)
        } else {
            fir
        }
    }
}

#[derive(Debug)]
pub struct LocalReceiveSource {
    source: Source,
    bye_sent_time: Option<Instant>,
    bye_reason: Option<String>,
}

impl LocalReceiveSource {
    pub(crate) fn new(ssrc: u32) -> Self {
        Self {
            source: Source::new(ssrc),
            bye_sent_time: None,
            bye_reason: None,
        }
    }

    pub fn ssrc(&self) -> u32 {
        self.source.ssrc
    }

    pub(crate) fn set_state(&mut self, state: SourceState) {
        self.source.set_state(state);
    }

    pub(crate) fn state(&self) -> SourceState {
        self.source.state
    }

    pub(crate) fn payload_type(&self) -> Option<u8> {
        self.source.payload_type
    }

    /// Set an sdes item for this source
    pub fn set_sdes_item(&mut self, type_: u8, value: &[u8]) {
        if let Ok(s) = std::str::from_utf8(value) {
            self.source.sdes.insert(type_, s.to_owned());
        }
    }

    /// Retrieve the sdes for this source
    pub fn sdes(&self) -> &HashMap<u8, String> {
        &self.source.sdes
    }

    /// Set the last time when activity was seen for this source
    pub fn set_last_activity(&mut self, time: Instant) {
        self.source.last_activity = time;
    }

    /// Retrieve the last time that activity was seen on this source
    pub fn last_activity(&self) -> Instant {
        self.source.last_activity
    }

    pub(crate) fn bye_sent_at(&mut self, time: Instant) {
        self.bye_sent_time = Some(time);
    }

    pub(crate) fn bye_sent_time(&self) -> Option<Instant> {
        self.bye_sent_time
    }

    pub(crate) fn mark_bye(&mut self, reason: &str) {
        if self.source.state == SourceState::Bye {
            return;
        }
        self.set_state(SourceState::Bye);
        self.bye_reason = Some(reason.to_string());
    }

    pub(crate) fn bye_reason(&self) -> Option<&String> {
        self.bye_reason.as_ref()
    }
}

#[derive(Debug)]
pub struct RemoteReceiveSource {
    source: Source,
    rtcp_from: Option<SocketAddr>,
    last_request_key_unit: HashMap<u32, Instant>,
}

impl RemoteReceiveSource {
    pub(crate) fn new(ssrc: u32) -> Self {
        Self {
            source: Source::new(ssrc),
            rtcp_from: None,
            last_request_key_unit: HashMap::new(),
        }
    }

    pub fn ssrc(&self) -> u32 {
        self.source.ssrc
    }

    pub(crate) fn set_state(&mut self, state: SourceState) {
        self.source.set_state(state);
    }

    pub(crate) fn state(&self) -> SourceState {
        self.source.state
    }

    pub(crate) fn set_rtcp_from(&mut self, from: Option<SocketAddr>) {
        self.rtcp_from = from;
    }

    pub(crate) fn rtcp_from(&self) -> Option<SocketAddr> {
        self.rtcp_from
    }

    pub(crate) fn set_last_activity(&mut self, time: Instant) {
        self.source.last_activity = time;
    }

    pub(crate) fn received_sdes(&mut self, type_: u8, value: &[u8]) {
        if let Ok(s) = std::str::from_utf8(value) {
            self.source.sdes.insert(type_, s.to_owned());
        }
    }

    /// Retrieve the SDES items currently received for this remote receiver
    pub fn sdes(&self) -> &HashMap<u8, String> {
        &self.source.sdes
    }

    /// Retrieve the last time that activity was seen on this source
    pub fn last_activity(&self) -> Instant {
        self.source.last_activity
    }

    pub(crate) fn into_send(self) -> RemoteSendSource {
        RemoteSendSource {
            source: self.source,
            probation_packets: DEFAULT_PROBATION_N_PACKETS,
            last_received_sr: None,
            rtp_from: None,
            rtcp_from: self.rtcp_from,
            initial_seqnum: None,
            ext_seqnum: ExtendedSeqnum::default(),
            recv_bytes: 0,
            recv_packets: 0,
            recv_packets_at_last_rtcp: 0,
            ext_seqnum_at_last_rtcp: 0,
            held_buffers: VecDeque::new(),
            jitter: 0,
            transit: None,
            bitrate: Bitrate::new(BITRATE_WINDOW),
            last_sent_rb: None,
            last_received_rb: HashMap::new(),
            last_request_key_unit: self.last_request_key_unit,
            send_pli: false,
            send_fir: false,
            send_fir_seqnum: 0,
            send_fir_count: None,
        }
    }

    pub(crate) fn remote_request_key_unit_allowed(
        &mut self,
        now: Instant,
        rb: &ReceivedRb,
    ) -> bool {
        let rtt = rb.round_trip_time();

        // Allow up to one key-unit request per RTT.
        let mut allowed = false;
        self.last_request_key_unit
            .entry(rb.rb.ssrc)
            .and_modify(|previous| {
                allowed = now.duration_since(*previous) >= rtt;
                *previous = now;
            })
            .or_insert_with(|| now);

        allowed
    }
}

#[derive(Debug)]
struct Bitrate {
    max_time: Duration,
    entries: VecDeque<(usize, Instant)>,
}

impl Bitrate {
    fn new(max_time: Duration) -> Self {
        Self {
            max_time,
            entries: VecDeque::new(),
        }
    }

    fn add_entry(&mut self, bytes: usize, time: Instant) {
        self.entries.push_back((bytes, time));
        while let Some((bytes, latest_time)) = self.entries.pop_front() {
            if time.duration_since(latest_time) < self.max_time {
                self.entries.push_front((bytes, latest_time));
                break;
            }
        }
    }

    fn bitrate(&self) -> usize {
        if let Some(front) = self.entries.front() {
            let back = self.entries.back().unwrap();
            let dur_micros = (back.1 - front.1).as_micros();
            if dur_micros == 0 {
                return front.0;
            }
            let bytes = self.entries.iter().map(|entry| entry.0).sum::<usize>();

            (bytes as u64)
                .mul_div_round(1_000_000, dur_micros as u64)
                .unwrap_or(front.0 as u64) as usize
        } else {
            0
        }
    }

    fn reset(&mut self) {
        self.entries.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtpbin2::session::tests::init_logs;

    const TEST_PT: u8 = 96;

    #[test]
    fn bitrate_single_value() {
        init_logs();
        // the bitrate of a single entry is the entry itself
        let mut bitrate = Bitrate::new(BITRATE_WINDOW);
        bitrate.add_entry(100, Instant::now());
        assert_eq!(bitrate.bitrate(), 100);
    }

    #[test]
    fn bitrate_two_values_over_half_second() {
        init_logs();
        let mut bitrate = Bitrate::new(Duration::from_secs(1));
        let now = Instant::now();
        bitrate.add_entry(100, now);
        bitrate.add_entry(300, now + Duration::from_millis(500));
        assert_eq!(bitrate.bitrate(), (100 + 300) * 2);
    }

    #[test]
    fn receive_probation() {
        init_logs();
        let mut source = RemoteSendSource::new(100);
        let now = Instant::now();
        let mut hold_buffer_id = 0;
        assert_eq!(
            source.state(),
            SourceState::Probation(DEFAULT_PROBATION_N_PACKETS)
        );
        assert_eq!(
            SourceRecvReply::Hold(0),
            source.recv_packet(16, now, 500, 100, TEST_PT, None, hold_buffer_id)
        );
        hold_buffer_id += 1;
        assert_eq!(
            SourceRecvReply::Forward(0),
            source.recv_packet(16, now, 501, 100, TEST_PT, None, hold_buffer_id)
        );
        assert_eq!(source.state(), SourceState::Normal);
        assert_eq!(
            SourceRecvReply::Passthrough,
            source.recv_packet(16, now, 501, 100, TEST_PT, None, hold_buffer_id)
        );
        assert_eq!(source.state(), SourceState::Normal);
    }

    #[test]
    fn receive_probation_gap() {
        init_logs();
        let mut source = RemoteSendSource::new(100);
        let now = Instant::now();
        let mut hold_buffer_id = 0;
        assert_eq!(
            source.state(),
            SourceState::Probation(DEFAULT_PROBATION_N_PACKETS)
        );
        assert_eq!(
            SourceRecvReply::Hold(0),
            source.recv_packet(100, now, 500, 100, TEST_PT, None, hold_buffer_id)
        );
        hold_buffer_id += 1;
        // push a buffer with a sequence gap and reset the probation counter
        assert_eq!(
            SourceRecvReply::Drop(0),
            source.recv_packet(101, now, 502, 100, TEST_PT, None, hold_buffer_id)
        );
        assert_eq!(
            SourceRecvReply::Hold(1),
            source.recv_packet(100, now, 502, 100, TEST_PT, None, hold_buffer_id)
        );
        hold_buffer_id += 1;
        assert_eq!(
            SourceRecvReply::Forward(1),
            source.recv_packet(101, now, 503, 100, TEST_PT, None, hold_buffer_id)
        );
        assert_eq!(source.state(), SourceState::Normal);
        assert_eq!(
            SourceRecvReply::Passthrough,
            source.recv_packet(101, now, 503, 100, TEST_PT, None, hold_buffer_id)
        );
    }

    #[test]
    fn receive_no_probation() {
        init_logs();
        let mut source = RemoteSendSource::new(100);
        let now = Instant::now();
        assert_eq!(
            source.state(),
            SourceState::Probation(DEFAULT_PROBATION_N_PACKETS)
        );
        source.set_probation_packets(0);
        assert_eq!(source.state(), SourceState::Probation(0));
        assert_eq!(
            SourceRecvReply::Passthrough,
            source.recv_packet(100, now, 500, 100, TEST_PT, None, 0)
        );
        assert_eq!(source.state(), SourceState::Normal);
    }

    #[test]
    fn receive_wraparound() {
        init_logs();
        let mut source = RemoteSendSource::new(100);
        source.set_probation_packets(0);
        let now = Instant::now();
        assert_eq!(
            SourceRecvReply::Passthrough,
            source.recv_packet(16, now, u16::MAX, u32::MAX, TEST_PT, None, 0)
        );
        assert_eq!(
            SourceRecvReply::Passthrough,
            source.recv_packet(16, now, 0, 0, TEST_PT, None, 0)
        );
    }
}
