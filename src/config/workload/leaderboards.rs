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
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum LeaderboardCommand {
    #[serde(alias = "delete_leaderboard")]
    Delete {
        #[serde(default = "one")]
        weight: usize,
    },

    GetRank {
        #[serde(default = "one")]
        #[serde(alias = "ids")]
        cardinality: usize,

        #[serde(default)]
        order: LeaderboardOrder,

        #[serde(default = "one")]
        weight: usize,
    },

    GetByRank {
        #[serde(default = "one")]
        #[serde(alias = "limit")]
        cardinality: usize,

        #[serde(default)]
        order: LeaderboardOrder,

        #[serde(default = "one")]
        weight: usize,
    },

    GetByScore {
        #[serde(default = "one")]
        #[serde(alias = "limit")]
        cardinality: usize,

        #[serde(default)]
        offset: usize,

        #[serde(default)]
        order: LeaderboardOrder,

        #[serde(default = "one")]
        weight: usize,
    },

    #[serde(alias = "get_leaderboard_length")]
    Length {
        #[serde(default = "one")]
        weight: usize,
    },

    #[serde(alias = "remove_elements")]
    Remove {
        #[serde(default = "one")]
        #[serde(alias = "elements")]
        cardinality: usize,

        #[serde(default = "one")]
        weight: usize,
    },

    #[serde(alias = "upsert_elements")]
    Upsert {
        #[serde(default = "one")]
        #[serde(alias = "elements")]
        cardinality: usize,

        #[serde(default = "one")]
        weight: usize,
    },
}

impl Command for LeaderboardCommand {
    fn weight(&self) -> usize {
        match self {
            Self::Delete { weight, .. } => *weight,
            Self::GetRank { weight, .. } => *weight,
            Self::GetByRank { weight, .. } => *weight,
            Self::GetByScore { weight, .. } => *weight,
            Self::Length { weight, .. } => *weight,
            Self::Remove { weight, .. } => *weight,
            Self::Upsert { weight, .. } => *weight,
        }
    }
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

impl Default for LeaderboardOrder {
    fn default() -> Self {
        Self::Ascending
    }
}
