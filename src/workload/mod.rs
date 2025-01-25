use super::*;
use crate::config::workload::LeaderboardCommand;
// use crate::config::workload::LeaderboardVerb;
use bytes::Bytes;
use bytes::BytesMut;
use config::{Command, RampCompletionAction, RampType, ValueKind, Verb};
use flate2::write::GzEncoder;
use flate2::Compression;
use rand::distributions::{Alphanumeric, Uniform};
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng, RngCore, SeedableRng};
use rand_distr::Distribution as RandomDistribution;
use rand_distr::WeightedAliasIndex;
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use ratelimit::Ratelimiter;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::io::{Result, Write};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::runtime::Runtime;
use zipf::ZipfDistribution;

pub mod client;
pub mod leaderboard;
mod oltp;
mod publisher;
mod topics;

pub use client::ClientRequest;
pub use leaderboard::{LeaderboardRequest, LeaderboardsWorkload};
pub use oltp::{Oltp, OltpRequest};
pub use publisher::PublisherWorkItem;

pub mod store;
pub use store::{StoreClientRequest, StoreWorkload};

pub use topics::TopicsWorkload;

pub mod cache;
pub use cache::CacheWorkload;

static SEQUENCE_NUMBER: AtomicU64 = AtomicU64::new(0);

// a multiplier for the ratelimiter token bucket capacity
static BUCKET_CAPACITY: u64 = 64;

