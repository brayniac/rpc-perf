use super::*;

#[derive(Clone, Deserialize)]
pub struct Oltp {
    tables: u8,
    keys: i32,

    #[serde(default = "one")]
    weight: usize,
}

impl Oltp {
    pub fn tables(&self) -> u8 {
        self.tables
    }

    pub fn keys(&self) -> i32 {
        self.keys
    }

    pub fn weight(&self) -> usize {
        self.weight
    }
}
