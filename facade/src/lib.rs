// Copyright 2019 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Metrics facade that allows for using multiple metrics
//! backends.
//! 
//! Metrics types should implement one of [`Counter`][counter],
//! [`Gauge`][gauge], or [`Histogram`][histogram]. Then they
//! can be registered through one of [`register_counter`][rctr], 
//! [`register_gauge`][rgauge], or [`register_histogram`][rhist]
//! functions.
//! 
//! # Metadata
//! Each individual metric can have metadata associated with.
//! This is a set of static key-value pairs that can be used
//! to store arbitrary properties of the metric. Empty metadata
//! can be created by calling [`Metadata::new`](facade::Metadata::new).
//! Otherwise, `Metadata` is created using the `metadata` macro.
//! 
//! # Introspection
//! TODO
//! 
//! # Example
//! ```rust
//! # use facade::*;
//! # struct Metric;
//! # impl facade::Metric for Metric {}
//! # impl Histogram for Metric {
//! #   fn record_values(&self, time: Instant, val: u64, count: u64) {}
//! # }
//! # fn function_that_takes_some_time() {}
//! // Create a metric named "example.metric" with no associated metadata
//! register_histogram("example.metric", Box::new(Metric), Metadata::empty());
//! 
//! // Alternatively, we can add metadata using the metadata! macro.
//! register_histogram(
//!     "example.metadata",
//!     Box::new(Metric),
//!     metadata! {
//!         "some key" => "some value",
//!         "unit" => "ns"
//!     }
//! );
//! 
//! // If you have a static reference to a metric, then you can avoid boxing it
//! static METRIC_INSTANCE: Metric = Metric;
//! register_histogram(
//!     "example.static",
//!     &METRIC_INSTANCE,
//!     Metadata::empty()
//! );
//! 
//! // Now we can use these metrics like so
//! 
//! // Record single values to "example.metric"
//! value!("example.metric", 10);
//! value!("example.metric", 11);
//! 
//! // Record a value and a count, note that the count
//! // is only used by histograms.
//! value!("example.static", 120, 44);
//! 
//! // Can also record timings
//! let start = current_time();
//! function_that_takes_some_time();
//! let end = current_time();
//! // This gets translated to a duration in nanoseconds
//! timing!("example.metadata", start, end);
//! ```
//! 
//! [counter]: facade::Counter
//! [gauge]: facade::Gauge
//! [histogram]: facade::Histogram
//! [rctr]: facade::register_counter
//! [rgauge]: facade::register_gauge
//! [rhist]: facade::register_histogram

#![allow(unused_variables)]

#[macro_use]
extern crate log;

#[macro_use]
mod macros;

mod dyncow;
mod metadata;
mod traits;
mod percentile;
mod state;
mod value;

use std::borrow::Cow;

pub use crate::dyncow::DynCow;
pub use crate::metadata::Metadata;
pub use crate::traits::{Counter, Gauge, Histogram, Instant, Interval, Metric};
pub use crate::percentile::Percentile;
pub use crate::value::MetricValue;

use crate::state::{State, MetricInner};

pub enum RegisterError {
    MetricAlreadyExists,
    LibraryShutdown
}
pub enum UnregisterError {
    NoSuchMetric,
    LibraryShutdown
}

/// Register a new counter under the given name.
/// 
/// 
pub fn register_counter(
    name: impl Into<Cow<'static, str>>,
    counter: impl Into<DynCow<'static, dyn Counter + Send + Sync>>,
    metadata: Metadata,
) -> Result<(), RegisterError> {
    State::get().register_metric(
        name.into(),
        MetricInner::Counter(counter.into()),
        metadata
    )
}

pub fn register_gauge(
    name: impl Into<Cow<'static, str>>,
    gauge: impl Into<DynCow<'static, dyn Gauge + Send + Sync>>,
    metadata: Metadata,
) -> Result<(), RegisterError> {
    State::get().register_metric(
        name.into(),
        MetricInner::Gauge(gauge.into()),
        metadata
    )
}

pub fn register_histogram(
    name: impl Into<Cow<'static, str>>,
    histogram: impl Into<DynCow<'static, dyn Histogram + Send + Sync>>,
    metadata: Metadata,
) -> Result<(), RegisterError> {
    State::get().register_metric(
        name.into(),
        MetricInner::Histogram(histogram.into()),
        metadata
    )
}

pub fn unregister_metric(name: impl AsRef<str>) -> Result<(), UnregisterError> {
    State::get().unregister_metric(name.as_ref())
}

pub fn record_value(
    name: impl AsRef<str>,
    value: impl Into<MetricValue>,
    count: impl Into<u64>,
    time: Instant
) {
    let name = name.as_ref();
    let value = value.into();
    let count = count.into();

    State::get().record_value(name, value, count, time);
}

#[doc(hidden)]
pub mod export {
    use super::*;

    pub fn create_metadata(attributes: &'static [(&'static str, &'static str)]) -> Metadata {
        Metadata::new(attributes)
    }

    pub fn current_time() -> Instant {
        unimplemented!()
    }
}
