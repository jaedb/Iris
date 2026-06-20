use std::fmt::{self, Write};
use std::time::{Duration, Instant};

#[cfg(feature = "tuning")]
use gstthreadshare::runtime::Context;

use super::CAT;

const LOG_PERIOD: Duration = Duration::from_secs(20);

#[derive(Debug, Default)]
pub struct Stats {
    ramp_up_instant: Option<Instant>,
    log_start_instant: Option<Instant>,
    last_delta_instant: Option<Instant>,
    buffer_count: f32,
    buffer_count_delta: f32,
    lateness_sum: f32,
    lateness_square_sum: f32,
    lateness_sum_delta: f32,
    lateness_square_sum_delta: f32,
    lateness_min: i64,
    lateness_min_delta: i64,
    lateness_max: i64,
    lateness_max_delta: i64,
    interval_sum: f32,
    interval_square_sum: f32,
    interval_sum_delta: f32,
    interval_square_sum_delta: f32,
    interval_min: i64,
    interval_min_delta: i64,
    interval_max: i64,
    interval_max_delta: i64,
    interval_late_warn: i64,
    interval_late_count: f32,
    interval_late_count_delta: f32,
    #[cfg(feature = "tuning")]
    parked_duration_init: Duration,
}

impl Stats {
    pub fn new(interval_late_warn: Duration) -> Self {
        Stats {
            interval_late_warn: interval_late_warn.as_nanos() as i64,
            ..Default::default()
        }
    }

    pub fn start(&mut self) {
        self.buffer_count = 0.0;
        self.buffer_count_delta = 0.0;
        self.lateness_sum = 0.0;
        self.lateness_square_sum = 0.0;
        self.lateness_sum_delta = 0.0;
        self.lateness_square_sum_delta = 0.0;
        self.lateness_min = i64::MAX;
        self.lateness_min_delta = i64::MAX;
        self.lateness_max = i64::MIN;
        self.lateness_max_delta = i64::MIN;
        self.interval_sum = 0.0;
        self.interval_square_sum = 0.0;
        self.interval_sum_delta = 0.0;
        self.interval_square_sum_delta = 0.0;
        self.interval_min = i64::MAX;
        self.interval_min_delta = i64::MAX;
        self.interval_max = i64::MIN;
        self.interval_max_delta = i64::MIN;
        self.interval_late_count = 0.0;
        self.interval_late_count_delta = 0.0;
        self.last_delta_instant = None;
        self.log_start_instant = None;

        self.ramp_up_instant = Some(Instant::now());
        gst::info!(CAT, "First stats logs in {:2?}", 2 * LOG_PERIOD);
    }

    pub fn is_active(&mut self) -> bool {
        if let Some(ramp_up_instant) = self.ramp_up_instant {
            if ramp_up_instant.elapsed() < LOG_PERIOD {
                return false;
            }

            self.ramp_up_instant = None;
            gst::info!(CAT, "Ramp up complete. Stats logs in {:2?}", LOG_PERIOD);
            self.log_start_instant = Some(Instant::now());
            self.last_delta_instant = self.log_start_instant;

            #[cfg(feature = "tuning")]
            {
                self.parked_duration_init = Context::current().unwrap().parked_duration();
            }
        }

        true
    }

