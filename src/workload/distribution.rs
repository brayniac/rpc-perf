use super::*;

#[derive(Clone)]
pub enum Distribution {
    Uniform(rand::distributions::Uniform<usize>),
    Zipf(zipf::ZipfDistribution),
}

impl Distribution {
    pub fn sample(&self, rng: &mut dyn RngCore) -> usize {
        match self {
            Self::Uniform(dist) => dist.sample(rng),
            Self::Zipf(dist) => dist.sample(rng),
        }
    }
}