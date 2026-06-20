// SPDX-License-Identifier: MPL-2.0

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};

use rtcp_types::*;
use rtp_types::RtpPacket;

use rand::prelude::*;

use crate::rtpbin2::source::SourceRecvReply;

use super::source::{
    LocalReceiveSource, LocalSendSource, RemoteReceiveSource, RemoteSendSource, SourceState,
};
use super::time::system_time_to_ntp_time_u64;

use gst::prelude::MulDiv;

// TODO: make configurable
pub const RTCP_MIN_REPORT_INTERVAL: Duration = Duration::from_secs(5);

const RTCP_SOURCE_TIMEOUT_N_INTERVALS: u32 = 5;
const RTCP_ADDRESS_CONFLICT_TIMEOUT: Duration = RTCP_MIN_REPORT_INTERVAL.saturating_mul(12);
// 5% of 8kB/s
const RTCP_MIN_BANDWIDTH: usize = 400;
const RTCP_MTU: usize = 1200;

const UDP_IP_OVERHEAD_BYTES: usize = 28;

#[derive(Debug, Default)]
struct RtcpTimeMembers {
    time: Option<Instant>,
    p_members: usize,
}

#[derive(Debug)]
struct ByeState {
    members: usize,
    pmembers: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RtpProfile {
    #[default]
    Avp,
    Avpf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyUnitRequestType {
    Pli,
    Fir(u32),
}

impl RtpProfile {
    fn is_feedback(&self) -> bool {
        self == &Self::Avpf
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestEarlyRtcpResult {
    NotScheduled,
    Scheduled,
    TimerReconsideration,
}

#[derive(Debug)]
pub struct Session {
    // settings
    min_rtcp_interval: Duration,
    profile: RtpProfile,
    reduced_size_rtcp: bool,
    // state
    local_senders: HashMap<u32, LocalSendSource>,
    local_receivers: HashMap<u32, LocalReceiveSource>,
    remote_receivers: HashMap<u32, RemoteReceiveSource>,
    remote_senders: HashMap<u32, RemoteSendSource>,
    average_rtcp_size: usize,
    last_sent_data: Option<Instant>,
    hold_buffer_counter: usize,
    sdes: HashMap<u8, String>,
    pt_map: HashMap<u8, u32>,
    conflicting_addresses: HashMap<SocketAddr, Instant>,
    // used when we have not sent anything but need a ssrc for Rr
    internal_rtcp_sender_src: Option<u32>,
    bye_state: Option<ByeState>,
    is_point_to_point: bool,

    // rtcp scheduling state
    // T_rr: holds the interval used to calculate the current `next_rtcp_send`
    rtcp_interval: Option<Duration>,
    // tp: last time that any rtcp was handled
    last_rtcp_handle_time: Option<Instant>,
    // tn: holds the next regular rtcp send time and the number of members at the
    // time when the time was calculated
    next_rtcp_send: RtcpTimeMembers,
    // T_rr_last: last two times a regular rtcp packet was sent
    last_rtcp_sent_times: VecDeque<Instant>,
    // time for the next early rtcp to be sent
    next_early_rtcp_time: Option<Instant>,
    pending_rtcp_send: VecDeque<RtcpSendReply>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvReply {
    /// A new ssrc was discovered.  If you want to change things about the new ssrc, then do it now.
    /// Call recv() again.
    NewSsrc(u32, u8),
    /// hold this buffer for later and give it the relevant id.  The id will be used in a Drop, or
    /// Forward return value
    Hold(usize),
    /// Drop a buffer by id. Should continue calling with the same input until not Drop or Forward
    Drop(usize),
    /// Forward a held buffer by id. Should continue calling with the same input until not Drop or Forward.
    Forward(usize),
    /// Forward the input buffer
    Passthrough,
    /// Ignore this buffer and do not passthrough
    Ignore,
    /// A ssrc collision has been detected for the provided ssrc. Sender (us) should change ssrc.
    SsrcCollision(u32),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SendReply {
    /// A new ssrc was discovered.  If you want to change things about the new ssrc, then do it now.
    /// Call send() again.
    NewSsrc(u32, u8),
    /// Forward the input buffer
    Passthrough,
    /// Drop this buffer
    Drop,
    /// SSRC collision detected, Sender (us) should change our SSRC and this packet must be dropped
    SsrcCollision(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpRecvReply {
    /// A new ssrc was discovered.  If you want to change things about the new ssrc, then do it now
    /// before pushing the buffer again
    NewSsrc(u32),
    /// SSRC collision detected, Sender (us) should change our SSRC and this packet must be dropped
    SsrcCollision(u32),
    /// RTCP timer needs to be reconsidered.  Call poll_rtcp_send_timeout() to get the new time
    TimerReconsideration,
    /// Request a key unit for the given SSRC of ours
    RequestKeyUnit { ssrcs: Vec<u32>, fir: bool },
    /// A new cname to ssrc mapping was found in a sdes: (cname, ssrc)
    NewCName((String, u32)),
    /// A new RTP to NTP mapping was received for an ssrc: (ssrc, RTP, NTP)
    NewRtpNtp((u32, u32, u64)),
    /// A ssrc has byed
    SsrcBye(u32),
}

#[derive(Debug)]
pub enum RtcpSendReply {
    /// Data needs to be sent
    Data(Vec<u8>),
    /// A ssrc has byed
    SsrcBye(u32),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RequestRemoteKeyUnitReply {
    /// RTCP timer needs to be reconsidered.  Call poll_rtcp_send_timeout() to get the new time
    TimerReconsideration,
}

impl Session {
    pub fn new() -> Self {
        let cname = generate_cname();
        let mut sdes = HashMap::new();
        sdes.insert(SdesItem::CNAME, cname);
        Self {
            min_rtcp_interval: RTCP_MIN_REPORT_INTERVAL,
            profile: RtpProfile::default(),
            reduced_size_rtcp: false,
            local_senders: HashMap::new(),
            // also known as remote_senders
            local_receivers: HashMap::new(),
            remote_receivers: HashMap::new(),
            remote_senders: HashMap::new(),
            last_rtcp_sent_times: VecDeque::new(),
            rtcp_interval: None,
            next_rtcp_send: RtcpTimeMembers {
                time: None,
                p_members: 0,
            },
            average_rtcp_size: 100,
            last_sent_data: None,
            hold_buffer_counter: 0,
            sdes,
            pt_map: HashMap::new(),
            conflicting_addresses: HashMap::new(),
            internal_rtcp_sender_src: None,
            bye_state: None,
            next_early_rtcp_time: None,
            last_rtcp_handle_time: None,
            is_point_to_point: true,
            pending_rtcp_send: VecDeque::new(),
        }
    }

    /// Set the minimum (regular) RTCP interval to use for this session
    pub fn set_min_rtcp_interval(&mut self, min_rtcp_interval: Duration) {
        self.min_rtcp_interval = min_rtcp_interval;
    }

    /// Set the RTP profile to use.
    pub fn set_profile(&mut self, profile: RtpProfile) {
        self.profile = profile;
    }

    /// Set usage of reduced size RTCP
    pub fn set_reduced_size_rtcp(&mut self, reduced_size_rtcp: bool) {
        self.reduced_size_rtcp = reduced_size_rtcp;
    }

    fn n_members(&self) -> usize {
        self.bye_state
            .as_ref()
            .map(|state| state.members)
            .unwrap_or_else(|| {
                self.local_senders
                    .values()
                    .filter(|source| source.state() == SourceState::Normal)
                    .count()
                    + self
                        .local_receivers
                        .values()
                        .filter(|source| source.state() == SourceState::Normal)
                        .count()
                    + self
                        .remote_senders
                        .values()
                        .filter(|source| source.state() == SourceState::Normal)
                        .count()
                    + self
                        .remote_receivers
                        .values()
                        .filter(|source| source.state() == SourceState::Normal)
                        .count()
            })
    }

    fn n_senders(&self) -> usize {
        self.bye_state.as_ref().map(|_state| 0).unwrap_or_else(|| {
            self.local_senders
                .values()
                .filter(|source| source.state() == SourceState::Normal)
                .count()
                + self
                    .remote_senders
                    .values()
                    .filter(|source| source.state() == SourceState::Normal)
                    .count()
        })
    }

    fn p_members(&self) -> usize {
        self.bye_state
            .as_ref()
            .map(|state| state.pmembers)
            .unwrap_or(self.next_rtcp_send.p_members)
    }

    fn update_point_to_point(&mut self) {
        // This definition of point to point is from RFC 8108
        // i.e. If there are multiple remote CNAMEs, then we are not point to point.
        //
        // XXX: We currently don't provide external overriding for this
        let mut cname = None;
        self.is_point_to_point = self
            .remote_senders
            .values()
            .filter_map(|source| source.sdes().get(&SdesItem::CNAME))
            .chain(
                self.remote_receivers
                    .values()
                    .filter_map(|source| source.sdes().get(&SdesItem::CNAME)),
            )
            .all(|cn| {
                let ret = cname.is_none() || cname == Some(cn);
                cname = cname.or(Some(cn));
                ret
            });
    }

    /// Set the RTP clock rate for a particular payload type
    pub fn set_pt_clock_rate(&mut self, pt: u8, clock_rate: u32) {
        self.pt_map.insert(pt, clock_rate);
    }

    /// Retrieve the RTP clock rate for a particular payload type
    pub fn clock_rate_from_pt(&self, pt: u8) -> Option<u32> {
        self.pt_map.get(&pt).copied()
    }

    fn handle_ssrc_conflict(&mut self, addr: SocketAddr, now: Instant) -> bool {
        if let Some(time) = self.conflicting_addresses.get_mut(&addr) {
            trace!("ignoring looped packet from known collision address {addr:?}");
            *time = now;
            false
        } else {
            trace!("New collision address {addr:?}");
            self.conflicting_addresses.insert(addr, now);
            true
        }
    }

    /// Handle receiving an RTP packet.  The [`RecvRecply`] return value outlines what the caller
    /// must do with the packet.
    pub fn handle_recv(
        &mut self,
        rtp: &RtpPacket,
        from: Option<SocketAddr>,
        now: Instant,
    ) -> RecvReply {
        trace!(
            "receive rtp from:{from:?} at {now:?}, ssrc:{}, pt:{}, seqno:{}, rtp ts:{}, bytes:{}",
            rtp.ssrc(),
            rtp.payload_type(),
            rtp.sequence_number(),
            rtp.timestamp(),
            rtp.payload().len()
        );
        if let Some(addr) = from {
            // handle possible collisions
            if let Some(_source) = self.local_senders.get(&rtp.ssrc()) {
                if self.handle_ssrc_conflict(addr, now) {
                    return RecvReply::SsrcCollision(rtp.ssrc());
                } else {
                    return RecvReply::Ignore;
                }
            } else if let Some(recv) = self.remote_receivers.remove(&rtp.ssrc()) {
                let mut sender = recv.into_send();
                sender.set_rtp_from(from);
                self.remote_senders.insert(rtp.ssrc(), sender);
            } else if let Some(recv) = self.remote_senders.get_mut(&rtp.ssrc()) {
                if let Some(from_addr) = recv.rtp_from() {
                    if addr != from_addr {
                        // this is favour old source behaviour
                        return RecvReply::Ignore;
                    }
                } else {
                    recv.set_rtp_from(from);
                }
            }
        }

        // TODO: handle CSRCs

        let clock_rate = self.clock_rate_from_pt(rtp.payload_type());

        if let Some(source) = self.remote_senders.get_mut(&rtp.ssrc()) {
            match source.recv_packet(
                rtp.payload().len() as u32,
                now,
                rtp.sequence_number(),
                rtp.timestamp(),
                rtp.payload_type(),
                clock_rate,
                self.hold_buffer_counter,
            ) {
                SourceRecvReply::Hold(id) => {
                    self.hold_buffer_counter += 1;
                    RecvReply::Hold(id)
                }
                SourceRecvReply::Drop(id) => RecvReply::Drop(id),
                SourceRecvReply::Ignore => RecvReply::Ignore,
                SourceRecvReply::Forward(id) => RecvReply::Forward(id),
                SourceRecvReply::Passthrough => RecvReply::Passthrough,
            }
        } else {
            let mut source = RemoteSendSource::new(rtp.ssrc());
            source.set_rtp_from(from);
            self.remote_senders.insert(rtp.ssrc(), source);
            trace!("new receive ssrc:{}, pt:{}", rtp.ssrc(), rtp.payload_type());
            RecvReply::NewSsrc(rtp.ssrc(), rtp.payload_type())
        }
    }

    /// Handle sending a RTP packet.  The [`SendReply`] return value indicates what the caller
    /// must do with this packet.
    pub fn handle_send(&mut self, rtp: &RtpPacket, now: Instant) -> SendReply {
        trace!(
            "sending at {now:?} ssrc:{}, pt:{}, seqno:{}, rtp ts:{}, bytes:{}",
            rtp.ssrc(),
            rtp.payload_type(),
            rtp.sequence_number(),
            rtp.timestamp(),
            rtp.payload().len()
        );
        self.last_sent_data = Some(now);

        // handle possible collision
        if let Some(source) = self.remote_senders.get(&rtp.ssrc()) {
            if let Some(rtp_from) = source.rtp_from().or(source.rtcp_from()) {
                if self.handle_ssrc_conflict(rtp_from, now) {
                    return SendReply::SsrcCollision(rtp.ssrc());
                }
            }
            return SendReply::Drop;
        } else if let Some(source) = self.remote_receivers.get(&rtp.ssrc()) {
            if let Some(rtcp_from) = source.rtcp_from() {
                if self.handle_ssrc_conflict(rtcp_from, now) {
                    return SendReply::SsrcCollision(rtp.ssrc());
                }
            }
            return SendReply::Drop;
        }

        if let Some(source) = self.local_senders.get_mut(&rtp.ssrc()) {
            if source.state() != SourceState::Normal {
                warn!(
                    "source {} is in state {:?}, dropping send",
                    source.ssrc(),
                    source.state()
                );
                return SendReply::Drop;
            }
            source.set_last_activity(now);
            if let Some(_clock_rate) = self.pt_map.get(&rtp.payload_type()) {
                source.sent_packet(
                    rtp.payload().len(),
                    now,
                    rtp.sequence_number(),
                    rtp.timestamp(),
                    rtp.payload_type(),
                );
                SendReply::Passthrough
            } else {
                trace!("no clock rate for pt:{}, dropping", rtp.payload_type());
                SendReply::Drop
            }
        } else {
            self.local_receivers.remove_entry(&rtp.ssrc());
            let mut source = LocalSendSource::new(rtp.ssrc());
            source.set_last_activity(now);
            source.set_state(SourceState::Normal);
            for (k, v) in self.sdes.iter() {
                source.set_sdes_item(*k, v.as_bytes());
            }
            if self.local_senders.is_empty() && self.rtcp_reverse_consideration(0, now) {
                // TODO: signal updated timeout
            }
            self.local_senders.insert(rtp.ssrc(), source);
            info!("new send ssrc:{}, pt:{}", rtp.ssrc(), rtp.payload_type());
            SendReply::NewSsrc(rtp.ssrc(), rtp.payload_type())
        }
    }

    fn update_rtcp_average(&mut self, additional_size: usize) {
        if self.average_rtcp_size == 0 {
            self.average_rtcp_size = additional_size;
        } else {
            self.average_rtcp_size = (additional_size + self.average_rtcp_size * 15) / 16;
        }
    }

    fn handle_rb(
        &mut self,
        sender_ssrc: u32,
        rb: ReportBlock<'_>,
        from: Option<SocketAddr>,
        now: Instant,
        ntp_time: SystemTime,
    ) -> Option<RtcpRecvReply> {
        let mut ret = None;

        if let Some(source) = self.local_senders.get_mut(&rb.ssrc()) {
            source.add_last_rb(sender_ssrc, rb, now, ntp_time);
            source.set_last_activity(now);
        } else {
            if let Some(source) = self.remote_receivers.remove(&rb.ssrc()) {
                let sender = source.into_send();
                self.remote_senders.insert(rb.ssrc(), sender);
            }

            let source = self.remote_senders.entry(rb.ssrc()).or_insert_with(|| {
                ret = Some(RtcpRecvReply::NewSsrc(rb.ssrc()));
                RemoteSendSource::new(rb.ssrc())
            });
            source.set_rtcp_from(from);
            source.set_state(SourceState::Normal);
            source.set_last_activity(now);
            source.add_last_rb(sender_ssrc, rb, now, ntp_time);
        }

        ret
    }

    fn rtcp_reverse_consideration(&mut self, initial_n_members: usize, now: Instant) -> bool {
        let n_members = self.n_members();
        if n_members >= self.p_members() {
            trace!("rtcp reverse consideration not applied, n_members >= p_members");
            // this only applies if nmembers is less than pmembers
            return false;
        }
        if let Some(ref mut prev) = self.next_rtcp_send.time {
            if now > *prev {
                trace!("rtcp reverse consideration not applied, last timeout in the past");
                // timer should have fired already, nothing to do
                return false;
            }
            let dur = prev.saturating_duration_since(now);
            if self.next_rtcp_send.p_members > 0 {
                let member_factor = initial_n_members as f64 / self.next_rtcp_send.p_members as f64;
                // FIXME: Does something have to be adjusted here in feedback profiles for
                // T_rr_interval?
                *prev = now + dur.mul_f64(member_factor);
                self.next_rtcp_send.p_members = n_members;
                if let Some(last_rtcp) = self.last_rtcp_sent_times.front_mut() {
                    let dur = last_rtcp.saturating_duration_since(now);
                    *last_rtcp = now - dur.mul_f64(member_factor);
                }
                trace!("rtcp reverse consideration applied");
                return true;
            }
            trace!("rtcp reverse consideration not applied, p_members <= 0");
        } else {
            trace!("rtcp reverse consideration not applied, have not sent initial rtcp");
        }
        false
    }

    /// Handle receiving a RTCP packet.  The returned [`RtcpRecvReply`]s indicates anything the
    /// caller may need to handle.
    pub fn handle_rtcp_recv(
        &mut self,
        rtcp: Compound,
        rtcp_len: usize,
        from: Option<SocketAddr>,
        now: Instant,
        ntp_time: SystemTime,
    ) -> Vec<RtcpRecvReply> {
        trace!("Receive RTCP at {now:?}, ntp:{ntp_time:?}");
        // TODO: handle from: Option<SocketAddr>
        let mut replies = vec![];

        if self.bye_state.is_none() {
            self.update_rtcp_average(rtcp_len + UDP_IP_OVERHEAD_BYTES);
        }

        let mut reconsidered_timeout = false;
        for (i, p) in rtcp.enumerate() {
            trace!("recv rtcp {i}th packet: {p:?}");
            match p {
                // TODO: actually handle App packets
                Ok(Packet::App(_app)) => (),
                Ok(Packet::Bye(bye)) => {
                    // https://datatracker.ietf.org/doc/html/rfc3550#section-6.3.4
                    let n_members = self.n_members();
                    let mut check_reconsideration = false;
                    for ssrc in bye.ssrcs() {
                        if let Some(source) = self.remote_senders.get_mut(&ssrc) {
                            source.set_rtcp_from(from);
                            source.set_last_activity(now);
                            source.set_state(SourceState::Bye);
                            check_reconsideration = true;
                            replies.push(RtcpRecvReply::SsrcBye(ssrc));
                        } else if let Some(source) = self.remote_receivers.get_mut(&ssrc) {
                            source.set_last_activity(now);
                            source.set_state(SourceState::Bye);
                            check_reconsideration = true;
                            replies.push(RtcpRecvReply::SsrcBye(ssrc));
                        }
                        // XXX: do we need to handle an unknown ssrc here?
                        // TODO: signal rtcp timeout needs recalcuating
                    }
                    if let Some(ref mut state) = self.bye_state {
                        state.members += 1;
                        let n_members = state.members;
                        self.update_rtcp_average(rtcp_len + UDP_IP_OVERHEAD_BYTES);
                        if check_reconsideration
                            && self.rtcp_reverse_consideration(n_members, now)
                            && !reconsidered_timeout
                        {
                            replies.push(RtcpRecvReply::TimerReconsideration);
                            reconsidered_timeout = true;
                        }
                    } else if check_reconsideration
                        && self.rtcp_reverse_consideration(n_members, now)
                        && !reconsidered_timeout
                    {
                        replies.push(RtcpRecvReply::TimerReconsideration);
                        reconsidered_timeout = true;
                    }
                }
                Ok(Packet::Rr(rr)) => {
                    if let Some(source) = self.remote_senders.remove(&rr.ssrc()) {
                        let receiver = source.into_receive();
                        self.remote_receivers.insert(rr.ssrc(), receiver);
                    }

                    let source = self.remote_receivers.entry(rr.ssrc()).or_insert_with(|| {
                        replies.push(RtcpRecvReply::NewSsrc(rr.ssrc()));
                        RemoteReceiveSource::new(rr.ssrc())
                    });
                    source.set_rtcp_from(from);
                    source.set_state(SourceState::Normal);
                    source.set_last_activity(now);

                    for rb in rr.report_blocks() {
                        if let Some(reply) = self.handle_rb(rr.ssrc(), rb, from, now, ntp_time) {
                            replies.push(reply);
                        }
                    }
                }
                Ok(Packet::Sr(sr)) => {
                    if let Some(addr) = from {
                        if self.local_senders.contains_key(&sr.ssrc())
                            || self.local_receivers.contains_key(&sr.ssrc())
                        {
                            if self.handle_ssrc_conflict(addr, now) {
                                replies.push(RtcpRecvReply::SsrcCollision(sr.ssrc()));
                            }
                            continue;
                        }
                    }

                    if let Some(source) = self.remote_receivers.remove(&sr.ssrc()) {
                        let sender = source.into_send();
                        self.remote_senders.insert(sr.ssrc(), sender);
                    }

                    let source = self.remote_senders.entry(sr.ssrc()).or_insert_with(|| {
                        replies.push(RtcpRecvReply::NewSsrc(sr.ssrc()));
                        RemoteSendSource::new(sr.ssrc())
                    });
                    source.set_rtcp_from(from);
                    source.set_state(SourceState::Normal);
                    source.set_last_activity(now);
                    source.set_last_received_sr(
                        ntp_time,
                        sr.ntp_timestamp().into(),
                        sr.rtp_timestamp(),
                        sr.octet_count(),
                        sr.packet_count(),
                    );

                    replies.push(RtcpRecvReply::NewRtpNtp((
                        sr.ssrc(),
                        sr.rtp_timestamp(),
                        sr.ntp_timestamp(),
                    )));

                    for rb in sr.report_blocks() {
                        if let Some(reply) = self.handle_rb(sr.ssrc(), rb, from, now, ntp_time) {
                            replies.push(reply);
                        }
                    }
                }
                Ok(Packet::Sdes(sdes)) => {
                    for chunk in sdes.chunks() {
                        for item in chunk.items() {
                            if let Some(addr) = from {
                                if self.local_senders.contains_key(&chunk.ssrc())
                                    || self.local_receivers.contains_key(&chunk.ssrc())
                                {
                                    if self.handle_ssrc_conflict(addr, now) {
                                        replies.push(RtcpRecvReply::SsrcCollision(chunk.ssrc()));
                                    }
                                    continue;
                                }
                            }
                            if !matches!(
                                item.type_(),
                                SdesItem::CNAME
                                    | SdesItem::NAME
                                    | SdesItem::EMAIL
                                    | SdesItem::PHONE
                                    | SdesItem::LOC
                                    | SdesItem::TOOL
                                    | SdesItem::NOTE
                            ) {
                                // FIXME: handle unknown sdes items
                                continue;
                            }
                            if let Some(source) = self.remote_senders.get_mut(&chunk.ssrc()) {
                                source.set_rtcp_from(from);
                                source.received_sdes(item.type_(), item.value());
                                source.set_state(SourceState::Normal);
                                source.set_last_activity(now);
                            } else {
                                let source = self
                                    .remote_receivers
                                    .entry(chunk.ssrc())
                                    .or_insert_with(|| {
                                        replies.push(RtcpRecvReply::NewSsrc(chunk.ssrc()));
                                        RemoteReceiveSource::new(chunk.ssrc())
                                    });
                                source.set_rtcp_from(from);
                                source.received_sdes(item.type_(), item.value());
                                source.set_state(SourceState::Normal);
                                source.set_last_activity(now);
                            }

                            if item.type_() == SdesItem::CNAME {
                                if let Ok(s) = std::str::from_utf8(item.value()) {
                                    replies.push(RtcpRecvReply::NewCName((
                                        s.to_owned(),
                                        chunk.ssrc(),
                                    )));
                                }
                            }
                        }
                    }
                }
                Ok(Packet::PayloadFeedback(pf)) => {
                    if let Ok(_pli) = pf.parse_fci::<rtcp_types::Pli>() {
                        self.handle_remote_request_key_unit(
                            now,
                            &mut replies,
                            false,
                            pf.sender_ssrc(),
                            std::iter::once(pf.media_ssrc()),
                        );
                    } else if let Ok(fir) = pf.parse_fci::<rtcp_types::Fir>() {
                        self.handle_remote_request_key_unit(
                            now,
                            &mut replies,
                            true,
                            pf.sender_ssrc(),
                            // TODO: What to do with the sequence?
                            fir.entries().map(|entry| entry.ssrc()),
                        );
                    }
                }
                Ok(Packet::TransportFeedback(_)) | Ok(Packet::Xr(_)) | Ok(Packet::Unknown(_)) => (),
                // TODO: in RFC4585 profile, need to listen for feedback messages and remove any
                // that we would have sent
                Err(_) => (),
            }
        }
        self.update_point_to_point();
        replies
    }

    fn handle_remote_request_key_unit(
        &mut self,
        now: Instant,
        replies: &mut Vec<RtcpRecvReply>,
        fir: bool,
        sender_ssrc: u32,
        media_ssrcs: impl Iterator<Item = u32>,
    ) {
        if !self.remote_receivers.contains_key(&sender_ssrc)
            && !self.remote_senders.contains_key(&sender_ssrc)
        {
            trace!("No remote source known for sender ssrc {sender_ssrc}");
        }

        let mut ssrcs = Vec::new();
        for media_ssrc in media_ssrcs {
            let Some(sender) = self.local_senders.get(&media_ssrc) else {
                trace!("Not a local sender for ssrc {media_ssrc}");
                continue;
            };

            let Some(rb) = sender
                .received_report_blocks()
                .find(|(ssrc, _rb)| *ssrc == sender_ssrc)
            else {
                trace!("No RB for sender ssrc {sender_ssrc} yet");
                continue;
            };

            if let Some(sender) = self.remote_senders.get_mut(&sender_ssrc) {
                if !sender.remote_request_key_unit_allowed(now, rb.1) {
                    trace!("Requesting key-unit not allowed again yet");
                    continue;
                }
            } else if let Some(sender) = self.remote_receivers.get_mut(&sender_ssrc) {
                if !sender.remote_request_key_unit_allowed(now, rb.1) {
                    trace!("Requesting key-unit not allowed again yet");
                    continue;
                }
            }

            trace!("Requesting key-unit from sender ssrc {sender_ssrc} for media ssrc {media_ssrc} (fir: {fir})");
            ssrcs.push(media_ssrc);
        }

        if !ssrcs.is_empty() {
            replies.push(RtcpRecvReply::RequestKeyUnit { ssrcs, fir });
        }
    }

    fn generate_sr<'a>(
        &mut self,
        mut rtcp: CompoundBuilder<'a>,
        now: Instant,
        ntp_now: SystemTime,
        minimum: bool, // RFC 4585
        ssrcs_reported: &mut Vec<u32>,
    ) -> CompoundBuilder<'a> {
        // Don't include in an early reduced-size RTCP packet
        if minimum && self.reduced_size_rtcp_allowed() {
            return rtcp;
        }

        let ntp_time = system_time_to_ntp_time_u64(ntp_now);
        if self
            .local_senders
            .values()
            .any(|source| match source.state() {
                SourceState::Normal => true,
                SourceState::Probation(_) => false,
                SourceState::Bye => source.bye_sent_time().is_none(),
            })
        {
            let mut sender_srs = vec![];
            for sender in self.local_senders.values() {
                if sender.state() != SourceState::Normal {
                    continue;
                }
                if sender.state() == SourceState::Bye && sender.bye_sent_time().is_some() {
                    continue;
                }
                // pick one of the sender ssrc's if we are going to
                if self.internal_rtcp_sender_src.is_none() {
                    self.internal_rtcp_sender_src = Some(sender.ssrc());
                }
                // get last rtp sent timestamp
                let rtp_timestamp = sender
                    .last_rtp_sent_timestamp()
                    .map(|(last_rtp_ts, instant)| {
                        let dur_since_last_rtp = now.duration_since(instant);
                        trace!("last_rtp_ts: {last_rtp_ts}, dur since last rtp: {dur_since_last_rtp:?}");
                        // get the clock-rate for this source
                        last_rtp_ts + sender
                            .payload_type()
                            .and_then(|pt| self.clock_rate_from_pt(pt))
                            .and_then(|clock_rate| {
                                // assume that the rtp times and clock times advance at a rate
                                // close to 1.0 and do a direct linear extrapolation to get the rtp
                                // time for 'now'
                                (dur_since_last_rtp.as_nanos() as u64).mul_div_round(
                                    clock_rate as u64,
                                    gst::ClockTime::SECOND.nseconds(),
                                ).map(|v| (v & 0xffff_ffff) as u32)
                            })
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                let mut sr = SenderReport::builder(sender.ssrc())
                    .packet_count((sender.packet_count() & 0xffff_ffff) as u32)
                    .octet_count((sender.octet_count() & 0xffff_ffff) as u32)
                    .ntp_timestamp(ntp_time.as_u64())
                    .rtp_timestamp(rtp_timestamp);

                sender_srs.push((sender.ssrc(), ntp_now, ntp_time, rtp_timestamp));
                if !ssrcs_reported.iter().any(|&ssrc| ssrc == sender.ssrc()) {
                    ssrcs_reported.push(sender.ssrc());
                }

                // Don't include RBs in minimal RTCP packets
                if !minimum {
                    for sender in self.remote_senders.values() {
                        if sender.state() != SourceState::Normal {
                            continue;
                        }
                        let rb = sender.generate_report_block(ntp_now);
                        sr = sr.add_report_block(rb.into());
                        if !ssrcs_reported.iter().any(|&ssrc| ssrc == sender.ssrc()) {
                            ssrcs_reported.push(sender.ssrc());
                        }
                    }
                }
                rtcp = rtcp.add_packet(sr);

                // A minimal RTCP packet only contains a single SR
                if minimum {
                    break;
                }
            }
            for (ssrc, ntp_now, ntp_time, rtp_timestamp) in sender_srs {
                self.local_senders
                    .entry(ssrc)
                    .and_modify(|sender| sender.take_sr_snapshot(ntp_now, ntp_time, rtp_timestamp));
            }
        }
        rtcp
    }

    fn reduced_size_rtcp_allowed(&self) -> bool {
        self.reduced_size_rtcp && matches!(self.profile, RtpProfile::Avpf)
    }

    fn have_ssrc(&self, ssrc: u32) -> bool {
        self.local_senders.contains_key(&ssrc)
            || self.local_receivers.contains_key(&ssrc)
            || self.remote_senders.contains_key(&ssrc)
            || self.remote_receivers.contains_key(&ssrc)
    }

    pub fn internal_ssrc(&self) -> Option<u32> {
        self.internal_rtcp_sender_src
    }

    fn ensure_internal_send_src(&mut self) -> u32 {
        match self.internal_rtcp_sender_src {
            Some(ssrc) => ssrc,
            None => loop {
                let ssrc = generate_ssrc();
                if !self.have_ssrc(ssrc) {
                    let mut source = LocalReceiveSource::new(ssrc);
                    source.set_state(SourceState::Normal);
                    for (k, v) in self.sdes.iter() {
                        source.set_sdes_item(*k, v.as_bytes());
                    }
                    self.local_receivers.insert(ssrc, source);
                    self.internal_rtcp_sender_src = Some(ssrc);
                    return ssrc;
                }
            },
        }
    }

    fn generate_rr<'a>(
        &mut self,
        mut rtcp: CompoundBuilder<'a>,
        now: Instant,
        ntp_now: SystemTime,
        minimum: bool, // RFC 4585
        ssrcs_reported: &mut Vec<u32>,
    ) -> CompoundBuilder<'a> {
        // Don't include in an early reduced-size RTCP packet
        if minimum && self.reduced_size_rtcp_allowed() {
            return rtcp;
        }

        if self
            .local_senders
            .values()
            .all(|source| match source.state() {
                SourceState::Normal => false,
                SourceState::Probation(_) => true,
                SourceState::Bye => source.bye_sent_time().is_some(),
            })
        {
            let ssrc = self.ensure_internal_send_src();
            self.local_senders
                .entry(ssrc)
                .and_modify(|source| source.set_last_activity(now));
            self.local_receivers
                .entry(ssrc)
                .and_modify(|source| source.set_last_activity(now));
            let mut rr = ReceiverReport::builder(ssrc);

            // Don't include RBs in minimal RTCP packets
            if !minimum {
                for sender in self.remote_senders.values() {
                    if sender.state() != SourceState::Normal {
                        continue;
                    }
                    let rb = sender.generate_report_block(ntp_now);
                    rr = rr.add_report_block(rb.into());
                    if !ssrcs_reported.iter().any(|&ssrc| ssrc == sender.ssrc()) {
                        ssrcs_reported.push(sender.ssrc());
                    }
                }
            }

            rtcp = rtcp.add_packet(rr);
        }
        rtcp
    }

    fn generate_sdes<'a>(
        &self,
        rtcp: CompoundBuilder<'a>,
        minimum: bool, // RFC 4585
    ) -> CompoundBuilder<'a> {
        // Don't include in an early reduced-size RTCP packet
        if minimum && self.reduced_size_rtcp_allowed() {
            return rtcp;
        }

        let mut sdes = Sdes::builder();
        let mut have_chunk = false;
        for sender in self.local_senders.values() {
            let sdes_map = sender.sdes();
            if !sdes_map.is_empty() {
                let mut chunk = SdesChunk::builder(sender.ssrc());
                for (ty, val) in sdes_map {
                    if !minimum || *ty == SdesItem::CNAME {
                        chunk = chunk.add_item_owned(SdesItem::builder(*ty, val));
                    }
                }
                have_chunk = true;
                sdes = sdes.add_chunk(chunk);
            }
        }
        for receiver in self.local_receivers.values() {
            let sdes_map = receiver.sdes();
            if !sdes_map.is_empty() {
                let mut chunk = SdesChunk::builder(receiver.ssrc());
                for (ty, val) in sdes_map {
                    if !minimum || *ty == SdesItem::CNAME {
                        chunk = chunk.add_item_owned(SdesItem::builder(*ty, val));
                    }
                }
                have_chunk = true;
                sdes = sdes.add_chunk(chunk);
            }
        }
        if have_chunk {
            rtcp.add_packet(sdes)
        } else {
            rtcp
        }
    }

    fn find_bye_sources(&mut self) -> HashMap<String, Vec<u32>> {
        let mut reason_ssrcs = HashMap::new();
        for source in self
            .local_senders
            .values_mut()
            .filter(|source| source.state() == SourceState::Bye)
        {
            if source.bye_sent_time().is_none() {
                let reason = source
                    .bye_reason()
                    .cloned()
                    .unwrap_or_else(|| String::from("Bye"));
                let ssrcs = reason_ssrcs.entry(reason).or_insert_with(Vec::new);
                ssrcs.push(source.ssrc());
            }
        }
        for source in self
            .local_receivers
            .values_mut()
            .filter(|source| source.state() == SourceState::Bye)
        {
            if source.bye_sent_time().is_none() {
                let reason = source
                    .bye_reason()
                    .cloned()
                    .unwrap_or_else(|| String::from("Bye"));
                let ssrcs = reason_ssrcs.entry(reason).or_insert_with(Vec::new);
                ssrcs.push(source.ssrc());
            }
        }
        reason_ssrcs
    }

    fn generate_bye<'a>(
        &mut self,
        mut rtcp: CompoundBuilder<'a>,
        now: Instant,
    ) -> CompoundBuilder<'a> {
        let bye_reason_ssrcs = self.find_bye_sources();
        if !bye_reason_ssrcs.is_empty() {
            for (reason, ssrcs) in bye_reason_ssrcs.iter() {
                let mut bye = Bye::builder().reason_owned(reason);
                for ssrc in ssrcs.iter() {
                    self.pending_rtcp_send
                        .push_front(RtcpSendReply::SsrcBye(*ssrc));
                    bye = bye.add_source(*ssrc);
                    if let Some(source) = self.local_senders.get_mut(ssrc) {
                        source.bye_sent_at(now);
                    } else if let Some(source) = self.local_receivers.get_mut(ssrc) {
                        source.bye_sent_at(now);
                    }
                }
                rtcp = rtcp.add_packet(bye);
            }
        }
        rtcp
    }

    fn generate_pli<'a>(
        &mut self,
        mut rtcp: CompoundBuilder<'a>,
        _now: Instant,
    ) -> CompoundBuilder<'a> {
        let ssrc = self.ensure_internal_send_src();

        for source in self.remote_senders.values_mut() {
            let pli = source.generate_pli();
            if let Some(pli) = pli {
                debug!("Generating PLI for sender {}: {:?}", source.ssrc(), pli);
                rtcp = rtcp.add_packet(
                    rtcp_types::PayloadFeedback::builder_owned(pli)
                        .sender_ssrc(ssrc)
                        .media_ssrc(source.ssrc()),
                );
            }
        }
        rtcp
    }

    fn generate_fir<'a>(
        &mut self,
        mut rtcp: CompoundBuilder<'a>,
        _now: Instant,
    ) -> CompoundBuilder<'a> {
        let mut have_fir = false;
        let mut fir = rtcp_types::Fir::builder();

