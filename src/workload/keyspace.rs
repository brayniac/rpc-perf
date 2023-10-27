use super::*;

/// A keyspace consists of a set of keys and the commands that will be used
/// within that keyspace.
///
/// On initialization, a set of random keys will be generated using a PRNG, the
/// keys generated will depend on the value of the `initial_seed` in the config
/// file. The same initialization procedure is followed for inner keys.
#[derive(Clone)]
pub struct Keyspace {
    keys: Vec<Arc<[u8]>>,
    key_dist: Distribution,
    commands: Vec<Command>,
    command_dist: WeightedAliasIndex<usize>,
    inner_keys: Vec<Arc<[u8]>>,
    inner_key_dist: Distribution,
    vlen: usize,
    vkind: ValueKind,
    ttl: Option<Duration>,
}

impl Keyspace {
    pub fn new(config: &Config, keyspace: &config::Keyspace) -> Self {
        // nkeys must be >= 1
        let nkeys = std::cmp::max(1, keyspace.nkeys());
        let klen = keyspace.klen();

        // initialize a PRNG with the default initial seed
        let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

        // generate the seed for key PRNG
        let mut raw_seed = [0_u8; 64];
        rng.fill_bytes(&mut raw_seed);
        let key_seed = Seed512(raw_seed);

        // generate the seed for inner key PRNG
        let mut raw_seed = [0_u8; 64];
        rng.fill_bytes(&mut raw_seed);
        let inner_key_seed = Seed512(raw_seed);

        // we use a predictable seed to generate the keys in the keyspace
        let mut rng = Xoshiro512PlusPlus::from_seed(key_seed);
        let mut keys = HashSet::with_capacity(nkeys);
        while keys.len() < nkeys {
            let key = (&mut rng)
                .sample_iter(&Alphanumeric)
                .take(klen)
                .collect::<Vec<u8>>();
            let _ = keys.insert(key);
        }
        let keys = keys.drain().map(|k| k.into()).collect();
        let key_dist = match keyspace.key_distribution() {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, nkeys)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(nkeys, 1.0).unwrap())
            }
        };

        let nkeys = keyspace.inner_keys_nkeys().unwrap_or(1);
        let klen = keyspace.inner_keys_klen().unwrap_or(1);

        // we use a predictable seed to generate the keys in the keyspace
        let mut rng = Xoshiro512PlusPlus::from_seed(inner_key_seed);
        let mut inner_keys = HashSet::with_capacity(nkeys);
        while inner_keys.len() < nkeys {
            let key = (&mut rng)
                .sample_iter(&Alphanumeric)
                .take(klen)
                .collect::<Vec<u8>>();
            let _ = inner_keys.insert(key);
        }
        let inner_keys: Vec<Arc<[u8]>> = inner_keys.drain().map(|k| k.into()).collect();
        let inner_key_dist = Distribution::Uniform(Uniform::new(0, nkeys));

        let mut commands = Vec::new();
        let mut command_weights = Vec::new();

        for command in keyspace.commands() {
            commands.push(*command);
            command_weights.push(command.weight());

            // validate that the keyspace is adaquately specified for the given
            // verb

            // commands that set generated values need a `vlen`
            if keyspace.vlen().is_none()
                && keyspace.vkind() == ValueKind::Bytes
                && matches!(command.verb(), Verb::Set | Verb::HashSet)
            {
                eprintln!(
                    "verb: {:?} requires that the keyspace has a `vlen` set when `vkind` is `bytes`",
                    command.verb()
                );
                std::process::exit(2);
            }

            // cardinality must always be > 0
            if command.cardinality() == 0 {
                eprintln!("cardinality must not be zero",);
                std::process::exit(2);
            }

            // not all commands support cardinality > 1
            if command.cardinality() > 1 && !command.verb().supports_cardinality() {
                eprintln!(
                    "verb: {:?} requires that `cardinality` is set to `1`",
                    command.verb()
                );
                std::process::exit(2);
            }

            if command.start().is_some() && !command.verb().supports_start() {
                eprintln!(
                    "verb: {:?} does not support the `start` argument",
                    command.verb()
                );
            }

            if command.end().is_some() && !command.verb().supports_end() {
                eprintln!(
                    "verb: {:?} does not support the `end` argument",
                    command.verb()
                );
            }

            if command.by_score() && !command.verb().supports_by_score() {
                eprintln!(
                    "verb: {:?} does not support the `by_score` option",
                    command.verb()
                );
            }

            if command.truncate().is_some() {
                // truncate must be >= 1
                if command.truncate().unwrap() == 0 {
                    eprintln!("truncate must be >= 1",);
                    std::process::exit(2);
                }

                // not all commands support truncate
                if !command.verb().supports_truncate() {
                    eprintln!("verb: {:?} does not support truncate", command.verb());
                    std::process::exit(2);
                }
            }

            if command.verb().needs_inner_key()
                && (keyspace.inner_keys_nkeys().is_none() || keyspace.inner_keys_klen().is_none())
            {
                eprintln!(
                    "verb: {:?} requires that `inner_key_klen` and `inner_key_nkeys` are set",
                    command.verb()
                );
                std::process::exit(2);
            }
        }

        let command_dist = WeightedAliasIndex::new(command_weights).unwrap();

        Self {
            keys,
            key_dist,
            commands,
            command_dist,
            inner_keys,
            inner_key_dist,
            vlen: keyspace.vlen().unwrap_or(0),
            vkind: keyspace.vkind(),
            ttl: keyspace.ttl(),
        }
    }

    fn key(&self, rng: &mut dyn RngCore) -> Arc<[u8]> {
        let index = self.key_dist.sample(rng);
        self.keys[index].clone()
    }

    fn inner_key(&self, rng: &mut dyn RngCore) -> Arc<[u8]> {
        let index = self.inner_key_dist.sample(rng);
        self.inner_keys[index].clone()
    }

    fn gen_value(&self, rng: &mut dyn RngCore) -> Vec<u8> {
        match self.vkind {
            ValueKind::I64 => format!("{}", rng.gen::<i64>()).into_bytes(),
            ValueKind::Bytes => {
                let mut buf = vec![0_u8; self.vlen];
                rng.fill(&mut buf[0..self.vlen]);
                buf
            }
        }
    }

    fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    pub fn generate_request(&self, rng: &mut dyn RngCore) -> ClientWorkItem {
        let command = self.commands[self.command_dist.sample(rng)];

        let request = match command.verb() {
            Verb::Add => ClientRequest::Add(client::Add {
                key: self.key(rng),
                value: self.gen_value(rng),
                ttl: self.ttl(),
            }),
            Verb::Get => ClientRequest::Get(client::Get {
                key: self.key(rng),
            }),
            Verb::Set => ClientRequest::Set(client::Set {
                key: self.key(rng),
                value: self.gen_value(rng),
                ttl: self.ttl(),
            }),
            Verb::Delete => ClientRequest::Delete(client::Delete {
                key: self.key(rng),
            }),
            Verb::Replace => ClientRequest::Replace(client::Replace {
                key: self.key(rng),
                value: self.gen_value(rng),
                ttl: self.ttl(),
            }),
            Verb::HashGet => {
                let cardinality = command.cardinality();
                let mut fields = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    fields.push(self.inner_key(rng));
                }

                ClientRequest::HashGet(client::HashGet {
                    key: self.key(rng),
                    fields,
                })
            }
            Verb::HashGetAll => ClientRequest::HashGetAll(client::HashGetAll {
                key: self.key(rng),
            }),
            Verb::HashDelete => {
                let cardinality = command.cardinality();
                let mut fields = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    fields.push(self.inner_key(rng));
                }

                ClientRequest::HashDelete(client::HashDelete {
                    key: self.key(rng),
                    fields,
                })
            }
            Verb::HashExists => ClientRequest::HashExists(client::HashExists {
                key: self.key(rng),
                field: self.inner_key(rng),
            }),
            Verb::HashIncrement => ClientRequest::HashIncrement(client::HashIncrement {
                key: self.key(rng),
                field: self.inner_key(rng),
                amount: rng.gen(),
                ttl: self.ttl(),
            }),
            Verb::HashSet => {
                let mut data = HashMap::new();
                while data.len() < command.cardinality() {
                    data.insert(self.inner_key(rng), self.gen_value(rng));
                }
                ClientRequest::HashSet(client::HashSet {
                    key: self.key(rng),
                    data,
                    ttl: self.ttl(),
                })
            }
            Verb::ListPushFront => {
                let cardinality = command.cardinality();
                let mut elements = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    elements.push(self.inner_key(rng));
                }
                ClientRequest::ListPushFront(client::ListPushFront {
                    key: self.key(rng),
                    elements,
                    truncate: command.truncate(),
                    ttl: self.ttl(),
                })
            }
            Verb::ListPushBack => {
                let cardinality = command.cardinality();
                let mut elements = Vec::with_capacity(cardinality);
                for _ in 0..cardinality {
                    elements.push(self.inner_key(rng));
                }
                ClientRequest::ListPushBack(client::ListPushBack {
                    key: self.key(rng),
                    elements,
                    truncate: command.truncate(),
                    ttl: self.ttl(),
                })
            }
            Verb::ListFetch => ClientRequest::ListFetch(client::ListFetch {
                key: self.key(rng),
            }),
            Verb::ListLength => ClientRequest::ListLength(client::ListLength {
                key: self.key(rng),
            }),
            Verb::ListPopFront => ClientRequest::ListPopFront(client::ListPopFront {
                key: self.key(rng),
            }),
            Verb::ListPopBack => ClientRequest::ListPopBack(client::ListPopBack {
                key: self.key(rng),
            }),
            Verb::Ping => ClientRequest::Ping(client::Ping {}),
            Verb::SetAdd => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(self.inner_key(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SetAdd(client::SetAdd {
                    key: self.key(rng),
                    members,
                    ttl: self.ttl(),
                })
            }
            Verb::SetMembers => ClientRequest::SetMembers(client::SetMembers {
                key: self.key(rng),
            }),
            Verb::SetRemove => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(self.inner_key(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SetRemove(client::SetRemove {
                    key: self.key(rng),
                    members,
                })
            }
            Verb::SortedSetAdd => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(self.inner_key(rng));
                }
                let members = members.drain().map(|m| (m, rng.gen())).collect();
                ClientRequest::SortedSetAdd(client::SortedSetAdd {
                    key: self.key(rng),
                    members,
                    ttl: self.ttl(),
                })
            }
            Verb::SortedSetRange => ClientRequest::SortedSetRange(client::SortedSetRange {
                key: self.key(rng),
                start: command.start(),
                end: command.end(),
                by_score: command.by_score(),
            }),
            Verb::SortedSetRemove => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(self.inner_key(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SortedSetRemove(client::SortedSetRemove {
                    key: self.key(rng),
                    members,
                })
            }
            Verb::SortedSetIncrement => {
                ClientRequest::SortedSetIncrement(client::SortedSetIncrement {
                    key: self.key(rng),
                    member: self.inner_key(rng),
                    amount: rng.gen(),
                    ttl: self.ttl(),
                })
            }
            Verb::SortedSetScore => {
                let mut members = HashSet::new();
                while members.len() < command.cardinality() {
                    members.insert(self.inner_key(rng));
                }
                let members = members.drain().collect();
                ClientRequest::SortedSetScore(client::SortedSetScore {
                    key: self.key(rng),
                    members,
                })
            }
            Verb::SortedSetRank => ClientRequest::SortedSetRank(client::SortedSetRank {
                key: self.key(rng),
                member: self.inner_key(rng),
            }),
        };

        ClientWorkItem::Request {
            request,
            sequence: SEQUENCE_NUMBER.fetch_add(1, Ordering::Relaxed),
        }
    }
}