use std::borrow::Cow;
use std::sync::Mutex;

use evmap::{self, ShallowCopy};
use once_cell::sync::Lazy;
use thread_local::CachedThreadLocal;

use crate::{
    Counter, DynCow, Gauge, Histogram, Instant, Metadata, MetricValue, RegisterError,
    UnregisterError,
};

static STATE: Lazy<State> = Lazy::new(State::new);

#[derive(Eq, PartialEq)]
pub(crate) enum MetricInner {
    Counter(DynCow<'static, dyn Counter + Send + Sync>),
    Gauge(DynCow<'static, dyn Gauge + Send + Sync>),
    Histogram(DynCow<'static, dyn Histogram + Send + Sync>),
}

#[derive(Eq, PartialEq)]
pub(crate) struct MetricInstance {
    metric: MetricInner,
    metadata: Metadata,
}

type WriteHandle = evmap::WriteHandle<Cow<'static, str>, MetricInstance>;
type ReadHandle = evmap::ReadHandle<Cow<'static, str>, MetricInstance>;
type ReadHandleFactory = evmap::ReadHandleFactory<Cow<'static, str>, MetricInstance>;

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

    pub(crate) fn get() -> &'static Self {
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

    pub(crate) fn record_value(
        &self,
        name: &str,
        value: MetricValue,
        count: u64,
        time: Instant,
    ) -> bool {
        let reader = self.reader();

        if reader.is_destroyed() {
            return false;
        }

        reader
            .get_and(name, |val| {
                let ref inner = val[0];

                match &inner.metric {
                    MetricInner::Counter(counter) => {
                        if let Some(val) = value.as_u64() {
                            counter.record_value(time, val);
                        } else {
                            // TODO: How to do error handling?
                            warn!("Tried record an invalid value to a counter");
                        }
                    }
                    MetricInner::Gauge(gauge) => {
                        if let Some(val) = value.as_i64() {
                            gauge.record_value(time, val);
                        } else {
                            // TODO: error handling?
                            warn!("Tried to record an invalid value to a gauge");
                        }
                    }
                    MetricInner::Histogram(histogram) => {
                        if let Some(val) = value.as_u64() {
                            histogram.record_values(time, val, count);
                        } else {
                            // TODO: error handling?
                            warn!("Tried to record a negative invalid value to a histogram");
                        }
                    }
                }
            })
            .is_some()
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
