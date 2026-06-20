use crate::utils::ExtendedSeqnum;
use rtp_types::RtpPacket;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct Stats {
    num_late: u64,
    num_lost: u64,
    num_duplicates: u64,
    num_pushed: u64,
}

impl From<Stats> for gst::Structure {
    fn from(stats: Stats) -> gst::Structure {
        gst::Structure::builder("application/x-rtp-jitterbuffer-stats")
            .field("num-late", stats.num_late)
            .field("num-duplicates", stats.num_duplicates)
            .field("num-lost", stats.num_lost)
            .field("num-pushed", stats.num_pushed)
            .build()
    }
}

#[derive(Debug)]
pub struct JitterBuffer {
    packet_counter: usize,
    // A set of extended seqnums that we've already seen through,
    // intentionally trimmed separately from the items list so that
    // we can detect duplicates after the first copy has exited the
    // queue
    seqnums: BTreeSet<u64>,
    items: BTreeSet<Item>,
    latency: Duration,
    // Arrival time, PTS
    base_times: Option<(Instant, u64)>,
    last_output_seqnum: Option<u64>,
    extended_seqnum: ExtendedSeqnum,
    last_input_ts: Option<u64>,
    stats: Stats,
    flushing: bool,
    can_forward_packets_when_empty: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollResult {
    Forward { id: usize, discont: bool },
    Drop(usize),
    Timeout(Instant),
    Empty,
    Flushing,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QueueResult {
    Forward(usize),
    Queued(usize),
    Late,
    Duplicate,
    Flushing,
}

#[derive(Eq, Debug)]
struct Item {
    id: usize,
    // If not set, this is an event / query
    pts: Option<u64>,
    seqnum: u64,
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        self.seqnum
            .cmp(&other.seqnum)
            .then(match (self.pts, other.pts) {
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl JitterBuffer {
    pub fn new(latency: Duration) -> Self {
        Self {
            packet_counter: 0,
            seqnums: BTreeSet::new(),
            items: BTreeSet::new(),
            latency,
            base_times: None,
            last_input_ts: None,
            last_output_seqnum: None,
            extended_seqnum: ExtendedSeqnum::default(),
            stats: Stats {
                num_late: 0,
                num_lost: 0,
                num_duplicates: 0,
                num_pushed: 0,
            },
            flushing: true,
            can_forward_packets_when_empty: false,
        }
    }

    pub fn queue_serialized_item(&mut self) -> QueueResult {
        if self.items.is_empty() {
            let id = self.packet_counter;
            self.packet_counter += 1;
            return QueueResult::Forward(id);
        }

        let id = self.packet_counter;
        self.packet_counter += 1;
        let item = Item {
            id,
            pts: None,
            seqnum: (*self.seqnums.last().unwrap_or(&0)),
        };
        self.items.insert(item);
        trace!("Queued serialized item and assigned ID {id}");

        QueueResult::Queued(id)
    }

    pub fn set_flushing(&mut self, flushing: bool) {
        trace!("Flush changed from {} to {flushing}", self.flushing);
        self.flushing = flushing;
        self.last_output_seqnum = None;
        self.can_forward_packets_when_empty = false;
    }

    pub fn queue_packet(&mut self, rtp: &RtpPacket, mut pts: u64, now: Instant) -> QueueResult {
        if self.flushing {
            return QueueResult::Flushing;
        }

        // From this point on we always work with extended sequence numbers
        let seqnum = self.extended_seqnum.next(rtp.sequence_number());

        if let Some(ts) = self.last_input_ts {
            pts = pts.max(ts);
        }

        self.last_input_ts = Some(pts);

        self.base_times.get_or_insert_with(|| {
            debug!("Selected base times {now:?} {pts}");

            (now, pts)
        });

        // Maintain (and trim) our seqnum list for duplicate detection
        while self.seqnums.len() >= u16::MAX as usize {
            debug!("Trimming");
            self.seqnums.pop_first();
        }

        if self.seqnums.contains(&seqnum) {
            trace!(
                "Duplicated packet {} (extended {})",
                rtp.sequence_number(),
                seqnum,
            );
            self.stats.num_duplicates += 1;
            return QueueResult::Duplicate;
        }

        self.seqnums.insert(seqnum);

        if let Some(last_output_seqnum) = self.last_output_seqnum {
            if last_output_seqnum >= seqnum {
                debug!(
                    "Late packet {} (extended {})",
                    rtp.sequence_number(),
                    seqnum
                );
                self.stats.num_late += 1;
                return QueueResult::Late;
            }
        }

        if self.items.is_empty()
            // can forward after the first packet's deadline has been reached
            && self.can_forward_packets_when_empty
            && self
                .last_output_seqnum
                .is_some_and(|last_output_seqnum| seqnum == last_output_seqnum.wrapping_add(1))
        {
            // No packets enqueued & seqnum is in order => can forward it immediately
            self.last_output_seqnum = Some(seqnum);
            let id = self.packet_counter;
            self.packet_counter += 1;
            self.stats.num_pushed += 1;

            return QueueResult::Forward(id);
        }

        // TODO: if the segnum base is known (i.e. first seqnum in RTSP)
        //       we could also Forward the initial Packet.

        let id = self.packet_counter;
        self.packet_counter += 1;
        let item = Item {
            id,
            pts: Some(pts),
            seqnum,
        };

        if !self.items.insert(item) {
            unreachable!()
        }

        trace!("Queued RTP packet with ts {pts}, assigned ID {id}");

        QueueResult::Queued(id)
    }

    pub fn poll(&mut self, now: Instant) -> PollResult {
        if self.flushing {
            if let Some(item) = self.items.pop_first() {
                return PollResult::Drop(item.id);
            } else {
                return PollResult::Flushing;
            }
        }

        trace!("Polling at {now:?}");

        let Some(item) = self.items.first() else {
            return PollResult::Empty;
        };

        // If an event / query is at the top of the queue, it can be forwarded immediately
        let Some(pts) = item.pts else {
            let item = self.items.pop_first().unwrap();
            return PollResult::Forward {
                id: item.id,
                discont: false,
            };
        };

        let Some((base_instant, base_ts)) = self.base_times else {
            return PollResult::Empty;
        };

        let duration_since_base_instant = now - base_instant;

        trace!("Duration since base instant {duration_since_base_instant:?}");

        let ts = pts.checked_sub(base_ts).unwrap();
        let deadline = Duration::from_nanos(ts) + self.latency;

        trace!(
            "Considering packet {} with ts {ts}, deadline is {deadline:?}",
            item.id
        );

        if deadline <= duration_since_base_instant {
            debug!("Packet with id {} is ready", item.id);

            let discont = match self.last_output_seqnum {
                None => true,
                Some(last_output_seq_ext) => {
                    let gap = item.seqnum - last_output_seq_ext;

                    self.stats.num_lost += gap - 1;

                    gap != 1
                }
            };

            self.last_output_seqnum = Some(item.seqnum);
            // Safe unwrap, we know the queue isn't empty at this point
            let packet = self.items.pop_first().unwrap();

            self.stats.num_pushed += 1;
            self.can_forward_packets_when_empty = true;

            PollResult::Forward {
                id: packet.id,
                discont,
            }
        } else {
            trace!("Packet with id {} is not ready", item.id);
            PollResult::Timeout(base_instant + deadline)
        }
    }

    pub fn stats(&self) -> gst::Structure {
        self.stats.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtpbin2::session::tests::generate_rtp_packet;

    #[test]
    fn empty() {
        let mut jb = JitterBuffer::new(Duration::from_secs(1));
        jb.set_flushing(false);

        let now = Instant::now();

        assert_eq!(jb.poll(now), PollResult::Empty);
    }

    #[test]
    fn receive_one_packet_no_latency() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();

        let now = Instant::now();

        let QueueResult::Queued(id) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(jb.poll(now), PollResult::Forward { id, discont: true });
    }

    #[test]
    fn receive_one_packet_with_latency() {
        let mut jb = JitterBuffer::new(Duration::from_secs(1));
        jb.set_flushing(false);

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();

        let mut now = Instant::now();

        let QueueResult::Queued(id) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(
            jb.poll(now),
            PollResult::Timeout(now + Duration::from_secs(1))
        );

        now += Duration::from_secs(1);
        now -= Duration::from_nanos(1);

        assert_eq!(
            jb.poll(now),
            PollResult::Timeout(now + Duration::from_nanos(1))
        );

        now += Duration::from_nanos(1);

        assert_eq!(jb.poll(now), PollResult::Forward { id, discont: true });
    }

    #[test]
    fn ordered_packets_no_latency() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();

        let QueueResult::Queued(id_first) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        let rtp_data = generate_rtp_packet(0x12345678, 1, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id_second) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_first,
                discont: true
            }
        );
        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_second,
                discont: false
            }
        );
    }

