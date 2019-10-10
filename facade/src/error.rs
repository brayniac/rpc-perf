
use crate::MetricType;

pub enum RegisterError {
    // A metric has already been registered under that name
    MetricAlreadyExists,
    // The metric library has been shut down.
    LibraryShutdown,
}
pub enum UnregisterError {
    // There is no metric with that name to remove
    NoSuchMetric,
    LibraryShutdown,
}

pub enum MetricError<'a> {
    UnknownMetric(&'a str),
    InvalidUnsignedValue(i64),
    InvalidSignedValue(u64),
    /// The metric is not a type of metric that can be incremented (it's a histogram)
    InvalidIncrement {
        metric: &'a str,
        ty: MetricType
    },
    /// The metric is not a type of metric that can be decremented
    /// (it's either a histogram or a counter)
    InvalidDecrement{
        metric: &'a str,
        ty: MetricType
    },

    WrongType {
        metric: &'a str,
        expected: MetricType,
        found: MetricType
    }
}
