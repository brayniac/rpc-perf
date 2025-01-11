use super::*;
use crate::config::workload::LeaderboardOrder;

use crate::config;
use crate::config::workload::LeaderboardCommand;
use crate::workload::WeightedAliasIndex;
use crate::Arc;
use crate::Config;

#[derive(Clone)]
pub struct LeaderboardsWorkload {
    pub leaderboards: Vec<Arc<String>>,
    pub leaderboard_dist: Distribution,
    pub commands: Vec<LeaderboardCommand>,
    pub command_dist: WeightedAliasIndex<usize>,
    pub ids: Vec<u32>,
    pub id_dist: Distribution,
}

impl LeaderboardsWorkload {
    pub fn new(config: &Config, workload: &config::workload::LeaderboardsConfig) -> Self {
        let n_leaderboards = std::cmp::max(1, workload.leaderboards);
        let name_len = std::cmp::max(
            std::cmp::max(1, workload.name_length),
            (n_leaderboards as f64).log(62.0).ceil() as usize,
        );
        let n_ids = std::cmp::max(1, workload.ids);

        // initialize a PRNG with the default initial seed
        let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

        // generate the seed for key PRNG
        let mut raw_seed = [0_u8; 64];
        rng.fill_bytes(&mut raw_seed);
        let name_seed = Seed512(raw_seed);

        // we use a predictable seed to generate the keys in the store
        let mut rng = Xoshiro512PlusPlus::from_seed(name_seed);
        let mut leaderboards = HashSet::with_capacity(n_leaderboards);
        while leaderboards.len() < n_leaderboards {
            let l = (&mut rng)
                .sample_iter(&Alphanumeric)
                .take(name_len)
                .collect::<Vec<u8>>();
            let _ = leaderboards.insert(l);
        }
        let leaderboards: Vec<Arc<String>> = leaderboards
            .into_iter()
            .map(|k| Arc::new(std::str::from_utf8(&k).unwrap().into()))
            .collect();
        let leaderboard_dist = match workload.distribution {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, n_leaderboards)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(n_leaderboards, 1.0).unwrap())
            }
        };

        let mut commands = Vec::new();
        let mut command_weights = Vec::new();

        for command in &workload.commands {
            commands.push(*command);
            command_weights.push(command.weight());

            // validate that the workload is adaquately specified for the given
            // verb
        }

        let command_dist = WeightedAliasIndex::new(command_weights).unwrap();

        let mut ids = HashSet::with_capacity(n_ids);
        while ids.len() < n_ids {
            ids.insert(rng.gen());
        }
        let ids = ids.into_iter().collect();

        let id_dist = match workload.id_distribution {
            config::Distribution::Uniform => Distribution::Uniform(Uniform::new(0, n_ids)),
            config::Distribution::Zipf => {
                Distribution::Zipf(ZipfDistribution::new(n_ids, 1.0).unwrap())
            }
        };

        Self {
            leaderboards,
            leaderboard_dist,
            commands,
            command_dist,
            ids,
            id_dist,
        }
    }

    pub fn leaderboard_name(&self, rng: &mut dyn RngCore) -> String {
        let index = self.leaderboard_dist.sample(rng);
        self.leaderboards[index].to_string()
    }

    pub fn command(&self, rng: &mut dyn RngCore) -> LeaderboardCommand {
        let index = self.command_dist.sample(rng);
        self.commands[index]
    }

    pub fn get_ids(&self, count: usize, rng: &mut dyn RngCore) -> Vec<u32> {
        let mut ids = HashSet::new();

        while ids.len() < count {
            ids.insert(self.ids[self.id_dist.sample(rng)]);
        }

        ids.into_iter().collect()
    }

    pub fn get_elements(&self, count: usize, rng: &mut dyn RngCore) -> Vec<(u32, f64)> {
        let mut elements = HashMap::new();

        while elements.len() < count {
            elements.insert(self.ids[self.id_dist.sample(rng)], rng.gen());
        }

        elements.into_iter().collect()
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum LeaderboardRequest {
    GetRank {
        leaderboard: String,
        ids: Vec<u32>,
        order: LeaderboardOrder,
    },
    Upsert {
        leaderboard: String,
        elements: Vec<(u32, f64)>,
    },
}
