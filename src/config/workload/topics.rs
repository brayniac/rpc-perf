use super::*;

#[derive(Clone, Deserialize)]
pub struct Topics {
    topics: usize,
    #[serde(default = "one")]
    partitions: usize,
    #[serde(default = "one")]
    replications: usize,
    topic_len: usize,
    #[serde(default)]
    topic_names: Vec<String>,
    message_len: usize,
    #[serde(default)]
    compression_ratio: Option<f64>,
    #[serde(default = "one")]
    key_len: usize,
    weight: usize,
    subscriber_poolsize: usize,
    #[serde(default = "one")]
    subscriber_concurrency: usize,
    #[serde(default)]
    topic_distribution: Distribution,
    #[serde(default)]
    kafka_single_subscriber_group: bool,
}

impl Topics {
    pub fn weight(&self) -> usize {
        self.weight
    }

    pub fn partitions(&self) -> usize {
        self.partitions
    }

    pub fn replications(&self) -> usize {
        self.replications
    }

    pub fn topics(&self) -> usize {
        self.topics
    }

    pub fn topic_names(&self) -> &[String] {
        &self.topic_names
    }

    pub fn topic_len(&self) -> usize {
        self.topic_len
    }

    pub fn key_len(&self) -> usize {
        self.key_len
    }

    pub fn message_len(&self) -> usize {
        self.message_len
    }

    pub fn compression_ratio(&self) -> f64 {
        self.compression_ratio.unwrap_or(1.0)
    }

    pub fn subscriber_poolsize(&self) -> usize {
        self.subscriber_poolsize
    }

    pub fn subscriber_concurrency(&self) -> usize {
        self.subscriber_concurrency
    }

    pub fn topic_distribution(&self) -> Distribution {
        self.topic_distribution
    }

    pub fn kafka_single_subscriber_group(&self) -> bool {
        self.kafka_single_subscriber_group
    }
}
