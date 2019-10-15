use std::any::Any;

use evmap::ShallowCopy;

use crate::{Counter, DynCow, Gauge, Histogram, Metadata, Metric, MetricType};

#[derive(Eq, PartialEq)]
pub enum MetricVal {
    Counter(DynCow<'static, dyn Counter>),
    Gauge(DynCow<'static, dyn Gauge>),
    Histogram(DynCow<'static, dyn Histogram>),
}

impl MetricVal {
    pub fn ty(&self) -> MetricType {
        match self {
            Self::Counter(_) => MetricType::Counter,
            Self::Gauge(_) => MetricType::Gauge,
            Self::Histogram(_) => MetricType::Histogram,
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct MetricInstance {
    pub(crate) inner: MetricVal,
    pub(crate) metadata: Metadata,
}

impl MetricInstance {
    pub(crate) fn new(metric: MetricVal, metadata: Metadata) -> Self {
        Self {
            inner: metric,
            metadata,
        }
    }

    pub fn ty(&self) -> MetricType {
        self.inner.ty()
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn as_metric(&self) -> &dyn Metric {
        self.metric()
    }

    pub fn metric(&self) -> &MetricVal {
        &self.inner
    }

    pub fn as_counter(&self) -> Option<&dyn Counter> {
        match self.metric() {
            MetricVal::Counter(c) => Some(&**c),
            _ => None,
        }
    }

    pub fn as_gauge(&self) -> Option<&dyn Gauge> {
        match self.metric() {
            MetricVal::Gauge(g) => Some(&**g),
            _ => None,
        }
    }

    pub fn as_histogram(&self) -> Option<&dyn Histogram> {
        match self.metric() {
            MetricVal::Histogram(h) => Some(&**h),
            _ => None,
        }
    }
}

impl Metric for MetricVal {
    fn as_any(&self) -> Option<&dyn Any> {
        match self {
            Self::Counter(c) => c.as_any(),
            Self::Gauge(g) => g.as_any(),
            Self::Histogram(h) => h.as_any(),
        }
    }
}

impl ShallowCopy for MetricInstance {
    unsafe fn shallow_copy(&mut self) -> Self {
        Self {
            inner: self.inner.shallow_copy(),
            metadata: self.metadata,
        }
    }
}

impl ShallowCopy for MetricVal {
    unsafe fn shallow_copy(&mut self) -> Self {
        match self {
            Self::Counter(x) => Self::Counter(x.shallow_copy()),
            Self::Gauge(x) => Self::Gauge(x.shallow_copy()),
            Self::Histogram(x) => Self::Histogram(x.shallow_copy()),
        }
    }
}
