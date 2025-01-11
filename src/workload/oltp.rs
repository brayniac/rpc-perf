use crate::config;
use crate::Config;

#[derive(Clone)]
pub struct Oltp {
    pub(crate) tables: u8,
    pub(crate) keys: i32,
}

impl Oltp {
    pub fn new(_config: &Config, oltp: &config::Oltp) -> Self {
        {
            Self {
                tables: oltp.tables(),
                keys: oltp.keys(),
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum OltpRequest {
    // "SELECT c FROM sbtest%u WHERE id=?",
    PointSelect(PointSelect),
}

#[derive(Debug, PartialEq)]
pub struct PointSelect {
    pub id: i32,
    pub table: String,
}
