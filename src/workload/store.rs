use super::*;
use bytes::Bytes;
use std::sync::Arc;

#[derive(Debug, PartialEq)]
pub struct Ping {}

#[derive(Debug, PartialEq)]
pub struct Get {
    pub key: Arc<String>,
}

#[derive(Debug, PartialEq)]
pub struct Delete {
    pub key: Arc<String>,
}

#[derive(Debug, PartialEq)]
pub struct Put {
    /// For a Momento PUT request to a store, keys will always be
    /// a `String` type.
    pub key: Arc<String>,
    pub value: Bytes,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum StoreClientRequest {
    // Ping
    Ping(Ping),

    // Key-Value
    Get(Get),
    Delete(Delete),
    Put(Put),

    Reconnect,
}

#[derive(Clone)]
pub struct StoreWorkload {
    pub keys: Vec<Arc<[u8]>>,
    pub key_dist: Distribution,
    pub commands: Vec<StoreCommand>,
    pub command_dist: WeightedAliasIndex<usize>,
    pub vlen: usize,
    pub vkind: ValueKind,
    pub vbuf: Bytes,
}

impl StoreWorkload {
    pub fn new(config: &Config, store: &config::Store) -> Self {
        let value_random_bytes =
            estimate_random_bytes_needed(store.vlen().unwrap_or(0), store.compression_ratio());

        // nkeys must be >= 1
        let nkeys = std::cmp::max(1, store.nkeys());
        let klen = store.klen();

        // initialize a PRNG with the default initial seed
        let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

        // generate the seed for key PRNG
        let mut raw_seed = [0_u8; 64];
        rng.fill_bytes(&mut raw_seed);
        let key_seed = Seed512(raw_seed);

        // generate the seed for inner key PRNG
        let mut raw_seed = [0_u8; 64];
        rng.fill_bytes(&mut raw_seed);

        // we use a predictable seed to generate the keys in the store
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
        let key_dist = match store.key_distribution() {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, nkeys)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(nkeys, 1.0).unwrap())
            }
        };

        let mut commands = Vec::new();
        let mut command_weights = Vec::new();

        for command in store.commands() {
            commands.push(*command);
            command_weights.push(command.weight());

            // validate that the store is adaquately specified for the given
            // verb

            // commands that set generated values need a `vlen`
            if store.vlen().is_none()
                && store.vkind() == ValueKind::Bytes
                && matches!(command.verb(), StoreVerb::Put)
            {
                eprintln!(
                    "verb: {:?} requires that the keyspace has a `vlen` set when `vkind` is `bytes`",
                    command.verb()
                );
                std::process::exit(2);
            }
        }

        // prepare 100MB of random value
        let len = 100 * 1024 * 1024;
        let mut vbuf = BytesMut::zeroed(len);
        rng.fill_bytes(&mut vbuf[0..value_random_bytes]);
        vbuf.shuffle(&mut rng);

        let command_dist = WeightedAliasIndex::new(command_weights).unwrap();
        Self {
            keys,
            key_dist,
            commands,
            command_dist,
            vlen: store.vlen().unwrap_or(0),
            vkind: store.vkind(),
            vbuf: vbuf.into(),
        }
    }

    pub fn sample(&self, rng: &mut dyn RngCore) -> Arc<[u8]> {
        let index = self.key_dist.sample(rng);
        self.keys[index].clone()
    }

    pub fn sample_string(&self, rng: &mut dyn RngCore) -> Arc<String> {
        let keys = self.sample(rng);
        Arc::new(String::from_utf8_lossy(&keys).into_owned())
    }

    pub fn gen_value(&self, sequence: usize, rng: &mut dyn RngCore) -> Bytes {
        match self.vkind {
            ValueKind::I64 => format!("{}", rng.gen::<i64>()).into_bytes().into(),
            ValueKind::Bytes => {
                let start = sequence % (self.vbuf.len() - self.vlen);
                let end = start + self.vlen;

                self.vbuf.slice(start..end)
            }
        }
    }
}
