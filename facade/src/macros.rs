
/// Record a value to a metric.
/// 
/// For counters and gauges this should directly set the value of
/// the metric, for histograms it will get rolled into a summary.
/// 
/// The required parameters for this macro are
/// - `name`: a string literal with the name of the metric
/// - `value`: the value to be recorded
/// 
/// Optional parameters
/// - `count`: The number of times the value should be registered.
///     This is ignored for counters and gauges. If not given, this
///     defaults to 1.
/// - `time`: The time at which the value was recorded, given as `time = <expr>`.
///     If not given then defaults to the current time.
#[macro_export]
macro_rules! value {
    ($name:literal, $value:expr) => {
        value!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, $count:expr) => {
        unimplemented!()
    };
    ($name:literal, $value:expr, time = $time:expr) => {
        value!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, $count:expr, time=$time:expr) => {
        unimplemented!()
    };
}

/// Increment a counter or gauge.
/// 
/// If the metric is a counter or a gauge then it will increment the
/// stored value within the metric. If the provided metric is not a
/// histogram, then it will call the user-provided error function.
/// 
/// The the only required parameter for this macro is
/// - `name`: a string literal with the name of the metric.
/// 
/// Optional Paramters
/// - `value`: the amount by which to increment the counter/gauge.
///     If not specified this defaults to `1`.
/// - `time`: The time at which the increment happened. If not specified
///     this defaults to the current time. Specified like `time = <expr>`.
#[macro_export]
macro_rules! increment {
    ($name:literal) => {
        increment!($name, 1)
    };
    ($name:literal, $value:expr) => {
        increment!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, time = $time:expr) => {
        unimplemented!()
    };
}

/// Decrement a gauge.
/// 
/// If the metric is a gauge then it will decrement the
/// stored value within the metric. If the provided metric is not a
/// histogram, then it will call the user-provided error function.
/// 
/// The the only required parameter for this macro is
/// - `name`: a string literal with the name of the metric.
/// 
/// Optional Paramters
/// - `value`: the amount by which to decrement the gauge.
///     If not specified this defaults to `1`.
/// - `time`: The time at which the decrement happened. If not specified
///     this defaults to the current time. Specified like `time = <expr>`.
#[macro_export]
macro_rules! decrement {
    ($name:literal) => {
        decrement!($name, 1)
    };
    ($name:literal, $value:expr) => {
        decrement!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, time = $time:expr) => {
        unimplemented!()
    }
}

/// Set the value of a counter.
/// 
/// If the metric is not a counter then it will call the
/// user-defined error function.
/// 
/// ## Parameters
/// - `name`: A string literal with the name of the metric.
/// - `value`: The new value of the counter.
#[macro_export]
macro_rules! counter {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
}

/// Set the value of a gauge.
/// 
/// If the metric is not a gauge then it will call the
/// user-defined error function.
/// 
/// ## Parameters
/// - `name`: A string literal with the name of the metric.
/// - `value`: The new value of the gauge.
#[macro_export]
macro_rules! gauge {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
}

/// Record a timing interval.
/// 
/// This is equivalent to calling `value!` with the
/// interval. 
/// 
/// This macro supports two argument formats. Either
/// it takes the duration the interval or it takes a
/// start and end time and uses that to calculate the
/// interval duration.
#[macro_export]
macro_rules! interval {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
    ($name:literal, $start:expr, $end:expr) => {
        unimplemented!()
    }
}

/// Register a new counter metric.
#[macro_export]
macro_rules! register_counter {
    (
        $name:expr,
        $counter:expr
        $( ,
            { $( $key:ident : $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_counter!($name, $counter, $crate::metadata! {
            $( $key : $val ),*
        })
    };
    (
        $name:expr,
        $counter:expr,
        $( ,
            { $( $key:expr => $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_counter!($name, $counter, $crate::metadata! {
            $( $key => $val ),*
        })
    };
    (
        $name:expr,
        $counter:expr,
        $metadata:expr $(,)?
    ) => {
        $crate::register_counter(
            $name,
            $counter,
            $metadata
        )
    }
}

/// Register a new gauge metric
#[macro_export]
macro_rules! register_gauge {
    (
        $name:expr,
        $gauge:expr
        $( ,
            { $( $key:tt : $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_gauge!($name, $gauge, $crate::metadata! {
            $( $key : $val ),*
        })
    };
    (
        $name:expr,
        $gauge:expr,
        $( ,
            { $( $key:expr => $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_gauge!($name, $gauge, $crate::metadata! {
            $( $key => $val ),*
        })
    };
    (
        $name:expr,
        $gauge:expr,
        $metadata:expr $(,)?
    ) => {
        $crate::register_gauge(
            $name,
            $counter,
            $metadata
        )
    }
}

/// Register a new histogram.
#[macro_export]
macro_rules! register_histogram {
    (
        $name:expr,
        $histogram:expr
        $( ,
            { $( $key:tt : $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_histogram!($name, $histogram, $crate::metadata! {
            $( $key : $val ),*
        })
    };
    (
        $name:expr,
        $histogram:expr,
        $( ,
            { $( $key:expr => $val:expr ),* $(,)? }
        )? $(,)?
    ) => {
        register_counter!($name, $histogram, $crate::metadata! {
            $( $key => $val ),*
        })
    };
    (
        $name:expr,
        $histogram:expr,
        $metadata:expr $(,)?
    ) => {
        $crate::register_histogram(
            $name,
            $counter,
            $metadata
        )
    }
}

#[macro_export]
macro_rules! metadata {
    {
        $( $key:ident : $val:expr ),* $(,)?
    } => {
        $crate::export::create_metadata(&[
            $( ( stringify!($key), $val ) ),*
        ])
    };
    {
        $( $key:expr => $val:expr ),* $(,)?
    } => {
        $crate::export::create_metadata(&[
            $( ( $key, $val ) ),*
        ])
    };
}
