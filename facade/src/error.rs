
use std::fmt;

use crate::MetricType;

#[doc(hidden)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

#[derive(Copy, Clone, Debug)]
pub enum RegisterError {
    // A metric has already been registered under that name
    MetricAlreadyExists,
    // The metric library has been shut down.
    LibraryShutdown,

    #[doc(hidden)]
    __Nonexhaustive(Empty)
}

#[derive(Copy, Clone, Debug)]
pub enum UnregisterError {
    // There is no metric with that name to remove
    NoSuchMetric,
    LibraryShutdown,

    #[doc(hidden)]
    __Nonexhaustive(Empty)
}

#[derive(Copy, Clone, Debug)]
pub enum MetricErrorData {
    InvalidUnsignedValue(i64),
    InvalidSignedValue(u64),
    /// The metric is not a type of metric that can be incremented (it's a histogram)
    InvalidIncrement {
        ty: MetricType
    },
    /// The metric is not a type of metric that can be decremented
    /// (it's either a histogram or a counter)
    InvalidDecrement{
        ty: MetricType
    },
    /// Tried to perform an operation that expected one type but
    /// instead we got another type.
    WrongType {
        expected: MetricType,
        found: MetricType
    },

    #[doc(hidden)]
    __Nonexhaustive(Empty)
}

#[derive(Copy, Clone, Debug)]
pub struct MetricError<'m> {
    pub metric: &'m str,
    pub data: MetricErrorData
}

impl<'m> MetricError<'m> {
    pub fn invalid_unsigned(metric: &'m str, val: i64) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidUnsignedValue(val)
        }
    }

    pub fn invalid_signed(metric: &'m str, val: u64) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidSignedValue(val)
        }
    }

    pub fn invalid_increment(metric: &'m str, ty: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidIncrement{ ty }
        }
    }

    pub fn invalid_decrement(metric: &'m str, ty: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidDecrement{ ty }
        }
    }

    pub fn wrong_type(metric: &'m str, expected: MetricType, found: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::WrongType{ expected, found }
        }
    }
}

impl<'m> fmt::Display for MetricError<'m> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::MetricErrorData::*;

        match &self.data {
            InvalidUnsignedValue(val) => {
                write!(
                    fmt, 
                    r#"Attempted to write a value '{}' to the metric '{}' \
                     but it could not be converted to a u64"#,
                    val,
                    self.metric
                )
            },
            InvalidSignedValue(val) => {
                write!(
                    fmt, 
                    r#"Attempted to write a value '{}' to the metric '{}' \
                       but it could not be converted to a i64"#,
                    val,
                    self.metric
                )
            },
            InvalidIncrement { ty } => {
                write!(
                    fmt,
                    r#"Attempted to increment metric '{}' but it \
                       is a {} which does not support being incremented"#,
                    self.metric,
                    ty
                )
            },
            InvalidDecrement { ty } => {
                write!(
                    fmt,
                    r#"Attempted to decrement metric '{}' but it \
                       is a {} which does not support being decrement"#,
                    self.metric,
                    ty
                )
            },
            WrongType { expected, found } => {
                write!(
                    fmt,
                    "Expected metric '{}' to be a {} but it was actually a {}",
                    self.metric,
                    expected,
                    found
                )
            },

            &__Nonexhaustive(e) => match e {}
        }
    }
}
