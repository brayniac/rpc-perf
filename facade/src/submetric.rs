use std::borrow::Cow;

use crate::Percentile;

pub enum SubMetricValue {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}

pub enum SubMetric {
    Bucket {
        min: u64,
        max: u64,
        count: u64,
    },
    Quantile {
        percentile: Percentile,
        value: u64,
    },
    Custom {
        name: Cow<'static, str>,
        value: SubMetricValue,
    },
}
