

// TODO: Determine how to represent an instant
pub struct Instant {
    ns_since_epoch: u64
}

pub struct Interval {
    pub start: Instant,
    pub end: Instant,
}

pub trait AsNanoseconds {
    fn as_nanoseconds() -> Self;
}
