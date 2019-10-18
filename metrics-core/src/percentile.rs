#![allow(missing_docs)]

const MAX_PERCENTILE: u32 = 1000000000u32;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Percentile {
    // Representation is a fixed-point number from 1 to 1000000000
    // with 1000000000 being 1 and 0 being 0.
    val: u32,
}

macro_rules! p {
    ($val:expr) => {
        Percentile { val: $val }
    };
}

impl Percentile {
    pub fn new(val: u32) -> Option<Self> {
        if val > MAX_PERCENTILE {
            return None;
        }
        Some(Percentile { val })
    }

    pub const unsafe fn new_unchecked(val: u32) -> Self {
        Self { val }
    }

    pub const fn into_inner(self) -> u32 {
        self.val
    }

    pub fn from_float(val: f64) -> Self {
        Self {
            val: (val.min(1.0).max(0.0) * (MAX_PERCENTILE as f64)) as u32,
        }
    }

    pub fn as_float(self) -> f64 {
        (self.val as f64) / (MAX_PERCENTILE as f64)
    }

    pub const fn minimum() -> Self {
        p!(0)
    }

    pub const fn maximum() -> Self {
        p!(MAX_PERCENTILE)
    }

    pub const fn p001() -> Self {
        p!(Self::maximum().val / 10000)
    }

    pub const fn p01() -> Self {
        p!(Self::maximum().val / 1000)
    }

    pub const fn p1() -> Self {
        p!(Self::maximum().val / 100)
    }

    pub const fn p5() -> Self {
        p!(Self::maximum().val / 20)
    }

    pub const fn p10() -> Self {
        p!(Self::maximum().val / 10)
    }

    pub const fn p25() -> Self {
        p!(Self::maximum().val / 4)
    }

    pub const fn p50() -> Self {
        p!(Self::maximum().val / 2)
    }

    pub const fn p75() -> Self {
        p!(Self::p25().val * 3)
    }

    pub const fn p90() -> Self {
        p!(Self::p10().val * 9)
    }

    pub const fn p95() -> Self {
        p!(Self::maximum().val - Self::p5().val)
    }

    pub const fn p99() -> Self {
        p!(Self::maximum().val - Self::p1().val)
    }

    pub const fn p999() -> Self {
        p!(Self::maximum().val - Self::p01().val)
    }

    pub const fn p9999() -> Self {
        p!(Self::maximum().val - Self::p001().val)
    }
}
