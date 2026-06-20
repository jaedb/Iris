/// Computes the seqnum distance
///
/// This makes sense if both seqnums are in the same cycle.
pub fn seqnum_distance(seqnum1: u16, seqnum2: u16) -> i16 {
    // See http://en.wikipedia.org/wiki/Serial_number_arithmetic

    let seqnum1 = i16::from_ne_bytes(seqnum1.to_ne_bytes());
    let seqnum2 = i16::from_ne_bytes(seqnum2.to_ne_bytes());

    seqnum1.wrapping_sub(seqnum2)
}

/// Defines a comparable new type `$typ` on a `[std::num::Wrapping]::<u32>`.
///
/// The new type will wrap-around on additions and substractions and it comparison
/// operators take the wrapping in consideration.
///
/// The comparison algorithm uses [serial number arithmetic](serial-number-arithmetic).
/// The limit being that it can't tell whether 0x8000_0000 is greater or less than 0.
///
/// # Examples
///
/// ```rust
/// # use gstrsrtp::define_wrapping_comparable_u32;
///
/// /// Error type to return when comparing 0x8000_0000 to 0.
/// struct RTPTimestampComparisonLimit;
///
/// /// Define the new type comparable and wrapping `u32` `RTPTimestamp`:
/// define_wrapping_comparable_u32!(RTPTimestamp, RTPTimestampComparisonLimit);
///
/// let ts0 = RTPTimestamp::ZERO;
/// assert!(ts0.is_zero());
///
/// let mut ts = ts0;
/// ts += 1;
/// assert_eq!(*ts, 1);
/// assert_eq!(RTPTimestamp::MAX + ts, ts0);
///
/// let ts2: RTPTimestamp = 2.into();
/// assert_eq!(*ts2, 2);
/// assert_eq!(ts - ts2, RTPTimestamp::MAX);
/// ```
///
/// [serial-number-arithmetic]: http://en.wikipedia.org/wiki/Serial_number_arithmetic
#[macro_export]
macro_rules! define_wrapping_comparable_u32 {
    ($typ:ident) => {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $typ(std::num::Wrapping<u32>);

        impl $typ {
            pub const ZERO: $typ = $typ(std::num::Wrapping(0));
            pub const MIN: $typ = $typ(std::num::Wrapping(u32::MIN));
            pub const MAX: $typ = $typ(std::num::Wrapping(u32::MAX));
            pub const NONE: Option<$typ> = None;

            #[inline]
            pub const fn new(val: u32) -> Self {
                Self(std::num::Wrapping((val)))
            }

            #[inline]
            pub fn from_ext(ext_val: u64) -> Self {
                Self(std::num::Wrapping((ext_val & 0xffff_ffff) as u32))
            }

            #[inline]
            pub fn is_zero(self) -> bool {
                self.0 .0 == 0
            }

            #[inline]
            pub fn distance(self, other: Self) -> Option<i32> {
                self.distance_u32(other.0 .0)
            }

            #[inline]
            pub fn distance_u32(self, other: u32) -> Option<i32> {
                // See http://en.wikipedia.org/wiki/Serial_number_arithmetic

                let this = i32::from_ne_bytes(self.0 .0.to_ne_bytes());
                let other = i32::from_ne_bytes(other.to_ne_bytes());

                match this.wrapping_sub(other) {
                    -0x8000_0000 => {
                        // This is the limit of the algorithm:
                        // arguments are too far away to determine the result sign,
                        // i.e. which one is greater than the other
                        None
                    }
                    delta => Some(delta),
                }
            }
        }

        impl From<u32> for $typ {
            fn from(value: u32) -> Self {
                Self(std::num::Wrapping(value))
            }
        }

        impl From<$typ> for u32 {
            fn from(value: $typ) -> Self {
                value.0 .0
            }
        }

        impl std::ops::Deref for $typ {
            type Target = u32;

            fn deref(&self) -> &u32 {
                &self.0 .0
            }
        }

        impl std::ops::Add for $typ {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0.add(rhs.0))
            }
        }

        impl std::ops::Add<u32> for $typ {
            type Output = Self;
            fn add(self, rhs: u32) -> Self {
                Self(self.0.add(std::num::Wrapping(rhs)))
            }
        }

        impl std::ops::Add<i32> for $typ {
            type Output = Self;
            fn add(self, rhs: i32) -> Self {
                // See http://en.wikipedia.org/wiki/Serial_number_arithmetic

                let this = i32::from_ne_bytes(self.0 .0.to_ne_bytes());
                let res = this.wrapping_add(rhs);

                let res = u32::from_ne_bytes(res.to_ne_bytes());
                Self(std::num::Wrapping(res))
            }
        }

        impl std::ops::AddAssign for $typ {
            fn add_assign(&mut self, rhs: Self) {
                self.0.add_assign(rhs.0);
            }
        }

        impl std::ops::AddAssign<u32> for $typ {
            fn add_assign(&mut self, rhs: u32) {
                self.0.add_assign(std::num::Wrapping(rhs));
            }
        }

        impl std::ops::AddAssign<i32> for $typ {
            fn add_assign(&mut self, rhs: i32) {
                *self = *self + rhs;
            }
        }

        impl std::ops::Sub for $typ {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                self.sub(rhs.0 .0)
            }
        }

        impl std::ops::Sub<u32> for $typ {
            type Output = Self;
            fn sub(self, rhs: u32) -> Self {
                Self(self.0.sub(std::num::Wrapping(rhs)))
            }
        }

        impl std::ops::SubAssign for $typ {
            fn sub_assign(&mut self, rhs: Self) {
                self.sub_assign(rhs.0 .0);
            }
        }

        impl std::ops::SubAssign<u32> for $typ {
            fn sub_assign(&mut self, rhs: u32) {
                self.0.sub_assign(std::num::Wrapping(rhs));
            }
        }

        impl std::cmp::PartialEq for $typ {
            fn eq(&self, other: &Self) -> bool {
                self.0 .0 == other.0 .0
            }
        }

        impl std::cmp::PartialEq<u32> for $typ {
            fn eq(&self, other: &u32) -> bool {
                self.0 .0 == *other
            }
        }

        impl std::cmp::Eq for $typ {}

        impl std::cmp::PartialOrd for $typ {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.distance(*other).map(|d| d.cmp(&0))
            }
        }

        impl gst::prelude::OptionOperations for $typ {}
    };

    ($typ:ident, $comp_err_type:ident) => {
        define_wrapping_comparable_u32!($typ);

        impl $typ {
            #[inline]
            pub fn try_cmp(&self, other: $typ) -> Result<std::cmp::Ordering, $comp_err_type> {
                self.partial_cmp(&other).ok_or($comp_err_type)
            }
        }
    };

    ($typ:ident, $err_enum:ty, $comp_err_variant:ident) => {
        define_wrapping_comparable_u32!($typ);

        impl $typ {
            #[inline]
            pub fn try_cmp(&self, other: $typ) -> Result<std::cmp::Ordering, $err_enum> {
                self.partial_cmp(&other)
                    .ok_or(<$err_enum>::$comp_err_variant)
            }
        }
    };
}

