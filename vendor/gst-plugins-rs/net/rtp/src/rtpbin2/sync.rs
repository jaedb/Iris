use gst::glib;
use gst::prelude::MulDiv;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use crate::utils::ExtendedTimestamp;

use super::time::NtpTime;

#[derive(Default, Debug)]
struct Ssrc {
    cname: Option<Arc<str>>,
    clock_rate: Option<u32>,
    extended_timestamp: ExtendedTimestamp,
    last_sr_ntp_timestamp: Option<NtpTime>,
    last_sr_rtp_ext: Option<u64>,
    // Arrival, RTP timestamp (extended), PTS (potentially skew-corrected)
    base_times: Option<(u64, u64, u64)>,
    current_delay: Option<i64>,
    observations: Observations,
}

impl Ssrc {
    fn new(clock_rate: Option<u32>) -> Self {
        Self {
            clock_rate,
            ..Default::default()
        }
    }

    fn reset_times(&mut self) {
        self.extended_timestamp = ExtendedTimestamp::default();
        self.last_sr_ntp_timestamp = None;
        self.last_sr_rtp_ext = None;
        self.base_times = None;
        self.current_delay = None;
        self.observations = Observations::default();
    }

    /* Returns whether the caller should reset timing associated
     * values for this ssrc (eg largest delay) */
    fn set_clock_rate(&mut self, clock_rate: u32) -> bool {
        if Some(clock_rate) == self.clock_rate {
            // No changes
            return false;
        }

        self.clock_rate = Some(clock_rate);
        self.reset_times();
        true
    }

    fn add_sender_report(&mut self, rtp_timestamp: u32, ntp_timestamp: u64) {
        self.last_sr_rtp_ext = Some(self.extended_timestamp.next(rtp_timestamp));
        self.last_sr_ntp_timestamp = Some(ntp_timestamp.into());
        // Reset so that the next call to calculate_pts recalculates the NTP / RTP delay
        self.current_delay = None;
    }
}

#[derive(Debug)]
struct CnameLargestDelay {
    largest_delay: i64,
    all_sync: bool,
}

/// Govern how to pick presentation timestamps for packets
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRtpBin2TimestampingMode")]
pub enum TimestampingMode {
    /// Simply use arrival time as timestamp
    #[allow(dead_code)]
    #[enum_value(name = "Use arrival time as timestamp", nick = "arrival")]
    Arrival,
    /// Use RTP timestamps as is
    #[allow(dead_code)]
    #[enum_value(name = "Use RTP timestamps as is", nick = "rtp")]
    Rtp,
    /// Correct skew to synchronize sender and receiver clocks
    #[default]
    #[enum_value(
        name = "Correct skew to synchronize sender and receiver clocks",
        nick = "skew"
    )]
    Skew,
}

#[derive(Debug)]
pub struct Context {
    ssrcs: HashMap<u32, Ssrc>,
    mode: TimestampingMode,
    cnames_to_ssrcs: HashMap<Arc<str>, Vec<u32>>,
    cname_to_largest_delays: HashMap<Arc<str>, CnameLargestDelay>,
}

impl Context {
    pub fn new(mode: TimestampingMode) -> Self {
        Self {
            ssrcs: HashMap::new(),
            mode,
            cnames_to_ssrcs: HashMap::new(),
            cname_to_largest_delays: HashMap::new(),
        }
    }

    pub fn set_clock_rate(&mut self, ssrc_val: u32, clock_rate: u32) {
        if let Some(ssrc) = self.ssrcs.get_mut(&ssrc_val) {
            if ssrc.set_clock_rate(clock_rate) {
                debug!("{ssrc_val:#08x} times reset after clock rate change");
                if let Some(ref cname) = ssrc.cname {
                    self.cname_to_largest_delays.remove(cname);
                }
            }
        } else {
            self.ssrcs.insert(ssrc_val, Ssrc::new(Some(clock_rate)));
        }
    }