pub fn launch_workload(
    generator: Generator,
    config: &Config,
    client_sender: Sender<ClientWorkItemKind<ClientRequest>>,
    pubsub_sender: Sender<PublisherWorkItem>,
    store_sender: Sender<ClientWorkItemKind<StoreClientRequest>>,
    oltp_sender: Sender<ClientWorkItemKind<OltpRequest>>,
    leaderboard_sender: Sender<ClientWorkItemKind<LeaderboardRequest>>,
) -> Runtime {
    debug!("Launching workload...");

    // spawn the request drivers on their own runtime
    let workload_rt = Builder::new_multi_thread()
        .enable_all()
        .thread_name("rpc-perf-gen")
        .worker_threads(config.workload().threads())
        .build()
        .expect("failed to initialize tokio runtime");

    // initialize a PRNG with the default initial seed. We will then use this to
    // generate unique seeds for each workload thread.
    let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

    // spawn the request generators on a blocking threads
    for _ in 0..config.workload().threads() {
        let client_sender = client_sender.clone();
        let pubsub_sender = pubsub_sender.clone();
        let store_sender = store_sender.clone();
        let oltp_sender = oltp_sender.clone();
        let generator = generator.clone();
        let leaderboard_sender = leaderboard_sender.clone();

        // generate the seed for this workload thread
        let mut seed = [0; 64];
        rng.fill_bytes(&mut seed);

        workload_rt.spawn(async move {
            // since this seed is unique, each workload thread should produce
            // requests in a different sequence
            let mut rng = Xoshiro512PlusPlus::from_seed(Seed512(seed));

            while RUNNING.load(Ordering::Relaxed) {
                generator
                    .generate(
                        &client_sender,
                        &pubsub_sender,
                        &store_sender,
                        &oltp_sender,
                        &leaderboard_sender,
                        &mut rng,
                    )
                    .await;
            }
        });
    }

    let c = config.clone();
    let store_c = config.clone();
    workload_rt.spawn_blocking(move || reconnect(client_sender, c));
    workload_rt.spawn_blocking(move || reconnect(store_sender, store_c));

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
        let ratelimiter = config.workload().ratelimit().start().map(|rate| {
            let rate = rate.get();
            let amount = (rate as f64 / 1_000_000.0).ceil() as u64;
            RATELIMIT_CURR.set(rate as i64);

            // even though we might not have nanosecond level clock resolution,
            // by using a nanosecond level duration, we achieve more accurate
            // ratelimits.
            let interval = Duration::from_nanos(1_000_000_000 / (rate / amount));

            Arc::new(
                Ratelimiter::builder(amount, interval)
                    .max_tokens(amount * BUCKET_CAPACITY)
                    .build()
                    .expect("failed to initialize ratelimiter"),
            )
        });

        let mut components = Vec::new();
        let mut component_weights = Vec::new();

        for keyspace in config.workload().keyspaces() {
            components.push(Component::Keyspace(CacheWorkload::new(config, keyspace)));
            component_weights.push(keyspace.weight());
        }

        for topics in config.workload().topics() {
            components.push(Component::Topics(TopicsWorkload::new(config, topics)));
            component_weights.push(topics.weight());
        }

        for store in config.workload().stores() {
            components.push(Component::Store(StoreWorkload::new(config, store)));
            component_weights.push(store.weight());
        }

        if let Some(oltp) = config.workload().oltp() {
            components.push(Component::Oltp(Oltp::new(config, oltp)));
            component_weights.push(oltp.weight());
        }

        for leaderboards in config.workload().leaderboards() {
            components.push(Component::Leaderboards(LeaderboardsWorkload::new(
                config,
                leaderboards,
            )));
            component_weights.push(leaderboards.weight);
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

    pub async fn generate(
        &self,
        client_sender: &Sender<ClientWorkItemKind<ClientRequest>>,
        pubsub_sender: &Sender<PublisherWorkItem>,
        store_sender: &Sender<ClientWorkItemKind<StoreClientRequest>>,
        oltp_sender: &Sender<ClientWorkItemKind<OltpRequest>>,
        leaderboard_sender: &Sender<ClientWorkItemKind<LeaderboardRequest>>,
        rng: &mut Xoshiro512PlusPlus,
    ) {
        if let Some(ref ratelimiter) = self.ratelimiter {
            loop {
                RATELIMIT_DROPPED.set(ratelimiter.dropped());

                if ratelimiter.try_wait().is_ok() {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }

        match &self.components[self.component_dist.sample(rng)] {
            Component::Keyspace(keyspace) => {
                if client_sender
                    .send(self.generate_request(keyspace, rng))
                    .await
                    .is_err()
                {
                    REQUEST_DROPPED.increment();
                }
            }
            Component::Topics(topics) => {
                if pubsub_sender
                    .send(self.generate_pubsub(topics, rng))
                    .await
                    .is_err()
                {
                    REQUEST_DROPPED.increment();
                }
            }
            Component::Store(store) => {
                if store_sender
                    .send(self.generate_store_request(store, rng))
                    .await
                    .is_err()
                {
                    REQUEST_DROPPED.increment();
                }
            }
            Component::Oltp(oltp) => {
                if oltp_sender
                    .send(self.generate_oltp_request(oltp, rng))
                    .await
                    .is_err()
                {
                    REQUEST_DROPPED.increment();
                }
            }
            Component::Leaderboards(leaderboards) => {
                if leaderboard_sender
                    .send(self.generate_leaderboard_request(leaderboards, rng))
                    .await
                    .is_err()
                {
                    REQUEST_DROPPED.increment();
                }
            }
        }
    }

    fn generate_pubsub(&self, topics: &TopicsWorkload, rng: &mut dyn RngCore) -> PublisherWorkItem {
        let topic_index = topics.topic_dist.sample(rng);
        let topic = topics.topics[topic_index].clone();

        let mut m = vec![0_u8; topics.message_len];

        // add a header
        [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]] =
            [0x54, 0x45, 0x53, 0x54, 0x49, 0x4E, 0x47, 0x21];

        // determine the range to fill with random bytes and fill that range
        let limit = std::cmp::min(topics.message_random_bytes + 32, m.len());
        rng.fill(&mut m[32..limit]);

        // generate the key
        if topics.key_len == 0 {
            PublisherWorkItem::Publish {
                topic,
                key: None,
                message: m,
            }
        } else {
            let mut k = vec![0_u8; topics.key_len];
            rng.fill(&mut k[0..topics.key_len]);
            PublisherWorkItem::Publish {
                topic,
                key: Some(k),
                message: m,
            }
        }
    }

    fn generate_store_request(
        &self,
        store: &StoreWorkload,
        rng: &mut dyn RngCore,
    ) -> ClientWorkItemKind<StoreClientRequest> {
        let command = &store.commands[store.command_dist.sample(rng)];
        let sequence = SEQUENCE_NUMBER.fetch_add(1, Ordering::Relaxed);

        let request = match command.verb() {
            StoreVerb::Put => StoreClientRequest::Put(store::Put {
                key: store.sample_string(rng),
                value: store.gen_value(sequence as _, rng),
            }),
            StoreVerb::Get => StoreClientRequest::Get(store::Get {
                key: store.sample_string(rng),
            }),
            StoreVerb::Delete => StoreClientRequest::Delete(store::Delete {
                key: store.sample_string(rng),
            }),
            StoreVerb::Ping => StoreClientRequest::Ping(store::Ping {}),
        };
        ClientWorkItemKind::Request { request, sequence }
    }

    fn generate_oltp_request(
        &self,
        oltp: &Oltp,
        rng: &mut dyn RngCore,
    ) -> ClientWorkItemKind<OltpRequest> {
        let id = rng.gen_range(1..=oltp.keys);
        let table = rng.gen_range(1..=oltp.tables) as i32;
        let sequence = SEQUENCE_NUMBER.fetch_add(1, Ordering::Relaxed);

        let request = OltpRequest::PointSelect(oltp::PointSelect {
            id,
            table: format!("sbtest{table}"),
        });

        ClientWorkItemKind::Request { request, sequence }
    }

    fn generate_leaderboard_request(
        &self,
        workload: &LeaderboardsWorkload,
        rng: &mut dyn RngCore,
    ) -> ClientWorkItemKind<LeaderboardRequest> {
        let leaderboard = workload.leaderboard_name(rng);
        let command = workload.command(rng);

        let sequence = SEQUENCE_NUMBER.fetch_add(1, Ordering::Relaxed);

        let request = match command {
            LeaderboardCommand::GetRank {
                cardinality,
                order,
                weight: _,
            } => LeaderboardRequest::GetRank {
                leaderboard,
                ids: workload.get_ids(std::cmp::max(1, cardinality), rng),
                order,
            },
            LeaderboardCommand::GetByRank {
                cardinality,
                order,
                weight: _,
            } => LeaderboardRequest::GetByRank {
                leaderboard,
                range: Some(0..(cardinality as u32)),
                order,
            },
            LeaderboardCommand::Upsert {
                cardinality,
                weight: _,
            } => LeaderboardRequest::Upsert {
                leaderboard,
                elements: workload.get_elements(std::cmp::max(1, cardinality), rng),
            },
            LeaderboardCommand::Delete { weight: _ } => LeaderboardRequest::Delete { leaderboard },
            LeaderboardCommand::GetByScore {
                cardinality,
                offset,
                order,
                weight: _,
            } => LeaderboardRequest::GetByScore {
                leaderboard,
                offset: offset as u32,
                limit: cardinality as u32,
                order,
            },
            LeaderboardCommand::Length { weight: _ } => LeaderboardRequest::Length { leaderboard },
            LeaderboardCommand::Remove {
                cardinality,
                weight: _,
            } => LeaderboardRequest::Remove {
                leaderboard,
                ids: workload.get_ids(std::cmp::max(1, cardinality), rng),
            },
        };

        ClientWorkItemKind::Request { request, sequence }
    }

    fn generate_request(
        &self,
        keyspace: &CacheWorkload,
        rng: &mut dyn RngCore,
    ) -> ClientWorkItemKind<ClientRequest> {
        let command = &keyspace.commands[keyspace.command_dist.sample(rng)];

        let sequence = SEQUENCE_NUMBER.fetch_add(1, Ordering::Relaxed);

        let request = match command.verb() {
            Verb::Add => ClientRequest::Add(client::Add {
                key: keyspace.sample(rng),
                value: keyspace.gen_value(sequence as _, rng),
                ttl: keyspace.ttl(),
            }),
            Verb::Get => ClientRequest::Get(client::Get {
                key: keyspace.sample(rng),
            }),
            Verb::Set => ClientRequest::Set(client::Set {
                key: keyspace.sample(rng),
                value: keyspace.gen_value(sequence as _, rng),
                ttl: keyspace.ttl(),
            }),
            Verb::Delete => ClientRequest::Delete(client::Delete {
                key: keyspace.sample(rng),
            }),
            Verb::Replace => ClientRequest::Replace(client::Replace {
                key: keyspace.sample(rng),
                value: keyspace.gen_value(sequence as _, rng),
                ttl: keyspace.ttl(),
            }),
            Verb::HashGet => {
                let cardinality = command.cardinality();
                let mut fields = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    fields.push(keyspace.sample_inner(rng));
                }

                ClientRequest::HashGet(client::HashGet {
                    key: keyspace.sample(rng),
                    fields,
                })
            }
            Verb::HashGetAll => ClientRequest::HashGetAll(client::HashGetAll {
                key: keyspace.sample(rng),
            }),
            Verb::HashDelete => {
                let cardinality = command.cardinality();
                let mut fields = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    fields.push(keyspace.sample_inner(rng));
                }

                ClientRequest::HashDelete(client::HashDelete {
                    key: keyspace.sample(rng),
                    fields,
                })
            }
            Verb::HashExists => ClientRequest::HashExists(client::HashExists {
                key: keyspace.sample(rng),
                field: keyspace.sample_inner(rng),
            }),
            Verb::HashIncrement => ClientRequest::HashIncrement(client::HashIncrement {
                key: keyspace.sample(rng),
                field: keyspace.sample_inner(rng),
                amount: rng.gen(),
                ttl: keyspace.ttl(),
            }),
            Verb::HashSet => {
                let mut data = HashMap::new();
                while data.len() < command.cardinality() {
                    data.insert(
                        keyspace.sample_inner(rng),
                        keyspace.gen_value(sequence as usize + data.len(), rng),
                    );
                }
                ClientRequest::HashSet(client::HashSet {
                    key: keyspace.sample(rng),
                    data,
                    ttl: keyspace.ttl(),
                })
            }
            Verb::ListPushFront => {
                let cardinality = command.cardinality();
                let mut elements = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    elements.push(keyspace.sample_inner(rng));
                }
                ClientRequest::ListPushFront(client::ListPushFront {
                    key: keyspace.sample(rng),
                    elements,
                    truncate: command.truncate(),
                    ttl: keyspace.ttl(),
                })
            }
            Verb::ListPushBack => {
                let cardinality = command.cardinality();
                let mut elements = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    elements.push(keyspace.sample_inner(rng));
                }
                ClientRequest::ListPushBack(client::ListPushBack {
                    key: keyspace.sample(rng),
                    elements,
                    truncate: command.truncate(),
                    ttl: keyspace.ttl(),
                })
            }
            Verb::ListFetch => ClientRequest::ListFetch(client::ListFetch {
                key: keyspace.sample(rng),
            }),
            Verb::ListLength => ClientRequest::ListLength(client::ListLength {
                key: keyspace.sample(rng),
            }),
            Verb::ListPopFront => ClientRequest::ListPopFront(client::ListPopFront {
                key: keyspace.sample(rng),
            }),
            Verb::ListPopBack => ClientRequest::ListPopBack(client::ListPopBack {
                key: keyspace.sample(rng),
            }),
            Verb::ListRemove => ClientRequest::ListRemove(client::ListRemove {
                key: keyspace.sample(rng),
                element: keyspace.sample_inner(rng),
            }),
            Verb::Ping => ClientRequest::Ping(client::Ping {}),
            Verb::SetAdd => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(keyspace.sample_inner(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SetAdd(client::SetAdd {
                    key: keyspace.sample(rng),
                    members,
                    ttl: keyspace.ttl(),
                })
            }
            Verb::SetMembers => ClientRequest::SetMembers(client::SetMembers {
                key: keyspace.sample(rng),
            }),
            Verb::SetRemove => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(keyspace.sample_inner(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SetRemove(client::SetRemove {
                    key: keyspace.sample(rng),
                    members,
                })
            }
            Verb::SortedSetAdd => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(keyspace.sample_inner(rng));
                }
                let members = members.drain().map(|m| (m, rng.gen())).collect();
                ClientRequest::SortedSetAdd(client::SortedSetAdd {
                    key: keyspace.sample(rng),
                    members,
                    ttl: keyspace.ttl(),
                })
            }
            Verb::SortedSetRange => ClientRequest::SortedSetRange(client::SortedSetRange {
                key: keyspace.sample(rng),
                start: command.start(),
                end: command.end(),
                by_score: command.by_score(),
            }),
            Verb::SortedSetRemove => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(keyspace.sample_inner(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SortedSetRemove(client::SortedSetRemove {
                    key: keyspace.sample(rng),
                    members,
                })
            }
            Verb::SortedSetIncrement => {
                ClientRequest::SortedSetIncrement(client::SortedSetIncrement {
                    key: keyspace.sample(rng),
                    member: keyspace.sample_inner(rng),
                    amount: rng.gen(),
                    ttl: keyspace.ttl(),
                })
            }
            Verb::SortedSetScore => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(keyspace.sample_inner(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SortedSetScore(client::SortedSetScore {
                    key: keyspace.sample(rng),
                    members,
                })
            }
            Verb::SortedSetRank => ClientRequest::SortedSetRank(client::SortedSetRank {
                key: keyspace.sample(rng),
                member: keyspace.sample_inner(rng),
            }),
        };

        ClientWorkItemKind::Request { request, sequence }
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }
}