    pub fn add_buffer(&mut self, lateness: i64, interval: i64) {
        if !self.is_active() {
            return;
        }

        self.buffer_count += 1.0;
        self.buffer_count_delta += 1.0;

        // Lateness
        let lateness_f32 = lateness as f32;
        let lateness_square = lateness_f32.powi(2);

        self.lateness_sum += lateness_f32;
        self.lateness_square_sum += lateness_square;
        self.lateness_min = self.lateness_min.min(lateness);
        self.lateness_max = self.lateness_max.max(lateness);

        self.lateness_sum_delta += lateness_f32;
        self.lateness_square_sum_delta += lateness_square;
        self.lateness_min_delta = self.lateness_min_delta.min(lateness);
        self.lateness_max_delta = self.lateness_max_delta.max(lateness);

        // Interval
        let interval_f32 = interval as f32;
        let interval_square = interval_f32.powi(2);

        self.interval_sum += interval_f32;
        self.interval_square_sum += interval_square;
        self.interval_min = self.interval_min.min(interval);
        self.interval_max = self.interval_max.max(interval);

        self.interval_sum_delta += interval_f32;
        self.interval_square_sum_delta += interval_square;
        self.interval_min_delta = self.interval_min_delta.min(interval);
        self.interval_max_delta = self.interval_max_delta.max(interval);

        if interval > self.interval_late_warn {
            self.interval_late_count += 1.0;
            self.interval_late_count_delta += 1.0;
        }

        let delta_duration = match self.last_delta_instant {
            Some(last_delta) => last_delta.elapsed(),
            None => return,
        };

        if delta_duration < LOG_PERIOD {
            return;
        }

        self.last_delta_instant = Some(Instant::now());

        gst::info!(CAT, "Delta stats:");
        let interval_mean = self.interval_sum_delta / self.buffer_count_delta;
        let interval_std_dev = f32::sqrt(
            self.interval_square_sum_delta / self.buffer_count_delta - interval_mean.powi(2),
        );

        gst::info!(
            CAT,
            "* interval: mean {:4.2?} σ {:4.1?} [{:4.1?}, {:4.1?}]",
            SignedDuration(interval_mean as i64),
            Duration::from_nanos(interval_std_dev as u64),
            SignedDuration(self.interval_min_delta),
            SignedDuration(self.interval_max_delta),
        );

        if self.interval_late_count_delta > f32::EPSILON {
            gst::warning!(
                CAT,
                "* {:5.2}% late buffers",
                100f32 * self.interval_late_count_delta / self.buffer_count_delta
            );
        }

        self.interval_sum_delta = 0.0;
        self.interval_square_sum_delta = 0.0;
        self.interval_min_delta = i64::MAX;
        self.interval_max_delta = i64::MIN;
        self.interval_late_count_delta = 0.0;

        let lateness_mean = self.lateness_sum_delta / self.buffer_count_delta;
        let lateness_std_dev = f32::sqrt(
            self.lateness_square_sum_delta / self.buffer_count_delta - lateness_mean.powi(2),
        );

        gst::info!(
            CAT,
            "* lateness: mean {:4.2?} σ {:4.1?} [{:4.1?}, {:4.1?}]",
            SignedDuration(lateness_mean as i64),
            Duration::from_nanos(lateness_std_dev as u64),
            SignedDuration(self.lateness_min_delta),
            SignedDuration(self.lateness_max_delta),
        );

        self.lateness_sum_delta = 0.0;
        self.lateness_square_sum_delta = 0.0;
        self.lateness_min_delta = i64::MAX;
        self.lateness_max_delta = i64::MIN;

        self.buffer_count_delta = 0.0;
    }

    pub fn log_global(&mut self) {
        if self.buffer_count < 1.0 {
            return;
        }

        let _log_start = if let Some(log_start) = self.log_start_instant {
            log_start
        } else {
            return;
        };

        gst::info!(CAT, "Global stats:");

        #[cfg(feature = "tuning")]
        {
            let duration = _log_start.elapsed();
            let parked_duration =
                Context::current().unwrap().parked_duration() - self.parked_duration_init;
            gst::info!(
                CAT,
                "* parked: {parked_duration:4.2?} ({:5.2?}%)",
                (parked_duration.as_nanos() as f32 * 100.0 / duration.as_nanos() as f32)
            );
        }

        let interval_mean = self.interval_sum / self.buffer_count;
        let interval_std_dev =
            f32::sqrt(self.interval_square_sum / self.buffer_count - interval_mean.powi(2));

        gst::info!(
            CAT,
            "* interval: mean {:4.2?} σ {:4.1?} [{:4.1?}, {:4.1?}]",
            SignedDuration(interval_mean as i64),
            Duration::from_nanos(interval_std_dev as u64),
            SignedDuration(self.interval_min),
            SignedDuration(self.interval_max),
        );

        if self.interval_late_count > f32::EPSILON {
            gst::warning!(
                CAT,
                "* {:5.2}% late buffers",
                100f32 * self.interval_late_count / self.buffer_count
            );
        }

        let lateness_mean = self.lateness_sum / self.buffer_count;
        let lateness_std_dev =
            f32::sqrt(self.lateness_square_sum / self.buffer_count - lateness_mean.powi(2));

        gst::info!(
            CAT,
            "* lateness: mean {:4.2?} σ {:4.1?} [{:4.1?}, {:4.1?}]",
            SignedDuration(lateness_mean as i64),
            Duration::from_nanos(lateness_std_dev as u64),
            SignedDuration(self.lateness_min),
            SignedDuration(self.lateness_max),
        );
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SignedDuration(i64);

impl fmt::Debug for SignedDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_negative() {
            f.write_char('-')?;
            Duration::from_nanos(i64::abs(self.0) as u64).fmt(f)
        } else {
            Duration::from_nanos(self.0 as u64).fmt(f)
        }
    }
}