    pub fn has_clock_rate(&self, ssrc_val: u32) -> bool {
        self.ssrcs
            .get(&ssrc_val)
            .is_some_and(|ssrc| ssrc.clock_rate.is_some())
    }

    fn disassociate(&mut self, ssrc_val: u32, cname: &str) {
        self.cname_to_largest_delays.remove(cname);

        if let Some(ssrcs) = self.cnames_to_ssrcs.get_mut(cname) {
            ssrcs.retain(|&other| other != ssrc_val);
        }
    }

    // FIXME: call this on timeouts / BYE (maybe collisions too?)
    #[allow(dead_code)]
    pub fn remove_ssrc(&mut self, ssrc_val: u32) {
        if let Some(ssrc) = self.ssrcs.remove(&ssrc_val) {
            debug!("{ssrc_val:#08x} ssrc removed");
            if let Some(ref cname) = ssrc.cname {
                self.disassociate(ssrc_val, cname)
            }
        }
    }

    pub fn associate(&mut self, ssrc_val: u32, cname: &str) {
        let ssrc = self
            .ssrcs
            .entry(ssrc_val)
            .or_insert_with(|| Ssrc::new(None));

        let cname = Arc::<str>::from(cname);
        if let Some(ref old_cname) = ssrc.cname {
            if old_cname == &cname {
                return;
            }

            ssrc.cname = Some(cname.clone());
            self.disassociate(ssrc_val, cname.as_ref());
        } else {
            ssrc.cname = Some(cname.clone());
        }

        let ssrcs = self.cnames_to_ssrcs.entry(cname.clone()).or_default();
        ssrcs.push(ssrc_val);
        // Recalculate a new largest delay next time calculate_pts is called
        self.cname_to_largest_delays.remove(cname.as_ref());
    }

    pub fn add_sender_report(&mut self, ssrc_val: u32, rtp_timestamp: u32, ntp_timestamp: u64) {
        debug!("Adding new sender report for ssrc {ssrc_val:#08x}");

        let ssrc = self
            .ssrcs
            .entry(ssrc_val)
            .or_insert_with(|| Ssrc::new(None));

        debug!(
            "Latest NTP time: {:?}",
            NtpTime::from(ntp_timestamp).as_duration().unwrap()
        );

        ssrc.add_sender_report(rtp_timestamp, ntp_timestamp)
    }