        for source in self.remote_senders.values_mut() {
            fir = source.generate_fir(fir, &mut have_fir);
        }

        if have_fir {
            let ssrc = self.ensure_internal_send_src();

            debug!("Generating FIR: {fir:?}");
            rtcp =
                rtcp.add_packet(rtcp_types::PayloadFeedback::builder_owned(fir).sender_ssrc(ssrc));
        }

        rtcp
    }

    // RFC 3550 6.3.5
    // FIXME: we should surface this information to the element in order
    // to perform clean up of the sync context
    fn handle_timeouts(&mut self, now: Instant) {
        trace!("handling rtcp timeouts");
        let td = RTCP_SOURCE_TIMEOUT_N_INTERVALS
            * self
                .deterministic_rtcp_duration(false)
                .max(Duration::from_secs(5));

        // delete all sources that are too old
        self.local_receivers
            .retain(|_ssrc, source| now - source.last_activity() < td);
        self.remote_senders
            .retain(|_ssrc, source| now - source.last_activity() < td);
        self.remote_receivers
            .retain(|_ssrc, source| now - source.last_activity() < td);

        // There is a SHOULD about performing RTCP reverse timer consideration here if any sources
        // were timed out, however we are here before calculating the next rtcp timeout so are
        // covered already with a changing number of members.
        // If we call this outside of rtcp handling, then rtcp_reverse_consideration() would need to
        // be called.

        // switch senders that haven't sent in a while to receivers
        if self.last_rtcp_sent_times.len() >= 2 {
            let two_rtcp_ago = *self.last_rtcp_sent_times.back().unwrap();
            let removed_senders = self
                .local_senders
                .iter()
                .filter_map(|(&ssrc, source)| {
                    trace!(
                        "now: {now:?}, last activity: {:?} two_rtcp_ago: {:?}",
                        source.last_activity(),
                        two_rtcp_ago
                    );
                    if source.last_activity() < two_rtcp_ago {
                        Some(ssrc)
                    } else {
                        None
                    }
                })
                .inspect(|source| trace!("ssrc {source} has become a receiver"))
                .collect::<Vec<_>>();

            for ssrc in removed_senders {
                if let Some(source) = self.local_senders.remove(&ssrc) {
                    let new_source = source.into_receive();
                    self.local_receivers.insert(new_source.ssrc(), new_source);
                }
            }
        }

        // remove outdated conflicting addresses
        self.conflicting_addresses
            .retain(|_addr, time| now - *time < RTCP_ADDRESS_CONFLICT_TIMEOUT);
    }

    /// Produce a RTCP packet (or `None` if it is too early to send a RTCP packet).  After this call returns,
    /// the next time to send a RTCP packet can be retrieved from `poll_rtcp_send_timeout`
    // TODO: return RtcpPacketBuilder thing
    pub fn poll_rtcp_send(&mut self, now: Instant, ntp_now: SystemTime) -> Option<RtcpSendReply> {
        if let Some(event) = self.pending_rtcp_send.pop_back() {
            return Some(event);
        }

        let Some(next_rtcp_send) = self.next_rtcp_send.time else {
            trace!("no next check time yet");
            return None;
        };
        let is_early = self.next_early_rtcp_time.is_some() && !self.last_rtcp_sent_times.is_empty();

        if is_early {
            if let Some(next_early_rtcp_time) = self.next_early_rtcp_time {
                if now < next_early_rtcp_time {
                    trace!("next early time {next_early_rtcp_time:?} not reached at {now:?}, nothing to produce");
                    return None;
                }
            }
        } else {
            if now < next_rtcp_send {
                trace!("next time {next_rtcp_send:?} not reached at {now:?}, nothing to produce");
                return None;
            }

            // timer reconsideration
            self.update_point_to_point();
            let interval = self.rtcp_interval();
            let test_next_rtcp_time = self.last_rtcp_handle_time.unwrap() + interval;
            if test_next_rtcp_time > now {
                trace!("timer reconsideration considers this wakeup {now:?} too early, nothing to produce. reconsidered time {test_next_rtcp_time:?}");
                self.next_rtcp_send.time = Some(test_next_rtcp_time);
                return None;
            }
        }

        debug!(
            "generating rtcp packet at {now:?}, ntp:{ntp_now:?}, duration since last rtcp {:?}",
            self.last_rtcp_sent_times
                .front()
                .copied()
                .unwrap_or(now)
                .duration_since(now)
        );

        let (data, ssrcs_reported) = {
            let mut rtcp = Compound::builder();
            let mut ssrcs_reported = vec![];

            // TODO: implement round robin of sr/rrs
            rtcp = self.generate_sr(rtcp, now, ntp_now, is_early, &mut ssrcs_reported);
            rtcp = self.generate_rr(rtcp, now, ntp_now, is_early, &mut ssrcs_reported);
            rtcp = self.generate_sdes(rtcp, is_early);
            rtcp = self.generate_pli(rtcp, now);
            rtcp = self.generate_fir(rtcp, now);
            rtcp = self.generate_bye(rtcp, now);

            let size = rtcp.calculate_size().unwrap();
            // TODO: handle dropping data
            assert!(size < RTCP_MTU);
            let mut data = vec![0; size];
            rtcp.write_into(&mut data).unwrap();
            (data, ssrcs_reported)
        };

        for ssrc in ssrcs_reported {
            if let Some(receiver) = self.remote_senders.get_mut(&ssrc) {
                receiver.update_last_rtcp();
            }
        }

        self.update_rtcp_average(data.len() + UDP_IP_OVERHEAD_BYTES);

        if !is_early {
            self.handle_timeouts(now);

            self.update_point_to_point();
            let mut interval = self.rtcp_interval();
            self.rtcp_interval = Some(interval);
            // Handle T_rr_interval
            if self.profile.is_feedback() && !self.min_rtcp_interval.is_zero() {
                let new_interval = interval.max(rtcp_dither(self.min_rtcp_interval));
                trace!("Updating interval from {interval:?} to {new_interval:?}");
                interval = new_interval;
            }
            self.next_rtcp_send = RtcpTimeMembers {
                time: Some(now + interval),
                p_members: self.n_members(),
            };
            trace!("next rtcp time {:?}", self.next_rtcp_send.time);
            self.last_rtcp_sent_times.push_front(now);
            while self.last_rtcp_sent_times.len() > 2 {
                self.last_rtcp_sent_times.pop_back();
            }
            self.bye_state = None;
        }
        self.last_rtcp_handle_time = Some(now);
        self.next_early_rtcp_time = None;
        Some(RtcpSendReply::Data(data))
    }

    /// Returns the next time to send a RTCP packet.
    pub fn poll_rtcp_send_timeout(&mut self, now: Instant) -> Option<Instant> {
        trace!(
            "poll-rtcp-send-timeout early time {:?}, next {:?}",
            self.next_early_rtcp_time,
            self.next_rtcp_send.time
        );
        if let Some(early_time) = self.next_early_rtcp_time {
            return Some(early_time);
        }
        if self.next_rtcp_send.time.is_none() {
            self.update_point_to_point();
            let interval = self.rtcp_interval();
            self.rtcp_interval = Some(interval);
            self.last_rtcp_handle_time = Some(now);
            self.next_rtcp_send = RtcpTimeMembers {
                time: Some(now + interval),
                p_members: self.n_members(),
            };
            trace!(
                "poll-rtcp-send-timeout initial rtcp time {:?}",
                self.next_rtcp_send.time
            );
        }
        self.next_rtcp_send.time
    }

    fn deterministic_rtcp_duration(&self, we_sent: bool) -> Duration {
        let n_senders = self.n_senders() as u64;
        let n_members = self.n_members() as u64;
        let session_bandwidth = self.session_bandwidth();
        // 5% of the session bandwidth, or the minimum of 400B/s
        let rtcp_bw = (session_bandwidth / 20).max(RTCP_MIN_BANDWIDTH);

        let (n, rtcp_bw) = if n_senders * 4 <= n_members {
            if we_sent {
                (n_senders, rtcp_bw / 4)
            } else {
                (n_members - n_senders, rtcp_bw / 4 * 3)
            }
        } else {
            (n_members, rtcp_bw)
        };

        let min_rtcp_interval = if self.profile.is_feedback() {
            // RFC 4585 3.4d), 3.5.1
            //
            // If not the first RTCP then Tmin is initialized to 0, otherwise to 1s.
            // Also for a multi-party session it is initialized to 0.
            if self.is_point_to_point
                && !self.last_rtcp_sent_times.is_empty()
                && self.bye_state.is_none()
            {
                Duration::ZERO
            } else {
                Duration::from_secs(1)
            }
        } else if !self.last_rtcp_sent_times.is_empty() && self.bye_state.is_none() {
            self.min_rtcp_interval
        } else {
            self.min_rtcp_interval / 2
        };

        // 1_000_000_000 / (e-1.5)
        let compensation_ns = 820_829_366;
        trace!(
            "members {n}, RTCP bandwidth {rtcp_bw}, average RTCP size {}",
            self.average_rtcp_size
        );
        let t_nanos = (compensation_ns.mul_div_round(
            self.average_rtcp_size as u64 * n.max(1),
            rtcp_bw.max(1) as u64,
        ))
        .unwrap()
        .max(min_rtcp_interval.as_nanos() as u64);
        trace!("deterministic rtcp interval {t_nanos}ns");
        Duration::from_nanos(t_nanos)
    }

    fn session_bandwidth(&self) -> usize {
        // TODO: allow to be externally provided
        self.local_senders
            .values()
            .filter(|source| source.state() == SourceState::Normal)
            .map(|source| source.bitrate())
            .sum::<usize>()
            + self
                .remote_senders
                .values()
                .filter(|source| source.state() == SourceState::Normal)
                .map(|source| source.bitrate())
                .sum::<usize>()
    }

    fn calculated_rtcp_duration(&self, we_sent: bool) -> Duration {
        // From RFC 3550: only active data senders may use the reduced minimum
        let dur = self.deterministic_rtcp_duration(we_sent);
        rtcp_dither(dur)
    }

    // returns whether to keep the feedback
    fn request_early_rtcp(&mut self, now: Instant, max_delay: Duration) -> RequestEarlyRtcpResult {
        // Implementation of RFC 4585 3.5.2
        if !self.profile.is_feedback() {
            warn!("requested early RTCP without relevant feedback RTP profile, ignoring");
            return RequestEarlyRtcpResult::NotScheduled;
        }

        if let Some(early_time) = self.next_early_rtcp_time {
            if now + max_delay >= early_time {
                debug!("early RTCP already scheduled for {early_time:?}");
                return RequestEarlyRtcpResult::Scheduled;
            } else {
                debug!("early RTCP already scheduled for {early_time:?} but too late");
                return RequestEarlyRtcpResult::NotScheduled;
            }
        }
        let Some(next_rtcp_send_time) = self.next_rtcp_send.time else {
            debug!("No regular RTCP scheduled yet, scheduling now");
            return RequestEarlyRtcpResult::TimerReconsideration;
        };

        // no regular RTCP sent yet, we cannot send early without the first regular RTCP being sent
        let Some(&last_rtcp_sent) = self.last_rtcp_sent_times.front() else {
            if now + max_delay >= next_rtcp_send_time {
                debug!("early RTCP can't be scheduled until first regular RTCP is sent but regular RTCP scheduled early enough");
                return RequestEarlyRtcpResult::Scheduled;
            } else {
                debug!("early RTCP can't be scheduled until first regular RTCP is sent and regular RTCP scheduled too late");
                return RequestEarlyRtcpResult::NotScheduled;
            }
        };

        self.update_point_to_point();

        // safe as all previous invariants ensure that the `handle_time` must have been set
        let last_rtcp_handle_time = self.last_rtcp_handle_time.unwrap();

        let t_rr = self.rtcp_interval.unwrap();
        let t_dither_max = if self.is_point_to_point {
            Duration::from_secs(0)
        } else {
            t_rr / 2
        };

        if now + t_dither_max > next_rtcp_send_time {
            if now + max_delay >= next_rtcp_send_time {
                debug!("early RTCP not scheduled because regular RTCP is early enough");
                return RequestEarlyRtcpResult::Scheduled;
            } else {
                debug!("early RTCP can't be scheduled because regular RTCP is scheduled soon but too late");
                return RequestEarlyRtcpResult::NotScheduled;
            }
        }

        let (allow_early, offset) = if last_rtcp_sent == last_rtcp_handle_time {
            // last rtcp was a regular rtcp, we can send early after that
            (true, Duration::ZERO)
        } else if last_rtcp_handle_time + t_rr <= now + max_delay {
            // 1. last rtcp packet was not regular rtcp so must be early rtcp
            // 2. More than t_rr has passed since so a regular rtcp packet has been suppressed thus
            //    allowing early send again
            (
                true,
                (last_rtcp_handle_time + t_rr).saturating_duration_since(now),
            )
        } else {
            debug!("Can't allow early RTCP again yet");
            (false, Duration::ZERO)
        };

        if !allow_early {
            if next_rtcp_send_time - now < max_delay {
                debug!("early RTCP not scheduled but regular RTCP scheduled time is soon enough");
                RequestEarlyRtcpResult::Scheduled
            } else {
                debug!("early RTCP not scheduled and regular RTCP scheduled time is too late");
                RequestEarlyRtcpResult::NotScheduled
            }
        } else {
            let mut rng = rand::rng();
            self.next_early_rtcp_time = Some(now + t_dither_max.mul_f64(rng.random()) + offset);
            debug!("early RTCP scheduled at {:?}", self.next_early_rtcp_time);
            RequestEarlyRtcpResult::TimerReconsideration
        }
    }

    pub fn schedule_bye(&mut self, reason: &str, now: Instant) {
        if self.bye_state.is_some() {
            return;
        }

        for source in self.local_senders.values_mut() {
            source.mark_bye(reason);
        }
        for source in self.local_receivers.values_mut() {
            source.mark_bye(reason);
        }

        if self.n_members() <= 50 {
            return;
        }

        self.bye_state = Some(ByeState {
            members: 1,
            pmembers: 1,
        });
        // tp is reset to tc
        self.last_rtcp_sent_times = VecDeque::new();
        self.last_rtcp_sent_times.push_front(now);
        // FIXME: use actual BYE packet size
        self.average_rtcp_size = 100;
        self.update_point_to_point();
        let mut interval = self.rtcp_interval();
        self.rtcp_interval = Some(interval);
        // Handle T_rr_interval
        if self.profile.is_feedback() && !self.min_rtcp_interval.is_zero() {
            let new_interval = interval.max(rtcp_dither(self.min_rtcp_interval));
            trace!("Updating interval from {interval:?} to {new_interval:?}");
            interval = new_interval;
        }
        self.next_rtcp_send = RtcpTimeMembers {
            time: Some(now + interval),
            p_members: self.n_members(),
        };
        trace!("next rtcp time {:?}", self.next_rtcp_send.time);
    }

    fn rtcp_interval(&self) -> Duration {
        let interval = self
            .calculated_rtcp_duration(!self.local_senders.is_empty() && self.bye_state.is_none());
        trace!("Calculated RTCP interval {interval:?}");
        interval
    }

    /// Retrieve a list of all ssrc's currently handled by this session
    pub fn ssrcs(&self) -> impl Iterator<Item = u32> + '_ {
        self.local_senders
            .keys()
            .chain(self.remote_senders.keys())
            .chain(self.local_receivers.keys())
            .chain(self.remote_receivers.keys())
            .cloned()
    }

    /// Retrieve a local send source by ssrc
    pub fn local_send_source_by_ssrc(&self, ssrc: u32) -> Option<&LocalSendSource> {
        self.local_senders.get(&ssrc)
    }

    /// Retrieve a local receive source by ssrc
    pub fn local_receive_source_by_ssrc(&self, ssrc: u32) -> Option<&LocalReceiveSource> {
        self.local_receivers.get(&ssrc)
    }

    /// Retrieve a remote send source by ssrc
    pub fn remote_send_source_by_ssrc(&self, ssrc: u32) -> Option<&RemoteSendSource> {
        self.remote_senders.get(&ssrc)
    }

    /// Retrieve a remote receive source by ssrc
    pub fn remote_receive_source_by_ssrc(&self, ssrc: u32) -> Option<&RemoteReceiveSource> {
        self.remote_receivers.get(&ssrc)
    }

    pub fn mut_local_send_source_by_ssrc(&mut self, ssrc: u32) -> Option<&mut LocalSendSource> {
        self.local_senders.get_mut(&ssrc)
    }

    #[cfg(test)]
    fn mut_remote_sender_source_by_ssrc(&mut self, ssrc: u32) -> Option<&mut RemoteSendSource> {
        self.remote_senders.get_mut(&ssrc)
    }

    pub(crate) fn request_remote_key_unit(
        &mut self,
        now: Instant,
        typ: KeyUnitRequestType,
        ssrc: u32,
    ) -> Vec<RequestRemoteKeyUnitReply> {
        let mut replies = Vec::new();

        if !self.remote_senders.contains_key(&ssrc) {
            trace!("No remote sender with ssrc {ssrc} known");
            return replies;
        };

        debug!("Requesting remote key-unit for ssrc {ssrc} of type {typ:?}");

        // FIXME: Use hard-coded 5s interval here
        let res = self.request_early_rtcp(now, Duration::from_secs(5));
        if res == RequestEarlyRtcpResult::TimerReconsideration {
            replies.push(RequestRemoteKeyUnitReply::TimerReconsideration);
        }

        if res != RequestEarlyRtcpResult::NotScheduled {
            let source = self.remote_senders.get_mut(&ssrc).unwrap();
            source.request_remote_key_unit(now, typ);
        }

        replies
    }
}

