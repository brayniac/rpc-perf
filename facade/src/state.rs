use std::borrow::Cow;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use evmap::{self, ShallowCopy};
use once_cell::sync::Lazy;
use thread_local::CachedThreadLocal;

use crate::{
    Counter, DynCow, Gauge, Histogram, Instant, Metadata, MetricError, MetricType, MetricValue,
    RegisterError, UnregisterError,
};

#[derive(Eq, PartialEq)]
pub(crate) enum MetricInner {
    Counter(DynCow<'static, dyn Counter + Send + Sync>),
    Gauge(DynCow<'static, dyn Gauge + Send + Sync>),
    Histogram(DynCow<'static, dyn Histogram + Send + Sync>),
}

impl MetricInner {
    pub(crate) fn ty(&self) -> MetricType {
        match self {
            Self::Counter(_) => MetricType::Counter,
            Self::Gauge(_) => MetricType::Gauge,
            Self::Histogram(_) => MetricType::Histogram,
        }
    }
}

#[derive(Eq, PartialEq)]
pub(crate) struct MetricInstance {
    metric: MetricInner,
    metadata: Metadata,
}

type WriteHandle = evmap::WriteHandle<Cow<'static, str>, MetricInstance>;
type ReadHandle = evmap::ReadHandle<Cow<'static, str>, MetricInstance>;
type ReadHandleFactory = evmap::ReadHandleFactory<Cow<'static, str>, MetricInstance>;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static STATE: Lazy<State> = Lazy::new(|| {
    INITIALIZED.store(true, Ordering::Relaxed);
    State::new()
});

pub(crate) struct State {
    writer: Mutex<WriteHandle>,
    factory: ReadHandleFactory,
    tls: CachedThreadLocal<ReadHandle>,
}

impl State {
    fn new() -> Self {
        let (reader, writer) = evmap::new();

        Self {
            writer: Mutex::new(writer),
            factory: reader.factory(),
            tls: CachedThreadLocal::new(),
        }
    }

    fn reader(&self) -> &ReadHandle {
        self.tls.get_or(|| self.factory.handle())
    }

    #[inline]
    pub(crate) fn get() -> Option<&'static Self> {
        if INITIALIZED.load(Ordering::Relaxed) {
            Some(&*STATE)
        } else {
            None
        }
    }