    pub fn calculate_pts(
        &mut self,
        ssrc_val: u32,
        timestamp: u32,
        arrival_time: u64,
    ) -> (u64, Option<NtpTime>) {
        let ssrc = self.ssrcs.get_mut(&ssrc_val).unwrap();
        let clock_rate = ssrc.clock_rate.unwrap() as u64;

        // Calculate an extended timestamp, calculations only work with extended timestamps
        // from that point on
        let rtp_ext_ns = ssrc
            .extended_timestamp
            .next(timestamp)
            .mul_div_round(1_000_000_000, clock_rate)
            .unwrap();

        // Now potentially correct the skew by observing how RTP times and arrival times progress
        let mut pts = match self.mode {
            TimestampingMode::Skew => {
                let (skew_corrected, discont) = ssrc.observations.process(rtp_ext_ns, arrival_time);
                trace!("{ssrc_val:#08x} using skew corrected RTP ext: {skew_corrected}");

                if discont {
                    ssrc.reset_times();
                    debug!("{ssrc_val:#08x} times reset after observations discontinuity");
                    if let Some(ref cname) = ssrc.cname {
                        self.cname_to_largest_delays.remove(cname);
                    }
                }

                skew_corrected
            }
            TimestampingMode::Rtp => {
                trace!("{ssrc_val:#08x} using uncorrected RTP ext: {rtp_ext_ns}");

                rtp_ext_ns
            }
            TimestampingMode::Arrival => {
                trace!("{ssrc_val:#08x} using arrival time: {arrival_time}");

                arrival_time
            }
        };

        // Determine the first arrival time and the first RTP time for that ssrc
        if ssrc.base_times.is_none() {
            ssrc.base_times = Some((arrival_time, rtp_ext_ns, pts));
        }

        let (base_arrival_time, base_rtp_ext_ns, base_pts) = ssrc.base_times.unwrap();

        // Base the PTS on the first arrival time
        pts += base_arrival_time;
        trace!("{ssrc_val:#08x} added up base arrival time: {pts}");
        // Now subtract the base PTS we calculated
        pts = pts.saturating_sub(base_pts);
        trace!("{ssrc_val:#08x} subtracted base PTS: {base_pts}");

        trace!("{ssrc_val:#08x} PTS prior to potential SR offsetting: {pts}");

        let mut ntp_time: Option<NtpTime> = None;

        // TODO: add property for enabling / disabling offsetting based on
        // NTP / RTP mapping, ie inter-stream synchronization
        if let Some((last_sr_ntp, last_sr_rtp_ext)) =
            ssrc.last_sr_ntp_timestamp.zip(ssrc.last_sr_rtp_ext)
        {
            let last_sr_rtp_ext_ns = last_sr_rtp_ext
                .mul_div_round(1_000_000_000, clock_rate)
                .unwrap();

            // We have a new SR, we can now figure out an NTP time and calculate how it
            // relates to arrival times
            if ssrc.current_delay.is_none() {
                if let Some(base_ntp_time) = if base_rtp_ext_ns > last_sr_rtp_ext_ns {
                    let rtp_range_ns = base_rtp_ext_ns - last_sr_rtp_ext_ns;

                    (last_sr_ntp.as_duration().unwrap().as_nanos() as u64).checked_add(rtp_range_ns)
                } else {
                    let rtp_range_ns = last_sr_rtp_ext_ns - base_rtp_ext_ns;

                    (last_sr_ntp.as_duration().unwrap().as_nanos() as u64).checked_sub(rtp_range_ns)
                } {
                    trace!(
                        "{ssrc_val:#08x} Base NTP time on first packet after new SR is {:?} ({:?})",
                        base_ntp_time,
                        Duration::from_nanos(base_ntp_time)
                    );

                    if base_ntp_time < base_arrival_time {
                        ssrc.current_delay = Some(base_arrival_time as i64 - base_ntp_time as i64);
                    } else {
                        ssrc.current_delay =
                            Some(-(base_ntp_time as i64 - base_arrival_time as i64));
                    }

                    trace!("{ssrc_val:#08x} Current delay is {:?}", ssrc.current_delay);

                    if let Some(ref cname) = ssrc.cname {
                        // We should recalculate a new largest delay for this CNAME
                        self.cname_to_largest_delays.remove(cname);
                    }
                } else {
                    warn!("{ssrc_val:#08x} Invalid NTP RTP time mapping, waiting for next SR");
                    ssrc.last_sr_ntp_timestamp = None;
                    ssrc.last_sr_rtp_ext = None;
                }
            }

            ntp_time = if rtp_ext_ns > last_sr_rtp_ext_ns {
                let rtp_range_ns = Duration::from_nanos(rtp_ext_ns - last_sr_rtp_ext_ns);

                last_sr_ntp
                    .as_duration()
                    .unwrap()
                    .checked_add(rtp_range_ns)
                    .map(NtpTime::from_duration)
            } else {
                let rtp_range_ns = Duration::from_nanos(last_sr_rtp_ext_ns - rtp_ext_ns);

                last_sr_ntp
                    .as_duration()
                    .unwrap()
                    .checked_sub(rtp_range_ns)
                    .map(NtpTime::from_duration)
            };
        }

        // Finally, if we have a CNAME for this SSRC and we have managed to calculate
        // a delay for all the other ssrcs for this CNAME, we can calculate by how much
        // we need to delay this stream to sync it with the others, if at all.
        if let Some(cname) = ssrc.cname.clone() {
            let delay = ssrc.current_delay;
            let cname_largest_delay = self
                .cname_to_largest_delays
                .entry(cname.clone())
                .or_insert_with(|| {
                    let mut cname_largest_delay = CnameLargestDelay {
                        largest_delay: i64::MIN,
                        all_sync: true,
                    };

                    trace!("{ssrc_val:#08x} searching for new largest delay");

                    let ssrc_vals = self.cnames_to_ssrcs.get(&cname).unwrap();

                    for ssrc_val in ssrc_vals {
                        let ssrc = self.ssrcs.get(ssrc_val).unwrap();

                        if let Some(delay) = ssrc.current_delay {
                            trace!("ssrc {ssrc_val:#08x} has delay {delay}",);

                            if delay > cname_largest_delay.largest_delay {
                                cname_largest_delay.largest_delay = delay;
                            }
                        } else {
                            trace!("{ssrc_val:#08x} has no delay calculated yet");
                            cname_largest_delay.all_sync = false;
                        }
                    }

                    cname_largest_delay
                });

            trace!("{ssrc_val:#08x} Largest delay is {cname_largest_delay:?}");

            if cname_largest_delay.all_sync {
                let offset = (cname_largest_delay.largest_delay - delay.unwrap()) as u64;

                trace!("{ssrc_val:#08x} applying offset {offset}");

                pts += offset;
            }
        }

        debug!("{ssrc_val:#08x} calculated PTS {pts}");

        (pts, ntp_time)
    }
}