    #[test]
    fn ordered_packets_no_latency_with_gap() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id_first) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        let rtp_data = generate_rtp_packet(0x12345678, 2, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id_second) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_first,
                discont: true
            }
        );
        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_second,
                discont: true
            }
        );
    }

    #[test]
    fn misordered_packets_no_latency() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 1, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(jb.poll(now), PollResult::Forward { id, discont: true });

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(jb.queue_packet(&packet, 0, now), QueueResult::Late);

        // Try and push a duplicate
        let rtp_data = generate_rtp_packet(0x12345678, 1, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(jb.queue_packet(&packet, 0, now), QueueResult::Duplicate);

        // We do accept future sequence numbers up to a distance of at least i16::MAX
        let rtp_data = generate_rtp_packet(0x12345678, i16::MAX as u16 + 1, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(jb.poll(now), PollResult::Forward { id, discont: true });

        // But no further
        let rtp_data = generate_rtp_packet(0x12345678, 2, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        assert_eq!(jb.queue_packet(&packet, 0, now), QueueResult::Late);
    }

    #[test]
    fn ordered_packets_with_latency() {
        let mut jb = JitterBuffer::new(Duration::from_secs(1));
        jb.set_flushing(false);

        let mut now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id_first) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        assert_eq!(
            jb.poll(now),
            PollResult::Timeout(now + Duration::from_secs(1))
        );

        let rtp_data = generate_rtp_packet(0x12345678, 1, 180000, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Queued(id_second) = jb.queue_packet(&packet, 2_000_000_000, now) else {
            unreachable!()
        };

        assert_eq!(
            jb.poll(now),
            PollResult::Timeout(now + Duration::from_secs(1))
        );

        now += Duration::from_secs(1);

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_first,
                discont: true
            }
        );

        assert_eq!(
            jb.poll(now),
            PollResult::Timeout(now + Duration::from_secs(2))
        );

        now += Duration::from_secs(2);

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_second,
                discont: false
            }
        );
    }

    fn assert_stats(
        jb: &JitterBuffer,
        num_late: u64,
        num_lost: u64,
        num_duplicates: u64,
        num_pushed: u64,
    ) {
        let stats = jb.stats();

        assert_eq!(stats.get::<u64>("num-late").unwrap(), num_late);
        assert_eq!(stats.get::<u64>("num-lost").unwrap(), num_lost);
        assert_eq!(stats.get::<u64>("num-duplicates").unwrap(), num_duplicates);
        assert_eq!(stats.get::<u64>("num-pushed").unwrap(), num_pushed);
    }

    #[test]
    fn stats() {
        let mut jb = JitterBuffer::new(Duration::from_secs(1));
        jb.set_flushing(false);

        let mut now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        jb.queue_packet(&packet, 0, now);

        assert_stats(&jb, 0, 0, 0, 0);

        // At this point pushing the same packet in before it gets output
        // results in an increment of the duplicate stat
        jb.queue_packet(&packet, 0, now);
        assert_stats(&jb, 0, 0, 1, 0);

        now += Duration::from_secs(1);
        let _ = jb.poll(now);

        assert_stats(&jb, 0, 0, 1, 1);

        // Pushing it after the first version got output also results in
        // an increment of the duplicate stat
        jb.queue_packet(&packet, 0, now);
        assert_stats(&jb, 0, 0, 2, 1);

        // Then after a packet with seqnum 2 goes through, the lost
        // stat must be incremented by 1 (as packet with seqnum 1 went missing)
        let rtp_data = generate_rtp_packet(0x12345678, 2, 9000, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        jb.queue_packet(&packet, 100_000_000, now);

        now += Duration::from_millis(100);
        let _ = jb.poll(now);
        assert_stats(&jb, 0, 1, 2, 2);

        // If the packet with seqnum 1 does arrive after that, it should be
        // considered both late and lost
        let rtp_data = generate_rtp_packet(0x12345678, 1, 4500, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        jb.queue_packet(&packet, 50_000_000, now);

        let _ = jb.poll(now);
        assert_stats(&jb, 1, 1, 2, 2);

        // Finally if it arrives again it should be considered a duplicate,
        // and will have achieved the dubious honor of simultaneously being
        // lost, late and duplicated
        jb.queue_packet(&packet, 50_000_000, now);

        let _ = jb.poll(now);
        assert_stats(&jb, 1, 1, 3, 2);
    }

    #[test]
    fn serialized_items() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();

        let QueueResult::Forward(id_first_serialized_item) = jb.queue_serialized_item() else {
            unreachable!()
        };
        assert_eq!(id_first_serialized_item, 0);

        // query has been forwarded immediately
        assert_eq!(jb.poll(now), PollResult::Empty);

        let QueueResult::Queued(id_first_packet) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };
        assert_eq!(id_first_packet, id_first_serialized_item + 1);
        let QueueResult::Queued(id_second_serialized_item) = jb.queue_serialized_item() else {
            unreachable!()
        };
        assert_eq!(id_second_serialized_item, id_first_packet + 1);

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_first_packet,
                discont: true
            }
        );

        assert_eq!(
            jb.poll(now),
            PollResult::Forward {
                id: id_second_serialized_item,
                discont: false
            }
        );

        let rtp_data = generate_rtp_packet(0x12345678, 1, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();
        let QueueResult::Forward(id_second_packet) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };
        assert_eq!(id_second_packet, id_second_serialized_item + 1);
        assert_eq!(jb.poll(now), PollResult::Empty);
    }

    #[test]
    fn flushing_queue() {
        let mut jb = JitterBuffer::new(Duration::from_secs(0));
        jb.set_flushing(false);

        let now = Instant::now();

        let rtp_data = generate_rtp_packet(0x12345678, 0, 0, 4);
        let packet = RtpPacket::parse(&rtp_data).unwrap();

        let QueueResult::Forward(_id_first_serialized_item) = jb.queue_serialized_item() else {
            unreachable!()
        };

        let QueueResult::Queued(id_first) = jb.queue_packet(&packet, 0, now) else {
            unreachable!()
        };

        let QueueResult::Queued(id_second_serialized_item) = jb.queue_serialized_item() else {
            unreachable!()
        };

        // Everything after this should eventually return flushing, poll() will instruct to drop
        // everything stored and then return flushing indefinitely.
        jb.set_flushing(true);
        assert_eq!(jb.queue_packet(&packet, 0, now), QueueResult::Flushing);

        assert_eq!(jb.poll(now), PollResult::Drop(id_first));
        assert_eq!(jb.poll(now), PollResult::Drop(id_second_serialized_item));
        assert_eq!(jb.poll(now), PollResult::Flushing);
        assert_eq!(jb.poll(now), PollResult::Flushing);

        jb.set_flushing(false);
        assert_eq!(jb.poll(now), PollResult::Empty);
    }
}
