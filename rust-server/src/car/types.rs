use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Cid {
    pub version: u8,
    pub codec: u8,
    pub digest_type: u8,
    pub digest: Vec<u8>,
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.digest))
    }
}

#[derive(Debug)]
pub struct CarEntry {
    pub cid: Cid,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CarHeader {
    #[allow(dead_code)]
    pub version: u8,
    pub roots: Vec<Cid>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CidLink(pub String);

impl From<&Cid> for CidLink {
    fn from(cid: &Cid) -> Self {
        CidLink(cid.to_string())
    }
}

impl CidLink {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
