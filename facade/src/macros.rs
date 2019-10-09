#[macro_export]
macro_rules! value {
    ($name:literal, $value:expr) => {
        record!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, $count:expr) => {
        unimplemented!()
    };
    ($name:literal, $value:expr, time = $time:expr) => {
        record!($name, $value, time = $crate::export::current_time())
    };
    ($name:literal, $value:expr, $count:expr, time=$time:expr) => {
        unimplemented!()
    };
}

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

#[macro_export]
macro_rules! counter {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
}

#[macro_export]
macro_rules! gauge {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
}

#[macro_export]
macro_rules! timing {
    ($name:literal, $value:expr) => {
        unimplemented!()
    };
    ($name:literal, $start:expr, $end:expr) => {
        unimplemented!()
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