#[derive(Clone)]
pub enum Component {
    Keyspace(CacheWorkload),
    Topics(TopicsWorkload),
    Store(StoreWorkload),
    Oltp(Oltp),
    Leaderboards(LeaderboardsWorkload),
}

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

#[derive(Debug, PartialEq)]
pub enum ClientWorkItemKind<T> {
    Reconnect,
    Request { request: T, sequence: u64 },
}

pub async fn reconnect<TRequestKind>(
    work_sender: Sender<ClientWorkItemKind<TRequestKind>>,
    config: Config,
) -> Result<()> {
    if config.client().is_none() {
        return Ok(());
    }

    let ratelimiter = config.client().unwrap().reconnect_rate().map(|rate| {
        let rate = rate.get();
        let amount = (rate as f64 / 1_000_000.0).ceil() as u64;
        RATELIMIT_CURR.set(rate as i64);

        // even though we might not have nanosecond level clock resolution,
        // by using a nanosecond level duration, we achieve more accurate
        // ratelimits.
        let interval = Duration::from_nanos(1_000_000_000 / (rate / amount));

        Arc::new(
            Ratelimiter::builder(amount, interval)
                .max_tokens(amount * BUCKET_CAPACITY)
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
                let _ = work_sender.send(ClientWorkItemKind::Reconnect).await;
            }
            Err(d) => {
                std::thread::sleep(d);
            }
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct Ratelimit {
    limits: Vec<u64>,
    interval: Duration,
    ramp_completion_action: RampCompletionAction,
    current_idx: usize,
}

impl Ratelimit {
    pub fn new(config: &Config) -> Option<Self> {
        let ratelimit_config = config.workload().ratelimit();

        if !ratelimit_config.is_dynamic() {
            return None;
        }

        // Unwrapping values is safe since the structure has already been
        // validated for dynamic ratelimit parameters
        let start: u64 = ratelimit_config.start().unwrap().into();
        let end = ratelimit_config.end().unwrap();
        let step = ratelimit_config.step().unwrap() as usize;
        let interval = ratelimit_config.interval().unwrap();
        let ramp_type = ratelimit_config.ramp_type();
        let ramp_completion_action = ratelimit_config.ramp_completion_action();

        // Store all the ratelimits to test in a vector
        let nsteps = ((end - start) as usize / step) + 1;
        let mut limits: Vec<u64> = Vec::with_capacity(nsteps);
        for i in (start..end + 1).step_by(step) {
            limits.push(i);
        }

        // Shuffle the order of ratelimits if specified
        if ramp_type == RampType::Shuffled {
            limits.shuffle(&mut thread_rng());
        }

        // If the test is to be mirrored, store the ratelimits in reverse
        // order in the vector as well
        if ramp_completion_action == RampCompletionAction::Mirror {
            for i in (0..limits.len()).rev() {
                limits.push(limits[i]);
            }
        }

        Some(Ratelimit {
            limits,
            interval,
            ramp_completion_action,
            current_idx: 0,
        })
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn next_ratelimit(&mut self) -> u64 {
        let limit = self.limits[self.current_idx];
        self.current_idx += 1;

        if self.current_idx == self.limits.len() {
            // If the test is to be looped or mirrored reset the pointer to the
            // beginning of the vector and start again, else move back to the
            // previous (final stable) value
            if self.ramp_completion_action == RampCompletionAction::Loop
                || self.ramp_completion_action == RampCompletionAction::Mirror
            {
                self.current_idx = 0;
            } else {
                self.current_idx -= 1;
            }
        }

        limit
    }
}

fn estimate_random_bytes_needed(length: usize, compression_ratio: f64) -> usize {
    // if compression ratio is low, all bytes should be random
    if compression_ratio <= 1.0 {
        return length;
    }

    // we need to approximate the number of random bytes to send, we do
    // this iteratively assuming gzip compression.

    // doesn't matter what seed we use here
    let mut rng = Xoshiro512PlusPlus::seed_from_u64(0);

    // message buffer
    let mut m = vec![0; length];

    let mut best = 0;

    for idx in 0..m.len() {
        // zero all bytes
        for b in &mut m {
            *b = 0
        }

        // fill first N bytes with pseudorandom data
        rng.fill_bytes(&mut m[0..idx]);

        // shuffle the value
        m.shuffle(&mut rng);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        let _ = encoder.write_all(&m);
        let compressed = encoder.finish().unwrap();

        let ratio = m.len() as f64 / compressed.len() as f64;

        if ratio < compression_ratio {
            break;
        }

        best = idx;
    }

    best
}
