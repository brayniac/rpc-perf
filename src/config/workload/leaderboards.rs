use super::*;

use momento::leaderboard::messages::data::Order;

#[derive(Clone, Deserialize)]
pub struct LeaderboardsConfig {
    #[serde(default)]
    pub leaderboards: usize,
    #[serde(default)]
    pub name_length: usize,
    #[serde(default)]
    pub distribution: Distribution,
    #[serde(default = "one")]
    pub weight: usize,
    pub commands: Vec<LeaderboardCommand>,
    #[serde(default = "one")]
    pub ids: usize,
    #[serde(default)]
    pub id_distribution: Distribution,
}

#[derive(Clone, Copy, Deserialize)]
pub struct LeaderboardCommand {
    verb: LeaderboardVerb,
    #[serde(default = "one")]
    weight: usize,
    #[serde(default)]
    order: Option<LeaderboardOrder>,
    cardinality: Option<usize>,
}

impl LeaderboardCommand {
    pub fn verb(&self) -> LeaderboardVerb {
        self.verb
    }

    pub fn weight(&self) -> usize {
        self.weight
    }

    pub fn order(&self) -> Option<LeaderboardOrder> {
        self.order
    }

    pub fn cardinality(&self) -> Option<usize> {
        self.cardinality
    }
}

#[derive(Clone, Deserialize, Copy, Debug, Ord, Eq, PartialOrd, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardVerb {
    GetRank,
    Upsert,
}

#[derive(Clone, Deserialize, Copy, Debug, Ord, Eq, PartialOrd, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardOrder {
    Ascending,
    Descending,
}

impl From<LeaderboardOrder> for Order {
    fn from(val: LeaderboardOrder) -> Self {
        match val {
            LeaderboardOrder::Ascending => Order::Ascending,
            LeaderboardOrder::Descending => Order::Descending,
        }
    }
}