fn generate_cname() -> String {
    let mut rng = rand::rng();
    let user = rng.random::<u32>();
    let host = rng.random::<u32>();
    format!("user{user}@{host:#}")
}

fn generate_ssrc() -> u32 {
    let mut rng = rand::rng();
    rng.random::<u32>()
}

fn rtcp_dither(dur: Duration) -> Duration {
    let mut rng = rand::rng();
    // need a factor in [0.5, 1.5]
    let factor = rng.random::<f64>();
    dur.mul_f64(factor + 0.5)
}

#[cfg(test)]
pub(crate) mod tests {
    use rtp_types::RtpPacketBuilder;

    use crate::rtpbin2::time::NtpTime;

    use super::*;

    pub(crate) fn init_logs() {
        let _ = gst::init();
        use crate::rtpbin2::internal::GstRustLogger;
        GstRustLogger::install();
    }

    const TEST_PT: u8 = 96;
    const TEST_CLOCK_RATE: u32 = 90000;

    #[test]
    fn receive_probation() {
        init_logs();
        let mut session = Session::new();
        let from = "127.0.0.1:1000".parse().unwrap();
        let now = Instant::now();
        let mut held = vec![];
        for seq_no in 0..5 {
            let mut rtp_data = [0; 128];
            let len = RtpPacketBuilder::new()
                .payload_type(TEST_PT)
                .ssrc(0x12345678)
                .sequence_number(seq_no)
                .write_into(&mut rtp_data)
                .unwrap();
            let rtp_data = &rtp_data[..len];
            let packet = RtpPacket::parse(rtp_data).unwrap();
            let mut ret = session.handle_recv(&packet, Some(from), now);
            match seq_no {
                // probation
                0 => {
                    if let RecvReply::NewSsrc(ssrc, pt) = ret {
                        assert_eq!(ssrc, 0x12345678);
                        assert_eq!(pt, TEST_PT);
                        if let RecvReply::Hold(id) = session.handle_recv(&packet, Some(from), now) {
                            held.push(id);
                        } else {
                            unreachable!();
                        }
                    } else {
                        unreachable!();
                    }
                }
                1 => {
                    while let RecvReply::Forward(id) = ret {
                        let pos = held.iter().position(|&held_id| held_id == id).unwrap();
                        held.remove(pos);
                        ret = session.handle_recv(&packet, Some(from), now);
                    }
                    assert!(held.is_empty());
                    assert_eq!(ret, RecvReply::Passthrough);
                }
                2..=4 => {
                    assert_eq!(ret, RecvReply::Passthrough)
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn generate_rtp_packet(ssrc: u32, seq_no: u16, rtp_ts: u32, payload_len: usize) -> Vec<u8> {
        init_logs();
        let mut rtp_data = [0; 1200];
        let payload = vec![1; payload_len];
        let len = RtpPacketBuilder::new()
            .payload_type(TEST_PT)
            .ssrc(ssrc)
            .sequence_number(seq_no)
            .timestamp(rtp_ts)
            .payload(payload.as_slice())
            .write_into(rtp_data.as_mut_slice())
            .unwrap();
        rtp_data[..len].to_vec()
    }

    fn increment_rtcp_times(
        old_now: Instant,
        new_now: Instant,
        ntp_now: SystemTime,
    ) -> (Instant, SystemTime) {
        (new_now, ntp_now + new_now.duration_since(old_now))
    }

    fn next_rtcp_packet(
        session: &mut Session,
        mut now: Instant,
        mut ntp_now: SystemTime,
    ) -> (RtcpSendReply, Instant, SystemTime) {
        let mut ret = None;
        while ret.is_none() {
            ret = session.poll_rtcp_send(now, ntp_now);
            if let Some(rtcp) = ret {
                return (rtcp, now, ntp_now);
            } else {
                (now, ntp_now) = increment_rtcp_times(
                    now,
                    session.poll_rtcp_send_timeout(now).unwrap(),
                    ntp_now,
                );
            }
        }
        unreachable!();
    }

    #[test]
    fn send_new_ssrc() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);

        let now = Instant::now();
        let rtp_data = generate_rtp_packet(0x12345678, 100, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(0x12345678, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);
    }

    fn session_recv_first_packet_disable_probation(
        session: &mut Session,
        packet: &RtpPacket<'_>,
        now: Instant,
    ) {
        assert_eq!(
            session.handle_recv(packet, None, now),
            RecvReply::NewSsrc(packet.ssrc(), packet.payload_type())
        );
        let src = session
            .mut_remote_sender_source_by_ssrc(packet.ssrc())
            .unwrap();
        src.set_probation_packets(0);
    }

    #[test]
    fn receive_disable_probation() {
        init_logs();
        let mut session = Session::new();
        let now = Instant::now();
        let rtp_data = generate_rtp_packet(0x12345678, 100, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );
    }

    #[test]
    fn receive_two_ssrc_rr() {
        init_logs();
        let mut session = Session::new();
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrcs = [0x12345678, 0x87654321];

        let rtp_data = generate_rtp_packet(ssrcs[0], 100, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let rtp_data = generate_rtp_packet(ssrcs[1], 200, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );
        assert_eq!(
            session.handle_recv(
                &RtpPacket::parse(&generate_rtp_packet(ssrcs[1], 207, 0, 4)).unwrap(),
                None,
                now
            ),
            RecvReply::Passthrough
        );

        let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut n_rb_ssrcs = 0;
        let mut found_sdes_cname = false;
        let mut sdes_ssrc = None;
        let mut rr_ssrc = None;

        for p in rtcp {
            match p {
                Ok(Packet::Rr(rr)) => {
                    rr_ssrc = Some(rr.ssrc());
                    for rb in rr.report_blocks() {
                        if ssrcs.contains(&rb.ssrc()) {
                            n_rb_ssrcs += 1;
                        }
                        match rb.ssrc() {
                            0x12345678 => {
                                assert_eq!(rb.extended_sequence_number() & 0xffff, 100);
                                assert_eq!(rb.cumulative_lost(), 0xFFFFFF); // -1 in 24-bit
                                assert_eq!(rb.fraction_lost(), 0);
                            }
                            0x87654321 => {
                                assert_eq!(rb.extended_sequence_number() & 0xffff, 207);
                                assert_eq!(rb.cumulative_lost(), 6);
                                assert_eq!(rb.fraction_lost(), 182);
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                Ok(Packet::Sdes(sdes)) => {
                    for chunk in sdes.chunks() {
                        sdes_ssrc = Some(chunk.ssrc());
                        for item in chunk.items() {
                            if item.type_() == SdesItem::CNAME {
                                found_sdes_cname = true;
                            } else {
                                unreachable!();
                            }
                        }
                    }
                }
                _ => unreachable!("{p:?}"),
            }
        }
        assert_eq!(n_rb_ssrcs, ssrcs.len());
        assert!(found_sdes_cname);
        assert_eq!(sdes_ssrc, rr_ssrc);
    }

    #[test]
    fn send_two_ssrc_sr() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);

        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrcs = [0x12345678, 0x87654321];

        // generate packets at the 'same time' as rtcp so some calculated timestamps will match
        let (now, ntp_now) =
            increment_rtcp_times(now, session.poll_rtcp_send_timeout(now).unwrap(), ntp_now);
        // ensure that timer reconsideration will not suppress
        let (now, ntp_now) = increment_rtcp_times(now, now + RTCP_MIN_REPORT_INTERVAL, ntp_now);

        let rtp_data = generate_rtp_packet(ssrcs[0], 100, 4, 1024);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrcs[0], 96)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let rtp_data = generate_rtp_packet(ssrcs[1], 200, 4, 1024);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrcs[1], TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let rtcp_data = session.poll_rtcp_send(now, ntp_now).unwrap();
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };
        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut n_rb_ssrcs = 0;
        for p in rtcp {
            match p {
                Ok(Packet::Sr(sr)) => {
                    assert_eq!(sr.n_reports(), 0);
                    if ssrcs.contains(&sr.ssrc()) {
                        n_rb_ssrcs += 1;
                    }
                    // we sent 1 packet on each ssrc, rtcp should reflect that
                    assert_eq!(sr.packet_count(), 1);
                    assert_eq!(sr.octet_count() as usize, 1024);
                    assert_eq!(
                        sr.ntp_timestamp(),
                        system_time_to_ntp_time_u64(ntp_now).as_u64()
                    );
                    assert_eq!(sr.rtp_timestamp(), 4);
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }
        assert_eq!(n_rb_ssrcs, ssrcs.len());
    }

    #[test]
    fn receive_two_ssrc_sr() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);

        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrcs = [0x12345678, 0x87654321];

        let rtp_data = generate_rtp_packet(ssrcs[0], 100, 4, 8);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let rtp_data = generate_rtp_packet(ssrcs[1], 200, 20, 12);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(
                SenderReport::builder(ssrcs[0])
                    .ntp_timestamp(system_time_to_ntp_time_u64(ntp_now).as_u64())
                    .packet_count(1)
                    .octet_count(8)
                    .rtp_timestamp(4),
            )
            .add_packet(
                SenderReport::builder(ssrcs[1])
                    .ntp_timestamp(system_time_to_ntp_time_u64(ntp_now).as_u64())
                    .packet_count(2)
                    .octet_count(24)
                    .rtp_timestamp(20),
            )
            .write_into(&mut data)
            .unwrap();
        let data = &data[..len];
        let rtcp = Compound::parse(data).unwrap();

        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, None, now, ntp_now),
            vec![
                RtcpRecvReply::NewRtpNtp((
                    ssrcs[0],
                    4,
                    system_time_to_ntp_time_u64(ntp_now).as_u64()
                )),
                RtcpRecvReply::NewRtpNtp((
                    ssrcs[1],
                    20,
                    system_time_to_ntp_time_u64(ntp_now).as_u64()
                ))
            ]
        );

        let (rtcp_data, _new_now, new_ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::parse(&rtcp_data).unwrap();
        for p in rtcp {
            match p {
                Ok(Packet::Rr(rr)) => {
                    assert_eq!(rr.n_reports(), 2);
                    let mut rb_ssrcs = rr.report_blocks().map(|rb| rb.ssrc()).collect::<Vec<_>>();
                    rb_ssrcs.sort();
                    assert_eq!(rb_ssrcs, &ssrcs);
                    for rb in rr.report_blocks() {
                        assert_eq!(
                            rb.last_sender_report_timestamp(),
                            system_time_to_ntp_time_u64(ntp_now).as_u32()
                        );
                        assert_eq!(
                            rb.delay_since_last_sender_report_timestamp(),
                            NtpTime::from_duration(new_ntp_now.duration_since(ntp_now).unwrap())
                                .as_u32()
                        );
                        if rb.ssrc() == ssrcs[0] {
                            assert_eq!(rb.extended_sequence_number() & 0xffff, 100);
                        } else if rb.ssrc() == ssrcs[1] {
                            assert_eq!(rb.extended_sequence_number() & 0xffff, 200);
                        } else {
                            unreachable!()
                        }
                    }
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn send_receiver_two_ssrc_sr_rr() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        session.set_min_rtcp_interval(Duration::from_secs(1));

        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrcs = [0x12345678, 0x87654321];

        // get the next rtcp packet times and send at the same time
        let (now, ntp_now) =
            increment_rtcp_times(now, session.poll_rtcp_send_timeout(now).unwrap(), ntp_now);
        // ensure that timer reconsideration will not suppress
        let (now, ntp_now) = increment_rtcp_times(now, now + RTCP_MIN_REPORT_INTERVAL, ntp_now);

        // send from two ssrcs
        let rtp_data = generate_rtp_packet(ssrcs[0], 100, 0, 1024);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrcs[0], TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let rtp_data = generate_rtp_packet(ssrcs[1], 200, 0, 1024);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrcs[1], TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let rtcp_data = session.poll_rtcp_send(now, ntp_now).unwrap();
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };
        trace!("rtcp data {rtcp_data:?}");
        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut n_sr_ssrcs = 0;
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Sr(sr)) => {
                    // no reports as there are no receivers
                    assert_eq!(sr.n_reports(), 0);
                    if ssrcs.contains(&sr.ssrc()) {
                        n_sr_ssrcs += 1;
                    }
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }
        assert_eq!(n_sr_ssrcs, ssrcs.len());

        let recv_ssrcs = [0x11223344, 0xFFEEDDCC];

        let rtp_data = generate_rtp_packet(recv_ssrcs[0], 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let rtp_data = generate_rtp_packet(recv_ssrcs[1], 600, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        trace!("rtcp data {rtcp_data:?}");
        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut n_sr_ssrcs = 0;
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Sr(sr)) => {
                    assert_eq!(sr.n_reports(), 2);
                    if ssrcs.contains(&sr.ssrc()) {
                        n_sr_ssrcs += 1;
                    }
                    let mut rb_ssrcs = sr.report_blocks().map(|rb| rb.ssrc()).collect::<Vec<_>>();
                    rb_ssrcs.sort();
                    assert_eq!(rb_ssrcs, recv_ssrcs);
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }
        assert_eq!(n_sr_ssrcs, ssrcs.len());
    }

    #[test]
    fn session_internal_sender_ssrc() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);

        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let recv_ssrc = 0x11223344;

        let rtp_data = generate_rtp_packet(recv_ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::parse(&rtcp_data).unwrap();
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Rr(rr)) => {
                    // no reports as there are no receivers
                    assert_eq!(rr.n_reports(), 1);
                    let mut rb_ssrcs = rr.report_blocks().map(|rb| rb.ssrc()).collect::<Vec<_>>();
                    rb_ssrcs.sort();
                    assert_eq!(rb_ssrcs, &[recv_ssrc]);
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn sender_source_timeout() {
        init_logs();
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);

        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x12345678;

        let rtp_data = generate_rtp_packet(ssrc, 200, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let (rtcp_data, now, ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::parse(&rtcp_data).unwrap();
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Sr(sr)) => {
                    assert_eq!(sr.n_reports(), 0);
                    assert_eq!(sr.ssrc(), ssrc);
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }

        let mut seen_rr = false;
        for _ in 0..=5 {
            let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
            let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
                unreachable!();
            };
            let rtcp = Compound::parse(&rtcp_data).unwrap();
            for p in rtcp {
                trace!("{p:?}");
                match p {
                    Ok(Packet::Sr(sr)) => {
                        assert_eq!(sr.n_reports(), 0);
                        assert_eq!(sr.ssrc(), ssrc);
                    }
                    Ok(Packet::Rr(rr)) => {
                        assert_eq!(rr.ssrc(), ssrc);
                        seen_rr |= true;
                    }
                    Ok(Packet::Sdes(_)) => (),
                    _ => unreachable!(),
                }
            }
        }
        assert!(seen_rr);
    }

    #[test]
    fn ignore_recv_bye_for_local_sender() {
        // test that receiving a BYE for our (local) senders is ignored
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let (rtcp_data, now, ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };
        let rtcp = Compound::parse(&rtcp_data).unwrap();

        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Sr(sr)) => {
                    assert_eq!(sr.n_reports(), 0);
                    assert_eq!(sr.ssrc(), ssrc);
                }
                Ok(Packet::Sdes(_)) => (),
                _ => unreachable!(),
            }
        }

        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(Bye::builder().add_source(ssrc))
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();

        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, None, now, ntp_now),
            vec![]
        );
        let source = session.mut_local_send_source_by_ssrc(ssrc).unwrap();
        assert_eq!(source.state(), SourceState::Normal);
    }

    #[test]
    fn ssrc_collision_on_send() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;
        let from = "127.0.0.1:8080".parse().unwrap();

        // add remote ssrc
        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(Sdes::builder().add_chunk(
                SdesChunk::builder(ssrc).add_item(SdesItem::builder(SdesItem::CNAME, "cname")),
            ))
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![
                RtcpRecvReply::NewSsrc(ssrc),
                RtcpRecvReply::NewCName(("cname".to_string(), ssrc))
            ]
        );

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::SsrcCollision(ssrc)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Drop);

        // add ssrc as if our packets are being looped.  As we have already discovered the
        // conflicting address, these looped packets should be dropped.
        let new_ssrc = 0x44332211;
        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(Sdes::builder().add_chunk(
                SdesChunk::builder(new_ssrc).add_item(SdesItem::builder(SdesItem::CNAME, "cname")),
            ))
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![
                RtcpRecvReply::NewSsrc(new_ssrc),
                RtcpRecvReply::NewCName(("cname".to_string(), new_ssrc))
            ]
        );

        let rtp_data = generate_rtp_packet(new_ssrc, 510, 10, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(session.handle_send(&packet, now), SendReply::Drop);
    }

    #[test]
    fn ssrc_collision_on_recv() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;
        let from = "127.0.0.1:8080".parse().unwrap();

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(Sdes::builder().add_chunk(
                SdesChunk::builder(ssrc).add_item(SdesItem::builder(SdesItem::CNAME, "cname")),
            ))
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![RtcpRecvReply::SsrcCollision(ssrc)]
        );
    }

    #[test]
    fn ssrc_collision_third_party() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ssrc = 0x11223344;
        let from1 = "127.0.0.1:8080".parse().unwrap();
        let from2 = "127.0.0.2:8080".parse().unwrap();

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, Some(from1), now),
            RecvReply::Passthrough
        );

        // packet from a different address should be dropped as a third party collision
        assert_eq!(
            session.handle_recv(&packet, Some(from2), now),
            RecvReply::Ignore
        );

        // packet from a original address should still succeed
        assert_eq!(
            session.handle_recv(&packet, Some(from1), now),
            RecvReply::Passthrough
        );
    }

    #[test]
    fn bye_remote_sender() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        // send initial rtcp
        let (rtcp_data, now, ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(_rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::builder().add_packet(Bye::builder().add_source(ssrc));
        let mut data = vec![0; 128];
        let len = rtcp.write_into(&mut data).unwrap();
        let data = &data[..len];

        let rtcp = Compound::parse(data).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, None, now, ntp_now),
            vec![
                RtcpRecvReply::SsrcBye(ssrc),
                RtcpRecvReply::TimerReconsideration
            ]
        );
        let source = session.mut_remote_sender_source_by_ssrc(ssrc).unwrap();
        assert_eq!(source.state(), SourceState::Bye);
    }

    #[test]
    fn bye_local_sender() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        // send initial rtcp
        let (rtcp_data, now, ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(_rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let source = session.mut_local_send_source_by_ssrc(ssrc).unwrap();
        source.mark_bye("Cya");
        assert_eq!(source.state(), SourceState::Bye);

        // data after bye should be dropped
        assert_eq!(session.handle_send(&packet, now), SendReply::Drop);

        let (rtcp_data, now, ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut received_bye = false;
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Bye(bye)) => {
                    assert_eq!(bye.reason(), Some(b"Cya".as_ref()));
                    assert_eq!(bye.ssrcs().next(), Some(ssrc));
                    // bye must not be followed by any other packets
                    received_bye = true;
                }
                Ok(Packet::Sdes(_sdes)) => {
                    assert!(!received_bye);
                }
                _ => unreachable!(),
            }
        }
        assert!(received_bye);
        let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::SsrcBye(bye_ssrc) = rtcp_data else {
            unreachable!();
        };
        assert_eq!(bye_ssrc, ssrc);
    }

    #[test]
    fn early_rtcp() {
        let mut session = Session::new();
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        session.set_profile(RtpProfile::Avpf);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let ssrc = 0x11223344;
        let send_ssrc = 0x55667788;
        let send2_ssrc = 0x99aabbcc;

        let rtp_data = generate_rtp_packet(ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        session_recv_first_packet_disable_probation(&mut session, &packet, now);
        assert_eq!(
            session.handle_recv(&packet, None, now),
            RecvReply::Passthrough
        );

        let rtp_data = generate_rtp_packet(send_ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(send_ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        let rtp_data = generate_rtp_packet(send2_ssrc, 500, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(
            session.handle_send(&packet, now),
            SendReply::NewSsrc(send2_ssrc, TEST_PT)
        );
        assert_eq!(session.handle_send(&packet, now), SendReply::Passthrough);

        // complete first regular rtcp
        let (rtcp_data, now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(_rtcp_data) = rtcp_data else {
            unreachable!();
        };

        let source = session.mut_local_send_source_by_ssrc(send_ssrc).unwrap();
        source.set_sdes_item(SdesItem::NAME, b"name");

        // request early rtcp.  The resulting RTCP packet, will only have the CNAME sdes item and a
        // single SR/RR.
        session.request_early_rtcp(now, RTCP_MIN_REPORT_INTERVAL);
        assert!(session.next_early_rtcp_time.is_some());

        let (rtcp_data, _now, _ntp_now) = next_rtcp_packet(&mut session, now, ntp_now);
        let RtcpSendReply::Data(rtcp_data) = rtcp_data else {
            unreachable!();
        };
        let rtcp = Compound::parse(&rtcp_data).unwrap();
        let mut n_sr_ssrc = 0;
        let mut n_sdes = 0;
        for p in rtcp {
            trace!("{p:?}");
            match p {
                Ok(Packet::Sr(sr)) => {
                    assert!([send_ssrc, send2_ssrc].contains(&sr.ssrc()));
                    n_sr_ssrc += 1;
                }
                Ok(Packet::Sdes(sdes)) => {
                    assert_eq!(n_sdes, 0);
                    for chunk in sdes.chunks() {
                        assert!([send_ssrc, send2_ssrc].contains(&chunk.ssrc()));
                        for item in chunk.items() {
                            assert_eq!(item.type_(), SdesItem::CNAME);
                            n_sdes += 1;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(n_sdes, 2);
        assert_eq!(n_sr_ssrc, 1);
    }

    #[test]
    fn point_to_point() {
        let mut session = Session::new();
        assert!(session.is_point_to_point);
        session.set_pt_clock_rate(TEST_PT, TEST_CLOCK_RATE);
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let recv_ssrc = 0x11223344;
        let recv2_ssrc = 0x55667788;
        let from = "127.0.0.1:8080".parse().unwrap();

        // single remote cname is still point to point
        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(
                Sdes::builder().add_chunk(
                    SdesChunk::builder(recv_ssrc)
                        .add_item(SdesItem::builder(SdesItem::CNAME, "cname1")),
                ),
            )
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![
                RtcpRecvReply::NewSsrc(recv_ssrc),
                RtcpRecvReply::NewCName(("cname1".to_string(), recv_ssrc))
            ]
        );
        assert!(session.is_point_to_point);

        // multiple ssrc but same cname is still point to point
        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(
                Sdes::builder().add_chunk(
                    SdesChunk::builder(recv_ssrc)
                        .add_item(SdesItem::builder(SdesItem::CNAME, "cname1")),
                ),
            )
            .add_packet(
                Sdes::builder().add_chunk(
                    SdesChunk::builder(recv2_ssrc)
                        .add_item(SdesItem::builder(SdesItem::CNAME, "cname1")),
                ),
            )
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![
                RtcpRecvReply::NewCName(("cname1".to_string(), recv_ssrc)),
                RtcpRecvReply::NewSsrc(recv2_ssrc),
                RtcpRecvReply::NewCName(("cname1".to_string(), recv2_ssrc))
            ]
        );
        assert!(session.is_point_to_point);

        // multiple ssrc and different cname is NOT point to point
        let mut data = vec![0; 128];
        let len = Compound::builder()
            .add_packet(
                Sdes::builder().add_chunk(
                    SdesChunk::builder(recv_ssrc)
                        .add_item(SdesItem::builder(SdesItem::CNAME, "cname1")),
                ),
            )
            .add_packet(
                Sdes::builder().add_chunk(
                    SdesChunk::builder(recv2_ssrc)
                        .add_item(SdesItem::builder(SdesItem::CNAME, "cname2")),
                ),
            )
            .write_into(&mut data)
            .unwrap();
        let rtcp = Compound::parse(&data[..len]).unwrap();
        assert_eq!(
            session.handle_rtcp_recv(rtcp, len, Some(from), now, ntp_now),
            vec![
                RtcpRecvReply::NewCName(("cname1".to_string(), recv_ssrc)),
                RtcpRecvReply::NewCName(("cname2".to_string(), recv2_ssrc))
            ]
        );
        assert!(!session.is_point_to_point);
    }
}
