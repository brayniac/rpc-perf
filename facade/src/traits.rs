use std::any::Any;

use crate::Instant;

/// Methods common to all metrics.
pub trait Metric: Send + Sync {
    /// Get the current metric as a pointer to a
    /// type implementing `Any`.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
}

/// A counter. Counts things.
///
/// This trait should be implemented by any type that
/// should be used as a counter metric.
///
/// Counters should be used when counting something.
/// (e.g. the total number of hits on a web endpoint, the
/// number of times that a function has run, etc.)
pub trait Counter: Metric {
    fn store(&self, time: Instant, value: u64);
    fn add(&self, time: Instant, amount: u64);

    fn load(&self) -> u64;
}

/// A gauge. Measures the instananeous value of some property.
///
/// This trait should be implemented by any type that
/// can be used as a gauge metric.
///
/// Gauges measure the instantaneous value of some
/// property. (e.g. number of requests currently in flight,
/// current CPU usage, memory usage, etc.)
pub trait Gauge: Metric {
    fn store(&self, time: Instant, value: i64);
    fn add(&self, time: Instant, amount: i64);
    fn sub(&self, time: Instant, amount: i64);

    fn load(&self) -> u64;
}

/// A histogram, records values and calculates some sort of summary.
///
/// This is the most flexible of all the metric types.
pub trait Histogram: Metric {
    fn increment(&self, time: Instant, val: u64, count: u64);

    // TODO: Get buckets somehow
}
