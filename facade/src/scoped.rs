use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem;

use crate::{
    register_counter, register_gauge, register_histogram, unregister_metric, Counter, Gauge,
    Histogram, Metadata, Metric, RegisterError,
};

/// A metric with a non-static lifetime.
///
/// This type allows for stack-allocated metrics to be used
/// easily and conveniently.
///
/// # Safety
/// For this type to be safe you must ensure that its drop
/// implementation is run. If this doesn't happen then values
/// recorded to this metric will use a dangling reference.
///
/// For this reason, all constructors on this type are `unsafe`.
pub struct ScopedMetric<'m, M: Metric> {
    marker: PhantomData<&'m M>,
    name: Cow<'static, str>,
}

impl<'m, M: Counter> ScopedMetric<'m, M> {
    pub unsafe fn counter(
        name: impl Into<Cow<'static, str>>,
        metric: &'m M,
        metadata: impl Into<Option<Metadata>>,
    ) -> Result<Self, RegisterError> {
        let static_metric: &'static dyn Counter = mem::transmute(metric as &dyn Counter);
        let metadata = metadata.into().unwrap_or(Metadata::empty());
        let name = name.into();

        register_counter(name.clone(), static_metric, metadata)?;

        Ok(Self {
            marker: PhantomData,
            name,
        })
    }
}

impl<'m, M: Gauge> ScopedMetric<'m, M> {
    pub unsafe fn gauge(
        name: impl Into<Cow<'static, str>>,
        metric: &'m M,
        metadata: impl Into<Option<Metadata>>,
    ) -> Result<Self, RegisterError> {
        let static_metric: &'static dyn Gauge = mem::transmute(metric as &dyn Gauge);
        let metadata = metadata.into().unwrap_or(Metadata::empty());
        let name = name.into();

        register_gauge(name.clone(), static_metric, metadata)?;

        Ok(Self {
            marker: PhantomData,
            name,
        })
    }
}

impl<'m, M: Histogram> ScopedMetric<'m, M> {
    pub unsafe fn histogram(
        name: impl Into<Cow<'static, str>>,
        metric: &'m M,
        metadata: impl Into<Option<Metadata>>,
    ) -> Result<Self, RegisterError> {
        let static_metric: &'static dyn Histogram = mem::transmute(metric as &dyn Histogram);
        let metadata = metadata.into().unwrap_or(Metadata::empty());
        let name = name.into();

        register_histogram(name.clone(), static_metric, metadata)?;

        Ok(Self {
            marker: PhantomData,
            name,
        })
    }
}

impl<'m, M: Metric> Drop for ScopedMetric<'m, M> {
    fn drop(&mut self) {
        let _ = unregister_metric(&self.name);
    }
}