#[macro_export]
macro_rules! define_wrapping_comparable_u32_with_display {
    ($typ:ident, impl) => {
        impl std::fmt::Display for $typ {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_fmt(format_args!("{}", self.0 .0))
            }
        }
    };

    ($typ:ident) => {
        define_wrapping_comparable_u32!($typ);
        define_wrapping_comparable_u32_with_display!($typ, impl);
    };

    ($typ:ident, $comp_err_type:ty) => {
        define_wrapping_comparable_u32!($typ, $comp_err_type);
        define_wrapping_comparable_u32_with_display!($typ, impl);
    };

    ($typ:ident, $err_enum:ty, $comp_err_variant:ident,) => {
        define_wrapping_comparable_u32!($typ, $err_enum, $comp_err_variant);
        define_wrapping_comparable_u32_with_display!($typ, impl);
    };
}

/// Stores information necessary to compute a series of extended timestamps
#[derive(Default, Debug)]
pub(crate) struct ExtendedTimestamp {
    last_ext: Option<u64>,
}

impl ExtendedTimestamp {
    /// Produces the next extended timestamp from a new RTP timestamp
    pub(crate) fn next(&mut self, rtp_timestamp: u32) -> u64 {
        let ext = match self.last_ext {
            None => (1u64 << 32) + rtp_timestamp as u64,
            Some(last_ext) => {
                // pick wraparound counter from previous timestamp and add to new timestamp
                let mut ext = rtp_timestamp as u64 + (last_ext & !0xffffffff);

                // check for timestamp wraparound
                if ext < last_ext {
                    let diff = last_ext - ext;

                    if diff > i32::MAX as u64 {
                        // timestamp went backwards more than allowed, we wrap around and get
                        // updated extended timestamp.
                        ext += 1u64 << 32;
                    }
                } else {
                    let diff = ext - last_ext;

                    if diff > i32::MAX as u64 {
                        if ext < 1u64 << 32 {
                            // We can't ever get to such a case as our counter is opaque
                            unreachable!()
                        } else {
                            ext -= 1u64 << 32;
                            // We don't want the extended timestamp storage to go back, ever
                            return ext;
                        }
                    }
                }

                ext
            }
        };

        self.last_ext = Some(ext);

        ext
    }
}

