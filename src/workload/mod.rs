use super::*;
use config::{Command, ValueKind, Verb};
use rand::distributions::{Alphanumeric, Uniform};
use rand::{Rng, RngCore, SeedableRng};
use rand_distr::Distribution as RandomDistribution;
use rand_distr::WeightedAliasIndex;
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use ratelimit::Ratelimiter;
use std::collections::{HashMap, HashSet};
use std::io::Result;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::runtime::Runtime;
use zipf::ZipfDistribution;

pub mod client;

mod distribution;
mod keyspace;
mod publisher;

pub use client::{ClientRequest, ClientWorkItem};
pub use publisher::PublisherWorkItem;

use distribution::Distribution;
use keyspace::Keyspace;

static SEQUENCE_NUMBER: AtomicU64 = AtomicU64::new(0);

pub fn launch_workload(
    generator: Generator,
    config: &Config,
    client_sender: Sender<ClientWorkItem>,
    pubsub_sender: Sender<PublisherWorkItem>,
) -> Runtime {
    debug!("Launching workload...");

    // spawn the request drivers on their own runtime
    let workload_rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .expect("failed to initialize tokio runtime");

    // initialize a PRNG with the default initial seed. We will then use this to
    // generate unique seeds for each workload thread.
    let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

    // spawn the request generators on a blocking threads
    for _ in 0..config.workload().threads() {
        let client_sender = client_sender.clone();
        let pubsub_sender = pubsub_sender.clone();
        let generator = generator.clone();

        // generate the seed for this workload thread
        let mut seed = [0; 64];
        rng.fill_bytes(&mut seed);

        workload_rt.spawn_blocking(move || {
            // since this seed is unique, each workload thread should produce
            // requests in a different sequence
            let mut rng = Xoshiro512PlusPlus::from_seed(Seed512(seed));

            while RUNNING.load(Ordering::Relaxed) {
                generator.generate(&client_sender, &pubsub_sender, &mut rng);
            }
        });
    }

    let c = config.clone();
    workload_rt.spawn_blocking(move || reconnect(client_sender, c));

    workload_rt
}

#[derive(Clone)]
pub struct Generator {
    ratelimiter: Option<Arc<Ratelimiter>>,
    components: Vec<Component>,
    component_dist: WeightedAliasIndex<usize>,
}

impl Generator {
    pub fn new(config: &Config) -> Self {
        let ratelimiter = config.workload().ratelimit().map(|rate| {
            let amount = (rate.get() as f64 / 1_000_000.0).ceil() as u64;

            // even though we might not have nanosecond level clock resolution,
            // by using a nanosecond level duration, we achieve more accurate
            // ratelimits.
            let interval = Duration::from_nanos(1_000_000_000 / (rate.get() / amount));

            let capacity = std::cmp::max(100, amount);

            Arc::new(
                Ratelimiter::builder(amount, interval)
                    .max_tokens(capacity)
                    .build()
                    .expect("failed to initialize ratelimiter"),
            )
        });

        let mut components = Vec::new();
        let mut component_weights = Vec::new();

        for keyspace in config.workload().keyspaces() {
            components.push(Component::Keyspace(Keyspace::new(config, keyspace)));
            component_weights.push(keyspace.weight());
        }

        for topics in config.workload().topics() {
            components.push(Component::Topics(Topics::new(config, topics)));
            component_weights.push(topics.weight());
        }

        if components.is_empty() {
            eprintln!("no workload components were specified in the config");
            std::process::exit(1);
        }

        Self {
            ratelimiter,
            components,
            component_dist: WeightedAliasIndex::new(component_weights).unwrap(),
        }
    }

    pub fn ratelimiter(&self) -> Option<Arc<Ratelimiter>> {
        self.ratelimiter.clone()
    }