    /// If the value hasn't been initializes then it creates it as well
    #[inline]
    pub(crate) fn get_force() -> &'static Self {
        &*STATE
    }

    /// Register a new metric, returns whether the metric was registered succesfully
    pub(crate) fn register_metric(
        &self,
        name: Cow<'static, str>,
        metric: MetricInner,
        metadata: Metadata,
    ) -> Result<(), RegisterError> {
        let mut writer = self.writer.lock().unwrap();
        let instance = MetricInstance { metric, metadata };

        if writer.is_destroyed() {
            return Err(RegisterError::LibraryShutdown);
        }

        if writer.contains_key(&name) {
            return Err(RegisterError::MetricAlreadyExists);
        }

        writer.update(name, instance);

        writer.refresh();

        return Ok(());
    }

    /// Unregister an existing metric, if the metric doesn't exist then this
    /// method does nothing.
    ///
    /// I'd like to somehow return the existing entry in the hash table.
    /// Unfortunately, evmap doesn't offer an API to get the removed value
    /// or even to tell if we removed a value.
    pub(crate) fn unregister_metric(&self, name: &str) -> Result<(), UnregisterError> {
        let mut writer = self.writer.lock().unwrap();

        if writer.is_destroyed() {
            return Err(UnregisterError::LibraryShutdown);
        }

        if !writer.contains_key(name) {
            return Err(UnregisterError::NoSuchMetric);
        }

        // Hack to get a Cow with a static lifetime
        // this is safe since we immediately call writer.refresh
        writer.empty(Cow::Borrowed(unsafe { &*(name as *const str) }));
        writer.refresh();

        Ok(())
    }

    #[cold]
    fn error(&self, err: MetricError) {
        unimplemented!()
    }

    pub(crate) fn record_value(&self, name: &str, value: MetricValue, count: u64, time: Instant) {
        let reader = self.reader();

        reader.get_and(name, |val| match &val[0].metric {
            MetricInner::Counter(counter) => {
                if let Some(val) = value.as_u64() {
                    counter.store(time, val);
                } else {
                    self.error(MetricError::InvalidUnsignedValue(value.as_i64_unchecked()));
                }
            }
            MetricInner::Gauge(gauge) => {
                if let Some(val) = value.as_i64() {
                    gauge.store(time, val);
                } else {
                    self.error(MetricError::InvalidSignedValue(value.as_u64_unchecked()));
                }
            }
            MetricInner::Histogram(histogram) => {
                if let Some(val) = value.as_u64() {
                    histogram.increment(time, val, count);
                } else {
                    self.error(MetricError::InvalidUnsignedValue(value.as_i64_unchecked()));
                }
            }
        });
    }

    pub(crate) fn record_increment(&self, name: &str, value: MetricValue, time: Instant) {
        let reader = self.reader();

        reader.get_and(name, |val| match &val[0].metric {
            MetricInner::Counter(counter) => match value.as_u64() {
                Some(val) => counter.add(time, val),
                None => self.error(MetricError::InvalidUnsignedValue(value.as_i64_unchecked())),
            },
            MetricInner::Gauge(gauge) => match value.as_i64() {
                Some(val) => gauge.add(time, val),
                None => self.error(MetricError::InvalidSignedValue(value.as_u64_unchecked())),
            },
            MetricInner::Histogram(_) => self.error(MetricError::InvalidIncrement {
                metric: name,
                ty: MetricType::Histogram,
            }),
        });
    }

    pub(crate) fn record_decrement(&self, name: &str, value: MetricValue, time: Instant) {
        let reader = self.reader();

        reader.get_and(name, |val| match &val[0].metric {
            MetricInner::Gauge(gauge) => match value.as_i64() {
                Some(val) => gauge.sub(time, val),
                None => self.error(MetricError::InvalidSignedValue(value.as_u64_unchecked())),
            },
            metric => self.error(MetricError::InvalidDecrement {
                metric: name,
                ty: metric.ty(),
            }),
        });
    }

    pub(crate) fn record_counter_value(&self, name: &str, value: u64, time: Instant) {
        let reader = self.reader();

        if reader.is_destroyed() {
            return;
        }

        reader.get_and(name, |val| match &val[0].metric {
            MetricInner::Counter(counter) => counter.store(time, value),
            metric => self.error(MetricError::WrongType {
                metric: name,
                expected: MetricType::Counter,
                found: metric.ty(),
            }),
        });
    }

    pub(crate) fn record_gauge_value(&self, name: &str, value: i64, time: Instant) {
        let reader = self.reader();

        if reader.is_destroyed() {
            return;
        }

        reader.get_and(name, |val| match &val[0].metric {
            MetricInner::Gauge(gauge) => gauge.store(time, value),
            metric => self.error(MetricError::WrongType {
                metric: name,
                expected: MetricType::Gauge,
                found: metric.ty(),
            }),
        });
    }
}

impl ShallowCopy for MetricInstance {
    unsafe fn shallow_copy(&mut self) -> Self {
        Self {
            metric: self.metric.shallow_copy(),
            metadata: self.metadata,
        }
    }
}

impl ShallowCopy for MetricInner {
    unsafe fn shallow_copy(&mut self) -> Self {
        match self {
            Self::Counter(x) => Self::Counter(x.shallow_copy()),
            Self::Gauge(x) => Self::Gauge(x.shallow_copy()),
            Self::Histogram(x) => Self::Histogram(x.shallow_copy()),
        }
    }
}