const WINDOW_LENGTH: u64 = 512;
const WINDOW_DURATION: u64 = 2_000_000_000;

#[derive(Debug)]
struct Observations {
    base_local_time: Option<u64>,
    base_remote_time: Option<u64>,
    highest_remote_time: Option<u64>,
    deltas: VecDeque<i64>,
    min_delta: i64,
    skew: i64,
    filling: bool,
    window_size: usize,
}

impl Default for Observations {
    fn default() -> Self {
        Self {
            base_local_time: None,
            base_remote_time: None,
            highest_remote_time: None,
            deltas: VecDeque::new(),
            min_delta: 0,
            skew: 0,
            filling: true,
            window_size: 0,
        }
    }
}

impl Observations {
    fn out_time(&self, base_local_time: u64, remote_diff: u64) -> (u64, bool) {
        let out_time = base_local_time + remote_diff;
        let out_time = if self.skew < 0 {
            out_time.saturating_sub((-self.skew) as u64)
        } else {
            out_time + (self.skew as u64)
        };

        trace!("Skew {}, min delta {}", self.skew, self.min_delta);
        trace!("Outputting {out_time}");

        (out_time, false)
    }

    // Based on the algorithm used in GStreamer's rtpjitterbuffer, which comes from
    // Fober, Orlarey and Letz, 2005, "Real Time Clock Skew Estimation over Network Delays":
    // http://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.102.1546
    fn process(&mut self, remote_time: u64, local_time: u64) -> (u64, bool) {
        trace!("Local time {local_time}, remote time {remote_time}",);

        let (base_remote_time, base_local_time) =
            match (self.base_remote_time, self.base_local_time) {
                (Some(remote), Some(local)) => (remote, local),
                _ => {
                    debug!("Initializing base time: local {local_time}, remote {remote_time}",);
                    self.base_remote_time = Some(remote_time);
                    self.base_local_time = Some(local_time);
                    self.highest_remote_time = Some(remote_time);

                    return (local_time, false);
                }
            };

        let highest_remote_time = self.highest_remote_time.unwrap();

        let remote_diff = remote_time.saturating_sub(base_remote_time);

        /* Only update observations when remote times progress forward */
        if remote_time <= highest_remote_time {
            return self.out_time(base_local_time, remote_diff);
        }

        self.highest_remote_time = Some(remote_time);

        let local_diff = local_time.saturating_sub(base_local_time);
        let delta = (local_diff as i64) - (remote_diff as i64);

        trace!("Local diff {local_diff}, remote diff {remote_diff}, delta {delta}",);

        if remote_diff > 0 && local_diff > 0 {
            let slope = (local_diff as f64) / (remote_diff as f64);
            if !(0.8..1.2).contains(&slope) {
                warn!("Too small/big slope {slope}, resetting");

                let discont = !self.deltas.is_empty();
                *self = Observations::default();

                debug!("Initializing base time: local {local_time}, remote {remote_time}",);
                self.base_remote_time = Some(remote_time);
                self.base_local_time = Some(local_time);
                self.highest_remote_time = Some(remote_time);

                return (local_time, discont);
            }
        }

        if (delta > self.skew && delta - self.skew > 1_000_000_000)
            || (delta < self.skew && self.skew - delta > 1_000_000_000)
        {
            warn!("Delta {} too far from skew {}, resetting", delta, self.skew);

            let discont = !self.deltas.is_empty();
            *self = Observations::default();

            debug!("Initializing base time: local {local_time}, remote {remote_time}",);
            self.base_remote_time = Some(remote_time);
            self.base_local_time = Some(local_time);
            self.highest_remote_time = Some(remote_time);

            return (local_time, discont);
        }

        if self.filling {
            if self.deltas.is_empty() || delta < self.min_delta {
                self.min_delta = delta;
            }
            self.deltas.push_back(delta);

            if remote_diff > WINDOW_DURATION || self.deltas.len() as u64 == WINDOW_LENGTH {
                self.window_size = self.deltas.len();
                self.skew = self.min_delta;
                self.filling = false;
            } else {
                let perc_time = remote_diff.mul_div_floor(100, WINDOW_DURATION).unwrap() as i64;
                let perc_window = (self.deltas.len() as u64)
                    .mul_div_floor(100, WINDOW_LENGTH)
                    .unwrap() as i64;
                let perc = std::cmp::max(perc_time, perc_window);

                self.skew = (perc * self.min_delta + ((10_000 - perc) * self.skew)) / 10_000;
            }
        } else {
            let old = self.deltas.pop_front().unwrap();
            self.deltas.push_back(delta);

            if delta <= self.min_delta {
                self.min_delta = delta;
            } else if old == self.min_delta {
                self.min_delta = self.deltas.iter().copied().min().unwrap();
            }

            self.skew = (self.min_delta + (124 * self.skew)) / 125;
        }

        self.out_time(base_local_time, remote_diff)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rtpbin2::session::tests::init_logs;
    use crate::rtpbin2::time::system_time_to_ntp_time_u64;

    #[test]
    fn test_single_stream_no_sr() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345678, 90000);