    pub fn generate(
        &self,
        client_sender: &Sender<ClientWorkItem>,
        pubsub_sender: &Sender<PublisherWorkItem>,
        rng: &mut dyn RngCore,
    ) {
        if let Some(ref ratelimiter) = self.ratelimiter {
            loop {
                if ratelimiter.try_wait().is_ok() {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }

        match &self.components[self.component_dist.sample(rng)] {
            Component::Keyspace(keyspace) => {
                let _ = client_sender.send_blocking(keyspace.generate_request(rng));
            }
            Component::Topics(topics) => {
                let _ = pubsub_sender.send_blocking(self.generate_pubsub(topics, rng));
            }
        }
    }

    fn generate_pubsub(&self, topics: &Topics, rng: &mut dyn RngCore) -> PublisherWorkItem {
        let topic_index = topics.topic_dist.sample(rng);
        let topic = topics.topics[topic_index].clone();
        let partition = topics.partition_dist.sample(rng);

        let mut m = vec![0_u8; topics.message_len];
        // add a header
        m[0..8] = [0x54, 0x45, 0x53, 0x54, 0x49, 0x4E, 0x47, 0x21];
        rng.fill(&mut m[32..topics.message_len]);
        let mut k = vec![0_u8; topics.key_len];
        rng.fill(&mut k[0..topics.key_len]);

        PublisherWorkItem::Publish {
            topic,
            partition,
            key: k,
            message: m,
        }
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }
}

#[derive(Clone)]
pub enum Component {
    Keyspace(Keyspace),
    Topics(Topics),
}

#[derive(Clone)]
pub struct Topics {
    topics: Vec<Arc<String>>,
    partitions: usize,
    topic_dist: Distribution,
    partition_dist: Distribution,
    key_len: usize,
    message_len: usize,
    subscriber_poolsize: usize,
    subscriber_concurrency: usize,
}

impl Topics {
    pub fn new(config: &Config, topics: &config::Topics) -> Self {
        // ntopics must be >= 1
        let ntopics = std::cmp::max(1, topics.topics());
        // partitions must be >= 1
        let partitions = std::cmp::max(1, topics.partitions());
        let topiclen = topics.topic_len();
        let message_len = topics.message_len();
        // key_len must be >= 1
        let key_len = std::cmp::max(1, topics.key_len());
        let subscriber_poolsize = topics.subscriber_poolsize();
        let subscriber_concurrency = topics.subscriber_concurrency();
        let topic_dist = match topics.topic_distribution() {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, ntopics)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(ntopics, 1.0).unwrap())
            }
        };
        let partition_dist = match topics.partition_distribution() {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, partitions)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(partitions, 1.0).unwrap())
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
            topic_dist,
            partition_dist,
            key_len,
            message_len,
            subscriber_poolsize,
            subscriber_concurrency,
        }
    }

    pub fn topics(&self) -> &[Arc<String>] {
        &self.topics
    }

    pub fn partitions(&self) -> usize {
        self.partitions
    }

    pub fn subscriber_poolsize(&self) -> usize {
        self.subscriber_poolsize
    }

    pub fn subscriber_concurrency(&self) -> usize {
        self.subscriber_concurrency
    }
}

pub async fn reconnect(work_sender: Sender<ClientWorkItem>, config: Config) -> Result<()> {
    if config.client().is_none() {
        return Ok(());
    }

    let ratelimiter = config.client().unwrap().reconnect_rate().map(|rate| {
        let amount = (rate.get() as f64 / 1_000_000.0).ceil() as u64;

        // even though we might not have nanosecond level clock resolution,
        // by using a nanosecond level duration, we achieve more accurate
        // ratelimits.
        let interval = Duration::from_nanos(1_000_000_000 / (rate.get() / amount));

        let capacity = std::cmp::max(100, amount);

        Arc::new(
            Ratelimiter::builder(amount, interval)
                .max_tokens(capacity)
                .build()
                .expect("failed to initialize ratelimiter"),
        )
    });

    if ratelimiter.is_none() {
        return Ok(());
    }

    let ratelimiter = ratelimiter.unwrap();

    while RUNNING.load(Ordering::Relaxed) {
        match ratelimiter.try_wait() {
            Ok(_) => {
                let _ = work_sender.send(ClientWorkItem::Reconnect).await;
            }
            Err(d) => {
                std::thread::sleep(d);
            }
        }
    }

    Ok(())
}
