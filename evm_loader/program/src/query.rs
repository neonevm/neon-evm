//! `EVMLoader` query account cache.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

/// Represents error states of queries.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("account not found")]
    AccountNotFound,
    #[error("account already cached")]
    AccountAlreadyCached,
    #[error("invalid argument")]
    InvalidArgument,
}

/// Result type for queries.
pub type Result<T> = std::result::Result<T, Error>;

/// Represents cache of queries to Solana accounts.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct AccountCache {
    cache: BTreeMap<Pubkey, Data>,
}

impl AccountCache {
    /// Creates new instance of the cache object.
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }

    pub fn add(_address: &Pubkey, _data: &[u8]) -> Result<()> {
        Ok(())
    }

    pub fn get(_address: &Pubkey, _offset: usize, _length: usize) -> Result<Vec<u8>> {
        Ok(vec![])
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Data {
    data: Vec<u8>
}
