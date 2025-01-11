use super::*;

#[derive(Clone, Deserialize)]
pub struct LeaderboardClientConfig {
    /// The number of connections this process will have to each endpoint.
    poolsize: usize,
    /// The number of concurrent sessions per connection.
    #[serde(default)]
    concurrency: usize,
    /// Request timeout
    request_timeout: u64,
    // number of threads for client tasks
    threads: usize,
}

impl LeaderboardClientConfig {
    pub fn threads(&self) -> usize {
        std::cmp::max(1, self.threads)
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout)
    }

    pub fn poolsize(&self) -> usize {
        std::cmp::max(1, self.poolsize)
    }

    pub fn concurrency(&self) -> usize {
        std::cmp::max(1, self.concurrency)
    }
}
