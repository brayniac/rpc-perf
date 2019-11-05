// Copyright 2019 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::error::Error;
use std::fmt;

use crate::MetricType;

#[doc(hidden)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

/// Error for when registering a metric fails.
///
/// This enum should not be matched exhaustively. However, if for testing
/// purposes it is desired to match exhaustively, that can be done by matching
/// the final variant like this
/// ```rust
/// # use metrics_core::*;
/// # let data = RegisterError::MetricAlreadyExists;
/// match data {
///     // ...
///     RegisterError::__Nonexhaustive(empty) => match empty { },
/// #   _ => ()
/// }
/// ```
#[derive(Copy, Clone, Debug)]
pub enum RegisterError {
    /// A metric has already been registered under that name
    MetricAlreadyExists,
    /// The metrics library has been shut down
    LibraryShutdown,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

/// Error for when unregistering a metric fails.
///
/// This enum should not be matched exhaustively. However, if for testing
/// purposes it is desired to match exhaustively, that can be done by matching
/// the final variant like this
/// ```rust
/// # use metrics_core::*;
/// # let data = UnregisterError::NoSuchMetric;
/// match data {
///     // ...
///     UnregisterError::__Nonexhaustive(empty) => match empty { },
/// #   _ => ()
/// }
/// ```
#[derive(Copy, Clone, Debug)]
pub enum UnregisterError {
    /// There is no metric with that name to remove
    NoSuchMetric,
    /// The metrics library has been shut down
    LibraryShutdown,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

/// Description of why writing to a metric failed.
///
/// This enum should not be matched exhaustively. However, if for testing
/// purposes it is desired to match exhaustively, that can be done by matching
/// the final variant like this
/// ```rust
/// # use metrics_core::*;
/// # let data = MetricErrorData::InvalidUnsignedValue(0);
/// match data {
///     // ...
///     MetricErrorData::__Nonexhaustive(empty) => match empty { },
/// #   _ => ()
/// }
/// ```
#[derive(Copy, Clone, Debug)]
pub enum MetricErrorData {
    /// Tried to pass a signed value that was negative to something expecting an
    /// unsigned value.
    InvalidUnsignedValue(i64),
    /// Tried to pass an unsigned value that was too large to something
    /// expecting a signed value.
    InvalidSignedValue(u64),
    /// The metric is not a type of metric that can be incremented (it's a
    /// histogram)
    InvalidIncrement {
        /// The type of metric we attempted to increment.
        ty: MetricType,
    },
    /// The metric is not a type of metric that can be decremented (it's either
    /// a histogram or a counter)
    InvalidDecrement {
        /// The type of metric we attempted to decrement
        ty: MetricType,
    },
    /// Tried to perform an operation that expected one type but instead we got
    /// another type.
    WrongType {
        /// The type of metric that we expected to find.
        expected: MetricType,
        /// What was actually found.
        found: MetricType,
    },

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

/// An error for when writing to a metric failed.
#[derive(Copy, Clone, Debug)]
pub struct MetricError<'m> {
    /// The metric that was being written to.
    pub metric: &'m str,
    /// Details on what exactly the problem was
    pub data: MetricErrorData,
}

impl<'m> MetricError<'m> {
    pub(crate) fn invalid_unsigned(metric: &'m str, val: i64) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidUnsignedValue(val),
        }
    }

    pub(crate) fn invalid_signed(metric: &'m str, val: u64) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidSignedValue(val),
        }
    }

    pub(crate) fn invalid_increment(metric: &'m str, ty: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidIncrement { ty },
        }
    }

    pub(crate) fn invalid_decrement(metric: &'m str, ty: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::InvalidDecrement { ty },
        }
    }

    pub(crate) fn wrong_type(metric: &'m str, expected: MetricType, found: MetricType) -> Self {
        Self {
            metric,
            data: MetricErrorData::WrongType { expected, found },
        }
    }
}

impl<'m> fmt::Display for MetricError<'m> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::MetricErrorData::*;

        match &self.data {
            InvalidUnsignedValue(val) => write!(
                fmt,
                r#"Attempted to write a value '{}' to the metric '{}' \
                     but it could not be converted to a u64"#,
                val, self.metric
            ),
            InvalidSignedValue(val) => write!(
                fmt,
                r#"Attempted to write a value '{}' to the metric '{}' \
                       but it could not be converted to a i64"#,
                val, self.metric
            ),
            InvalidIncrement { ty } => write!(
                fmt,
                r#"Attempted to increment metric '{}' but it \
                       is a {} which does not support being incremented"#,
                self.metric, ty
            ),
            InvalidDecrement { ty } => write!(
                fmt,
                r#"Attempted to decrement metric '{}' but it \
                       is a {} which does not support being decrement"#,
                self.metric, ty
            ),
            WrongType { expected, found } => write!(
                fmt,
                "Expected metric '{}' to be a {} but it was actually a {}",
                self.metric, expected, found
            ),

            &__Nonexhaustive(e) => match e {},
        }
    }
}

impl<'m> Error for MetricError<'m> {}
