use std::any::Any;

use crate::{Instant, SubMetric};

/// Methods common to all metrics.
pub trait MetricCommon: Send + Sync {
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
pub trait Counter: MetricCommon {
    /// Set the value of the counter.
    fn store(&self, time: Instant, value: u64);
    /// Add a value to the counter.
    fn add(&self, time: Instant, amount: u64);

    /// Get the current value of the counter.
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
pub trait Gauge: MetricCommon {
    /// Store a value into the gauge.
    fn store(&self, time: Instant, value: i64);
    /// Add a value to the gauge.
    fn add(&self, time: Instant, amount: i64);
    /// Subtract a value from the gauge.
    fn sub(&self, time: Instant, amount: i64);

    /// Get the current value of the gauge.
    fn load(&self) -> u64;
}

/// Any sort of summary of the record values
pub trait Summary: MetricCommon {
    /// Record `count` instances of `val`.
    fn record(&self, time: Instant, val: u64, count: u64);

    /// Get all statistics exposed by the implementation
    fn submetrics(&self) -> Vec<SubMetric>;
}
