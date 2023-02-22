// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rand::SeedableRng;
use rand::rngs::SmallRng;
use seg::{Seg, Policy};
use crate::metrics::*;
use crate::*;
use heatmap::Heatmap;
use ratelimit::Ratelimiter;
use std::sync::Arc;
use config_file::Verb;

pub struct Worker {
    seg: Seg,
    request_ratelimit: Option<Arc<Ratelimiter>>,
    request_heatmap: Option<Arc<Heatmap>>,
    request_waterfall: Option<Arc<Heatmap>>,
    rng: SmallRng,
    config: Arc<Config>,
}

impl Worker {
    pub fn new(config: Arc<Config>) -> Result<Self, std::io::Error> {
        let seg = seg::Builder::default()
            .hash_power(22)
            .overflow_factor(1.0)
            .heap_size(4 * 1024 * 1024 * 1024)
            .segment_size(1024 * 1024)
            .eviction(Policy::Merge { max: 8, merge: 4, compact: 2})
            .build().unwrap();

        // return the worker
        Ok(Worker {
            request_ratelimit: None,
            request_heatmap: None,
            request_waterfall: None,
            seg,
            rng: SmallRng::from_entropy(),
            config,
        })
    }

    /// Controls the request rate
    pub fn set_request_ratelimit(&mut self, ratelimiter: Option<Arc<Ratelimiter>>) {
        self.request_ratelimit = ratelimiter;
    }

    /// Provide a heatmap for recording request latency
    pub fn set_request_heatmap(&mut self, heatmap: Option<Arc<Heatmap>>) {
        self.request_heatmap = heatmap;
    }

    /// Provide a heatmap for recording request latencies into the waterfall
    pub fn set_request_waterfall(&mut self, heatmap: Option<Arc<Heatmap>>) {
        self.request_waterfall = heatmap;
    }

    /// Starts the worker event loop. Typically used in a child thread.
    pub fn run(&mut self) {
        let mut key: Vec<u8> = Vec::new();
        let mut value: Vec<u8> = Vec::new();
        loop {
            if let Some(r) = &self.request_ratelimit {
                r.wait();
            }

            let keyspace = self.config.choose_keyspace(&mut self.rng);
            let command = keyspace.choose_command(&mut self.rng);
            let (start, stop) = match command.verb() {
                Verb::Get => {
                    metrics::REQUEST_GET.increment();
                    metrics::RESPONSE.increment();
                    key = keyspace.generate_key(&mut self.rng, key);
                    let start = Instant::now();
                    if self.seg.get(&key).is_some() {
                        metrics::RESPONSE.increment();
                        RESPONSE_HIT.increment();
                    }
                    metrics::RESPONSE.increment();
                    (start, Instant::now())
                }
                Verb::Set => {
                    key = keyspace.generate_key(&mut self.rng, key);
                    value = keyspace.generate_value(&mut self.rng, value);
                    let ttl = keyspace.ttl();
                    let start = Instant::now();
                    if self.seg.insert(&key, &value, None, std::time::Duration::from_secs(ttl as u64)).is_err() {
                        RESPONSE_EX.increment();
                    }
                    RESPONSE.increment();
                    (start, Instant::now())
                }
                // Verb::Delete => Self::delete(&mut self.rng, keyspace, buf),
                _ => {
                    continue;
                }
            };

            if let Some(ref heatmap) = self.request_heatmap {
                let elapsed = (stop - start).as_nanos();
                heatmap.increment(stop, elapsed, 1);

                if let Some(ref waterfall) = self.request_waterfall {
                    waterfall.increment(stop, elapsed, 1);
                }
            }
        }
    }
}
