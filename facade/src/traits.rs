use std::any::Any;

// TODO: Determine how to represent an instant
pub struct Instant {}

pub struct Interval {
    pub start: Instant,
    pub end: Instant,
}

pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// Methods common to all metrics.
/// 
/// Currently empty, more methods TBD
pub trait Metric {
    fn as_counter(&self) -> Option<&dyn Counter>;
    fn as_gauge(&self) -> Option<&dyn Gauge>;
    fn as_histogram(&self) -> Option<&dyn Histogram>;

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
    fn record_value(&self, time: Instant, val: u64);
    fn increment(&self, time: Instant, incr: u64);

    fn value(&self) -> u64;
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
    fn record_value(&self, time: Instant, val: i64);
    fn increment(&self, time: Instant, incr: i64);
    fn decrement(&self, time: Instant, decr: i64);

    fn value(&self) -> u64;
}

/// A histogram, records values and calculates some sort of summary.
/// 
/// This is the most flexible of all the metric types.
pub trait Histogram: Metric {
    fn record_values(&self, time: Instant, val: u64, count: u64);

    // TODO: Get buckets somehow
}
