use super::*;

mod cache;
mod leaderboards;
mod oltp;
mod store;
mod topics;

pub use cache::*;
pub use leaderboards::*;
pub use oltp::*;
pub use store::*;
pub use topics::*;

fn one() -> usize {
    1
}

pub trait Command {
    fn weight(&self) -> usize;
}

#[derive(Clone, Deserialize)]
pub struct Workload {
    #[serde(default)]
    keyspace: Vec<Keyspace>,
    #[serde(default)]
    stores: Vec<Store>,
    #[serde(default)]
    topics: Vec<Topics>,
    #[serde(default)]
    oltp: Option<Oltp>,
    #[serde(default)]
    leaderboards: Vec<LeaderboardsConfig>,
    threads: usize,
    ratelimit: Ratelimit,
}

#[derive(Clone, Deserialize, Copy, Debug, Ord, Eq, PartialOrd, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    I64,
    Bytes,
}

impl Workload {
    pub fn keyspaces(&self) -> &[Keyspace] {
        &self.keyspace
    }

    pub fn stores(&self) -> &[Store] {
        &self.stores
    }

    pub fn topics(&self) -> &[Topics] {
        &self.topics
    }

    pub fn oltp(&self) -> Option<&Oltp> {
        self.oltp.as_ref()
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn ratelimit(&self) -> &Ratelimit {
        &self.ratelimit
    }

    pub fn leaderboards(&self) -> &[LeaderboardsConfig] {
        &self.leaderboards
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    Uniform,
    Zipf,
}

impl Default for Distribution {
    fn default() -> Self {
        Self::Uniform
    }
}

// A linear ramp means that the ratelimit is increased between the start
// and end value by the step function in a sequence. A shuffled ramp means
// that the same stepwise ratelimits are explored in random order; however,
// only ratelimits at the specified steps are applied.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RampType {
    #[default]
    Linear,
    Shuffled,
}

// Once the ramp is completed, the workload can remain at the final stable
// state, it can loop around and repeat the entire workload in the same
// sequence, or can repeat the same workload in reverse order (for example,
// to perform a corresponding ramp-down to the initial ramp-up).
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RampCompletionAction {
    #[default]
    Stable,
    Loop,
    Mirror,
}

#[derive(Clone, Deserialize)]
pub struct Ratelimit {
    #[serde(default)]
    start: u64,

    #[serde(default)]
    end: Option<u64>,

    #[serde(default)]
    step: Option<u64>,

    #[serde(default)]
    interval: Option<u64>,

    #[serde(default)]
    ramp: RampType,

    #[serde(default)]
    on_ramp_completion: RampCompletionAction,
}

impl Ratelimit {
    pub fn start(&self) -> Option<NonZeroU64> {
        NonZeroU64::new(self.start)
    }

    pub fn end(&self) -> Option<u64> {
        self.end
    }

    pub fn step(&self) -> Option<u64> {
        self.step
    }

    pub fn interval(&self) -> Option<Duration> {
        self.interval.map(Duration::from_secs)
    }

    pub fn ramp_type(&self) -> RampType {
        self.ramp
    }

    pub fn ramp_completion_action(&self) -> RampCompletionAction {
        self.on_ramp_completion
    }

    pub fn is_dynamic(&self) -> bool {
        self.end.is_some() || self.step.is_some() || self.interval.is_some()
    }

    pub fn validate(&self) {
        if !self.is_dynamic() {
            return;
        }

        if !(self.end.is_some() && self.step.is_some() && self.interval.is_some()) {
            eprintln!("end, step, and interval need to be specified for dynamic ratelimit");
            std::process::exit(2);
        }

        if self.start > self.end.unwrap() || self.step.unwrap() > self.end.unwrap() {
            eprintln!("invalid configuration for dynamic workload ratelimiting");
            std::process::exit(2);
        }
    }
}
