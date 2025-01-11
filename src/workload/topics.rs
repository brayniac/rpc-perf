use super::*;

#[derive(Clone)]
pub struct TopicsWorkload {
    pub topics: Vec<Arc<String>>,
    pub partitions: usize,
    pub replications: usize,
    pub topic_dist: Distribution,
    pub key_len: usize,
    pub message_len: usize,
    pub message_random_bytes: usize,
    pub subscriber_poolsize: usize,
    pub subscriber_concurrency: usize,
    pub kafka_single_subscriber_group: bool,
}

impl TopicsWorkload {
    pub fn new(config: &Config, topics: &config::Topics) -> Self {
        let message_random_bytes =
            estimate_random_bytes_needed(topics.message_len(), topics.compression_ratio());
        // ntopics, partitions, and replications must be >= 1
        let ntopics = std::cmp::max(1, topics.topics());
        let partitions = std::cmp::max(1, topics.partitions());
        let replications = std::cmp::max(1, topics.replications());
        let topiclen = topics.topic_len();
        let message_len = topics.message_len();
        let key_len = topics.key_len();
        let subscriber_poolsize = topics.subscriber_poolsize();
        let subscriber_concurrency = topics.subscriber_concurrency();
        let topic_dist = match topics.topic_distribution() {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, ntopics)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(ntopics, 1.0).unwrap())
            }
        };
        let topic_names: Vec<Arc<String>>;
        // if the given topic_names has the matched format, we use topic names there
        if topics
            .topic_names()
            .iter()
            .map(|n| n.len() == topiclen)
            .fold(topics.topic_names().len() == ntopics, |acc, c| acc && c)
        {
            topic_names = topics
                .topic_names()
                .iter()
                .map(|k| Arc::new((*k).clone()))
                .collect();
            debug!("Use given topic names:{:?}", topic_names);
        } else {
            // initialize topic name PRNG and generate a set of unique topics
            let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());
            let mut raw_seed = [0_u8; 64];
            rng.fill_bytes(&mut raw_seed);
            let topic_name_seed = Seed512(raw_seed);
            let mut rng = Xoshiro512PlusPlus::from_seed(topic_name_seed);
            let mut topics = HashSet::with_capacity(ntopics);
            while topics.len() < ntopics {
                let topic = (&mut rng)
                    .sample_iter(&Alphanumeric)
                    .take(topiclen)
                    .collect::<Vec<u8>>();
                let _ = topics.insert(unsafe { std::str::from_utf8_unchecked(&topic) }.to_string());
            }
            topic_names = topics.drain().map(|k| k.into()).collect();
        }

        Self {
            topics: topic_names,
            partitions,
            replications,
            topic_dist,
            key_len,
            message_len,
            message_random_bytes,
            subscriber_poolsize,
            subscriber_concurrency,
            kafka_single_subscriber_group: topics.kafka_single_subscriber_group(),
        }
    }

    pub fn topics(&self) -> &[Arc<String>] {
        &self.topics
    }

    pub fn partitions(&self) -> usize {
        self.partitions
    }

    pub fn replications(&self) -> usize {
        self.replications
    }

    pub fn subscriber_poolsize(&self) -> usize {
        self.subscriber_poolsize
    }

    pub fn subscriber_concurrency(&self) -> usize {
        self.subscriber_concurrency
    }

    pub fn kafka_single_subscriber_group(&self) -> bool {
        self.kafka_single_subscriber_group
    }
}
