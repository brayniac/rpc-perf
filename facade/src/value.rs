

#[derive(Copy, Clone, Debug)]
pub enum MetricValue {
    Signed(i64),
    Unsigned(u64)
}

impl MetricValue {
    #[inline]
    pub fn as_u64(self) -> Option<u64> {
        match self {
            Self::Signed(x) if x >= 0 => Some(x as u64),
            Self::Signed(_) => None,
            Self::Unsigned(x) => Some(x),
        }
    }

    #[inline]
    pub fn as_i64(self) -> Option<i64> {
        match self {
            Self::Unsigned(x) if x > std::i64::MAX as u64 => None,
            Self::Unsigned(x) => Some(x as i64),
            Self::Signed(x) => Some(x)
        }
    }

    #[inline]
    pub fn as_u64_unchecked(self) -> u64 {
        match self {
            Self::Signed(x) => x as u64,
            Self::Unsigned(x) => x,
        }
    }

    #[inline]
    pub fn as_i64_unchecked(self) -> i64 {
        match self {
            Self::Signed(x) => x,
            Self::Unsigned(x) => x as i64,
        }
    }
}



