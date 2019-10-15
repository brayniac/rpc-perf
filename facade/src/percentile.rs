#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Percentile {
    // Representation is a fixed-point number from 1 to 100000
    // with 100000 being 1 and 0 being 0.
    val: u32,
}

macro_rules! percentile {
    ($val:expr) => {
        Percentile { val: $val }
    };
}

impl Percentile {
    pub fn new(val: u32) -> Option<Self> {
        if val > 100000 {
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
            val: (val.min(1.0).max(0.0) * 100000.0) as u32,
        }
    }

    pub const fn minimum() -> Self {
        percentile!(0)
    }

    pub const fn maximum() -> Self {
        percentile!(100000)
    }

    pub const fn p001() -> Self {
        percentile!(10)
    }

    pub const fn p01() -> Self {
        percentile!(100)
    }

    pub const fn p1() -> Self {
        percentile!(1000)
    }

    pub const fn p5() -> Self {
        percentile!(5000)
    }

    pub const fn p10() -> Self {
        percentile!(10000)
    }

    pub const fn p25() -> Self {
        percentile!(25000)
    }

    pub const fn p50() -> Self {
        percentile!(50000)
    }

    pub const fn p75() -> Self {
        percentile!(75000)
    }

    pub const fn p90() -> Self {
        percentile!(90000)
    }

    pub const fn p95() -> Self {
        percentile!(95000)
    }

    pub const fn p99() -> Self {
        percentile!(99000)
    }

    pub const fn p999() -> Self {
        percentile!(99900)
    }

    pub const fn p9999() -> Self {
        percentile!(99990)
    }
}
