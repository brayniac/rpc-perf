use super::*;

#[derive(Clone, Deserialize)]
pub struct Store {
    #[serde(default)]
    nkeys: usize,
    #[serde(default)]
    klen: usize,
    #[serde(default)]
    key_distribution: Distribution,
    #[serde(default = "one")]
    weight: usize,
    commands: Vec<StoreCommand>,
    #[serde(default)]
    vlen: Option<usize>,
    #[serde(default)]
    vkind: Option<ValueKind>,
    #[serde(default)]
    compression_ratio: Option<f64>,
}

impl Store {
    pub fn nkeys(&self) -> usize {
        self.nkeys
    }

    pub fn klen(&self) -> usize {
        self.klen
    }

    pub fn key_distribution(&self) -> Distribution {
        self.key_distribution
    }

    pub fn weight(&self) -> usize {
        self.weight
    }

    pub fn commands(&self) -> &[StoreCommand] {
        &self.commands
    }

    pub fn vlen(&self) -> Option<usize> {
        self.vlen
    }

    pub fn vkind(&self) -> ValueKind {
        self.vkind.unwrap_or(ValueKind::Bytes)
    }

    pub fn compression_ratio(&self) -> f64 {
        self.compression_ratio.unwrap_or(1.0)
    }
}

#[derive(Clone, Copy, Deserialize)]
pub struct StoreCommand {
    verb: StoreVerb,
    #[serde(default = "one")]
    weight: usize,
}

impl StoreCommand {
    pub fn verb(&self) -> StoreVerb {
        self.verb
    }

    pub fn weight(&self) -> usize {
        self.weight
    }
}

#[derive(Clone, Deserialize, Copy, Debug, Ord, Eq, PartialOrd, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StoreVerb {
    /// Sends a `PING` to the server and expects a `PONG`
    /// * Ping: `PING`
    /// * RESP: `PING`
    Ping,

    /*
     * KEY-VALUE
     */
    /// Read the value for one a key.
    /// * Momento: `get`
    Get,
    /// Put the value for a key.
    /// * Momento: `put`
    Put,
    /// Remove a key.
    /// * Momento: `delete`
    #[serde(alias = "del")]
    Delete,
}