/// Stores information necessary to compute a series of extended seqnums
#[derive(Default, Debug)]
pub(crate) struct ExtendedSeqnum {
    last_ext: Option<u64>,
}

impl ExtendedSeqnum {
    /// The current extended sequence number
    pub(crate) fn current(&self) -> Option<u64> {
        self.last_ext
    }

    /// Produces the next extended sequence number from a new RTP sequence number
    pub(crate) fn next(&mut self, rtp_seqnum: u16) -> u64 {
        let ext = match self.last_ext {
            None => (1u64 << 16) + rtp_seqnum as u64,
            Some(last_ext) => {
                // pick wraparound counter from previous timestamp and add to new timestamp
                let mut ext = rtp_seqnum as u64 + (last_ext & !0xffff);

                // check for timestamp wraparound
                if ext < last_ext {
                    let diff = last_ext - ext;

                    if diff > i16::MAX as u64 {
                        // timestamp went backwards more than allowed, we wrap around and get
                        // updated extended timestamp.
                        ext += 1u64 << 16;
                    }
                } else {
                    let diff = ext - last_ext;

                    if diff > i16::MAX as u64 {
                        if ext < 1u64 << 16 {
                            // We can't ever get to such a case as our counter is opaque
                            unreachable!()
                        } else {
                            ext -= 1u64 << 16;
                            // We don't want the extended timestamp storage to go back, ever
                            return ext;
                        }
                    }
                }

                ext
            }
        };

        self.last_ext = Some(ext);

        ext
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    define_wrapping_comparable_u32!(MyWrapper);

    #[test]
    fn compare_seqnums() {
        assert_eq!(seqnum_distance(0, 1), -1);
        assert_eq!(seqnum_distance(1, 1), 0);
        assert_eq!(seqnum_distance(1, 0), 1);

        assert_eq!(seqnum_distance(0x7fff, 0), 0x7fff);
        assert_eq!(seqnum_distance(0xffff, 0), -1);

        assert_eq!(seqnum_distance(0, 0x7fff), -0x7fff);
        assert_eq!(seqnum_distance(0, 0xffff), 1);

        // This is the limit of the algorithm:
        assert_eq!(seqnum_distance(0x8000, 0), -0x8000);
        assert_eq!(seqnum_distance(0, 0x8000), -0x8000);
    }

    #[test]
    fn wrapping_u32_basics() {
        let zero = MyWrapper::ZERO;
        let one = MyWrapper::from(1);
        let two = MyWrapper::from(2);

        assert_eq!(u32::from(zero), 0);
        assert!(zero.is_zero());
        assert_eq!(u32::from(one), 1);
        assert_eq!(u32::from(two), 2);

        let max_plus_1_u64 = MyWrapper::from_ext((u32::MAX as u64) + 1);
        assert_eq!(max_plus_1_u64, MyWrapper::ZERO);
    }

    #[test]
    fn add_wrapping_u32() {
        let one = MyWrapper::from(1);
        let two = MyWrapper::from(2);

        assert_eq!(MyWrapper::ZERO + one, one);
        assert_eq!(MyWrapper::ZERO + 1u32, one);
        assert_eq!(one + one, two);
        assert_eq!(one + 1u32, two);

        assert_eq!(MyWrapper::MAX + MyWrapper::ZERO, MyWrapper::MAX);
        assert_eq!(MyWrapper::MAX + one, MyWrapper::ZERO);
        assert_eq!(MyWrapper::MAX + two, one);

        let mut var = MyWrapper::ZERO;
        assert!(var.is_zero());
        var += 1;
        assert_eq!(var, one);
        var += one;
        assert_eq!(var, two);

        let mut var = MyWrapper::MAX;
        var += 1;
        assert!(var.is_zero());
        var += one;
        assert_eq!(var, one);
    }

    #[test]
    fn add_wrapping_u32_i32() {
        let one = MyWrapper::from(1);

        assert_eq!(MyWrapper::ZERO + 1i32, one);
        assert_eq!(MyWrapper::ZERO + -1i32, MyWrapper::MAX);
        assert_eq!(MyWrapper::MAX + 1i32, MyWrapper::ZERO);
        assert_eq!(MyWrapper::MAX + 2i32, one);

        assert_eq!(
            MyWrapper::from(0x8000_0000) + -0i32,
            MyWrapper::from(0x8000_0000)
        );
        assert_eq!(
            MyWrapper::from(0x8000_0000) + 1i32,
            MyWrapper::from(0x8000_0001)
        );
        assert_eq!(
            MyWrapper::from(0x8000_0000) + -1i32,
            MyWrapper::from(0x7fff_ffff)
        );
        assert_eq!(
            MyWrapper::from(0x7fff_ffff) + 1i32,
            MyWrapper::from(0x8000_0000)
        );
        assert_eq!(MyWrapper::ZERO + i32::MIN, MyWrapper::from(0x8000_0000));

        let mut var = MyWrapper::ZERO;
        var += 1i32;
        assert_eq!(var, one);

        let mut var = MyWrapper::ZERO;
        var += -1i32;
        assert_eq!(var, MyWrapper::MAX);

        let mut var = MyWrapper::MAX;
        var += 1;
        assert_eq!(var, MyWrapper::ZERO);
    }

    #[test]
    fn sub_wrapping_u32() {
        let one = MyWrapper::from(1);

        assert_eq!(MyWrapper::ZERO - MyWrapper::ZERO, MyWrapper::ZERO);
        assert_eq!(MyWrapper::MAX - MyWrapper::MAX, MyWrapper::ZERO);
        assert_eq!(MyWrapper::ZERO - one, MyWrapper::MAX);
        assert_eq!(MyWrapper::ZERO - MyWrapper::MAX, one);
        assert_eq!(
            MyWrapper::ZERO - MyWrapper::from(0x8000_0000),
            MyWrapper::from(0x8000_0000)
        );
        assert_eq!(
            MyWrapper::from(0x8000_0000) - MyWrapper::ZERO,
            MyWrapper::from(0x8000_0000)
        );

        let mut var = MyWrapper::ZERO;
        assert!(var.is_zero());
        var -= 1;
        assert_eq!(var, MyWrapper::MAX);

        let mut var = MyWrapper::MAX;
        var -= MyWrapper::MAX;
        assert!(var.is_zero());
    }

    #[test]
    fn compare_wrapping_u32() {
        use std::cmp::Ordering::*;

        #[derive(Debug, PartialEq)]
        pub struct ComparisonLimit;
        define_wrapping_comparable_u32!(MyWrapper, ComparisonLimit);

        let cmp = |a: u32, b: u32| MyWrapper::from(a).partial_cmp(&MyWrapper::from(b));
        let try_cmp = |a: u32, b: u32| MyWrapper::from(a).try_cmp(MyWrapper::from(b));

        assert_eq!(cmp(0, 1).unwrap(), Less);
        assert_eq!(try_cmp(0, 1), Ok(Less));
        assert_eq!(cmp(1, 1).unwrap(), Equal);
        assert_eq!(try_cmp(1, 1), Ok(Equal));
        assert_eq!(cmp(1, 0).unwrap(), Greater);
        assert_eq!(try_cmp(1, 0), Ok(Greater));

        assert_eq!(cmp(0x7fff_ffff, 0).unwrap(), Greater);
        assert_eq!(try_cmp(0x7fff_ffff, 0), Ok(Greater));
        assert_eq!(cmp(0xffff_ffff, 0).unwrap(), Less);
        assert_eq!(try_cmp(0xffff_ffff, 0), Ok(Less));

        assert_eq!(cmp(0, 0x7fff_ffff).unwrap(), Less);
        assert_eq!(try_cmp(0, 0x7fff_ffff), Ok(Less));
        assert_eq!(cmp(0, 0xffff_ffff).unwrap(), Greater);
        assert_eq!(try_cmp(0, 0xffff_ffff), Ok(Greater));

        // This is the limit of the algorithm:
        assert!(cmp(0x8000_0000, 0).is_none());
        assert!(cmp(0, 0x8000_0000).is_none());
        assert_eq!(try_cmp(0x8000_0000, 0), Err(ComparisonLimit));
        assert_eq!(try_cmp(0, 0x8000_0000), Err(ComparisonLimit));
    }

    #[test]
    fn extended_timestamp_basic() {
        let mut ext_ts = ExtendedTimestamp::default();

        // No wraparound when timestamps are increasing
        assert_eq!(ext_ts.next(0), (1 << 32));
        assert_eq!(ext_ts.next(10), (1 << 32) + 10);
        assert_eq!(ext_ts.next(10), (1 << 32) + 10);
        assert_eq!(
            ext_ts.next(1 + i32::MAX as u32),
            (1 << 32) + 1 + i32::MAX as u64
        );

        // Even big bumps under G_MAXINT32 don't result in wrap-around
        ext_ts = ExtendedTimestamp::default();

        assert_eq!(ext_ts.next(1087500), (1 << 32) + 1087500);
        assert_eq!(ext_ts.next(24), (1 << 32) + 24);
    }

    #[test]
    fn extended_timestamp_wraparound() {
        let mut ext_ts = ExtendedTimestamp::default();
        assert_eq!(
            ext_ts.next(u32::MAX - 90000 + 1),
            (1 << 32) + u32::MAX as u64 - 90000 + 1
        );
        assert_eq!(ext_ts.next(0), (1 << 32) + u32::MAX as u64 + 1);
        assert_eq!(ext_ts.next(90000), (1 << 32) + u32::MAX as u64 + 1 + 90000);
    }

    #[test]
    fn extended_timestamp_wraparound_disordered() {
        let mut ext_ts = ExtendedTimestamp::default();

        assert_eq!(
            ext_ts.next(u32::MAX - 90000 + 1),
            (1 << 32) + u32::MAX as u64 - 90000 + 1
        );
        assert_eq!(ext_ts.next(0), (1 << 32) + u32::MAX as u64 + 1);

        // Unwrapping around
        assert_eq!(
            ext_ts.next(u32::MAX - 90000 + 1),
            (1 << 32) + u32::MAX as u64 - 90000 + 1
        );
        assert_eq!(ext_ts.next(90000), (1 << 32) + u32::MAX as u64 + 1 + 90000);
    }

    #[test]
    fn extended_timestamp_wraparound_disordered_backwards() {
        let mut ext_ts = ExtendedTimestamp::default();

        assert_eq!(ext_ts.next(90000), (1 << 32) + 90000);

        // Wraps backwards
        assert_eq!(
            ext_ts.next(u32::MAX - 90000 + 1),
            u32::MAX as u64 - 90000 + 1
        );

        // Wraps again forwards
        assert_eq!(ext_ts.next(90000), (1 << 32) + 90000);
    }

    #[test]
    fn extended_seqnum_basic() {
        let mut ext_seq = ExtendedSeqnum::default();

        // No wraparound when seqnums are increasing
        assert_eq!(ext_seq.next(0), (1 << 16));
        assert_eq!(ext_seq.next(10), (1 << 16) + 10);
        assert_eq!(ext_seq.next(10), (1 << 16) + 10);
        assert_eq!(
            ext_seq.next(1 + i16::MAX as u16),
            (1 << 16) + 1 + i16::MAX as u64
        );

        // Even big bumps under MAXINT16 don't result in wrap-around
        ext_seq = ExtendedSeqnum::default();

        assert_eq!(ext_seq.next(27500), (1 << 16) + 27500);
        assert_eq!(ext_seq.next(24), (1 << 16) + 24);
    }

    #[test]
    fn extended_seqnum_wraparound() {
        let mut ext_seq = ExtendedSeqnum::default();
        assert_eq!(
            ext_seq.next(u16::MAX - 9000 + 1),
            (1 << 16) + u16::MAX as u64 - 9000 + 1
        );
        assert_eq!(ext_seq.next(0), (1 << 16) + u16::MAX as u64 + 1);
        assert_eq!(ext_seq.next(9000), (1 << 16) + u16::MAX as u64 + 1 + 9000);
    }

    #[test]
    fn extended_seqnum_wraparound_disordered() {
        let mut ext_seq = ExtendedSeqnum::default();

        assert_eq!(
            ext_seq.next(u16::MAX - 9000 + 1),
            (1 << 16) + u16::MAX as u64 - 9000 + 1
        );
        assert_eq!(ext_seq.next(0), (1 << 16) + u16::MAX as u64 + 1);

        // Unwrapping around
        assert_eq!(
            ext_seq.next(u16::MAX - 9000 + 1),
            (1 << 16) + u16::MAX as u64 - 9000 + 1
        );
        assert_eq!(ext_seq.next(9000), (1 << 16) + u16::MAX as u64 + 1 + 9000);
    }

    #[test]
    fn extended_seqnum_wraparound_disordered_backwards() {
        let mut ext_seq = ExtendedSeqnum::default();

        assert_eq!(ext_seq.next(9000), (1 << 16) + 9000);

        // Wraps backwards
        assert_eq!(
            ext_seq.next(u16::MAX - 9000 + 1),
            u16::MAX as u64 - 9000 + 1
        );

        // Wraps again forwards
        assert_eq!(ext_seq.next(9000), (1 << 16) + 9000);
    }
}
