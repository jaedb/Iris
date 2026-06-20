// SPDX-License-Identifier: MPL-2.0

use std::{
    ops::{Add, Sub},
    time::{Duration, SystemTime},
};

// time between the NTP time at 1900-01-01 and the unix EPOCH (1970-01-01)
const NTP_OFFSET: Duration = Duration::from_secs((365 * 70 + 17) * 24 * 60 * 60);

// 2^32
const F32: f64 = 4_294_967_296.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NtpTime(u64);

impl NtpTime {
    pub fn from_duration(dur: Duration) -> Self {
        Self((dur.as_secs_f64() * F32) as u64)
    }

    pub fn as_duration(&self) -> Result<Duration, std::time::TryFromFloatSecsError> {
        Duration::try_from_secs_f64(self.0 as f64 / F32)
    }

    pub fn as_u32(self) -> u32 {
        ((self.0 >> 16) & 0xffffffff) as u32
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Sub for NtpTime {
    type Output = NtpTime;
    fn sub(self, rhs: Self) -> Self::Output {
        NtpTime(self.0 - rhs.0)
    }
}

impl Add for NtpTime {
    type Output = NtpTime;
    fn add(self, rhs: Self) -> Self::Output {
        NtpTime(self.0 + rhs.0)
    }
}

pub fn system_time_to_ntp_time_u64(time: SystemTime) -> NtpTime {
    let dur = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time is before unix epoch?!")
        + NTP_OFFSET;

    NtpTime::from_duration(dur)
}

impl From<u64> for NtpTime {
    fn from(value: u64) -> Self {
        NtpTime(value)
    }
}
