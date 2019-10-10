
use std::ops::Sub;

use time;

/// High-resolution timestamp
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Instant {
    // Our representation is the number of nanoseconds
    // since an unspecified (but consistent within the 
    // process) epoch. For more details see the docs
    // for the time crate.
    // 
    // In practice this is the return value of
    // `time::precise_time_ns`
    ns_since_epoch: u64
}

impl Instant {
    pub fn now() -> Self {
        Instant {
            ns_since_epoch: time::precise_time_ns()
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Interval {
    duration_ns: u64
}

impl Interval {
    pub fn from_seconds(secs: u64) -> Self {
        Self { duration_ns: secs.saturating_mul(1_000_000_000) }
    }

    pub fn from_millis(millis: u64) -> Self {
        Self { duration_ns: millis.saturating_mul(1_000_000) }
    }

    pub fn from_micros(micros: u64) -> Self {
        Self { duration_ns: micros.saturating_mul(1_000) }
    }

    pub fn from_nanos(nanos: u64) -> Self {
        Self { duration_ns: nanos }
    }

    pub fn as_seconds(self) -> u64 {
        self.duration_ns / 1_000_000_000
    }

    pub fn as_millis(self) -> u64 {
        self.duration_ns / 1_000_000
    }

    pub fn as_micros(self) -> u64 {
        self.duration_ns / 1_000
    }

    pub fn as_nanos(self) -> u64 {
        self.duration_ns
    }
}

impl Sub<Instant> for Instant {
    type Output = Interval;
    
    fn sub(self, other: Instant) -> Interval {
        Interval {
            duration_ns: self.ns_since_epoch.saturating_sub(other.ns_since_epoch)
        }
    }
}
