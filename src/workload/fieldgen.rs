use std::sync::Arc;
use std::borrow::Cow;
use rand::SeedableRng;
pub use rand_xoshiro::Seed512;
use rand::RngCore;
use rand_xoshiro::Xoshiro512PlusPlus;

// General purpose pseudorandom number generator
pub struct Random {
	inner: Xoshiro512PlusPlus,
}

impl RngCore for Random {
	fn next_u32(&mut self) -> u32 { self.inner.next_u32() }
	fn next_u64(&mut self) -> u64 { self.inner.next_u64() }
	fn fill_bytes(&mut self, buf: &mut [u8]) { self.inner.fill_bytes(buf) }
	fn try_fill_bytes(&mut self, buf: &mut [u8]) -> std::result::Result<(), rand::Error> { self.inner.try_fill_bytes(buf) }
}

impl SeedableRng for Random {
	type Seed = Seed512;

	fn from_seed(seed: Self::Seed) -> Self {
		Self {
			inner: Xoshiro512PlusPlus::from_seed(seed)
		}
	}
}

pub struct RandomSet {
	values: Vec<Arc<Vec<u8>>>,
	weights: Vec<usize>,
}