        assert_eq!(ctx.calculate_pts(0x12345678, 0, now), (0, None));
        now += 1_000_000_000;
        assert_eq!(
            ctx.calculate_pts(0x12345678, 90000, now),
            (1_000_000_000, None)
        );
    }

    #[test]
    fn test_single_stream_with_sr() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345678, 90000);

        ctx.add_sender_report(
            0x12345678,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        assert_eq!(
            ctx.calculate_pts(0x12345678, 0, now),
            (0, Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH)))
        );
        now += 1_000_000_000;
        assert_eq!(
            ctx.calculate_pts(0x12345678, 90000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(1000)
                ))
            )
        );
    }

    #[test]
    fn test_two_streams_with_sr() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345, 90000);
        ctx.set_clock_rate(0x67890, 90000);
        ctx.associate(0x12345, "foo@bar");
        ctx.associate(0x67890, "foo@bar");

        ctx.add_sender_report(
            0x12345,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        ctx.add_sender_report(
            0x67890,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH + Duration::from_millis(500))
                .as_u64(),
        );

        // NTP time 0
        assert_eq!(
            ctx.calculate_pts(0x12345, 0, now),
            (0, Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH)))
        );
        now += 500_000_000;

        // NTP time 500, arrival time 500
        assert_eq!(
            ctx.calculate_pts(0x12345, 45000, now),
            (
                500_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );
        // NTP time 500, arrival time 500
        assert_eq!(
            ctx.calculate_pts(0x67890, 0, now),
            (
                500_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );
        now += 500_000_000;
        // NTP time 1000, arrival time 1000
        assert_eq!(
            ctx.calculate_pts(0x12345, 90000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(1000)
                ))
            )
        );
        now += 500_000_000;
        // NTP time 1500, arrival time 1500
        assert_eq!(
            ctx.calculate_pts(0x67890, 90000, now),
            (
                1_500_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(1500)
                ))
            )
        );
    }

    #[test]
    fn test_two_streams_no_sr_and_offset_arrival_times() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345, 90000);
        ctx.set_clock_rate(0x67890, 90000);
        ctx.associate(0x12345, "foo@bar");
        ctx.associate(0x67890, "foo@bar");

        assert_eq!(ctx.calculate_pts(0x12345, 0, now), (0, None));

        now += 500_000_000;

        assert_eq!(ctx.calculate_pts(0x67890, 0, now), (500_000_000, None));
        assert_eq!(ctx.calculate_pts(0x12345, 45000, now), (500_000_000, None));
    }

    #[test]
    fn test_two_streams_with_same_sr_and_offset_arrival_times() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345, 90000);
        ctx.set_clock_rate(0x67890, 90000);
        ctx.associate(0x12345, "foo@bar");
        ctx.associate(0x67890, "foo@bar");

        ctx.add_sender_report(
            0x12345,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        ctx.add_sender_report(
            0x67890,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        assert_eq!(
            ctx.calculate_pts(0x12345, 0, now),
            (0, Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH)))
        );

        now += 500_000_000;

        assert_eq!(
            ctx.calculate_pts(0x67890, 0, now),
            (
                500_000_000,
                Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH))
            )
        );

        assert_eq!(
            ctx.calculate_pts(0x12345, 45000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );

        now += 500_000_000;

        assert_eq!(
            ctx.calculate_pts(0x67890, 45000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );

        // Now remove the delayed source and observe that the offset is gone
        // for the other source

        ctx.remove_ssrc(0x67890);

        assert_eq!(
            ctx.calculate_pts(0x12345, 90000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(1000)
                ))
            )
        );
    }

    #[test]
    fn test_two_streams_with_sr_different_cnames() {
        init_logs();

        let mut ctx = Context::new(TimestampingMode::Rtp);

        let mut now = 0;

        ctx.set_clock_rate(0x12345, 90000);
        ctx.set_clock_rate(0x67890, 90000);
        ctx.associate(0x12345, "foo@bar");
        ctx.associate(0x67890, "foo@baz");

        ctx.add_sender_report(
            0x12345,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        ctx.add_sender_report(
            0x67890,
            0,
            system_time_to_ntp_time_u64(std::time::UNIX_EPOCH).as_u64(),
        );

        assert_eq!(
            ctx.calculate_pts(0x12345, 0, now),
            (0, Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH)))
        );

        now += 500_000_000;

        assert_eq!(
            ctx.calculate_pts(0x67890, 0, now),
            (
                500_000_000,
                Some(system_time_to_ntp_time_u64(std::time::UNIX_EPOCH))
            )
        );

        assert_eq!(
            ctx.calculate_pts(0x12345, 45000, now),
            (
                500_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );

        now += 500_000_000;

        assert_eq!(
            ctx.calculate_pts(0x67890, 45000, now),
            (
                1_000_000_000,
                Some(system_time_to_ntp_time_u64(
                    std::time::UNIX_EPOCH + Duration::from_millis(500)
                ))
            )
        );
    }
}
