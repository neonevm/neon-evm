//! `EVMLoader` query account cache.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use solana_program::{keccak::Hash, pubkey::Pubkey};

/// Represents error states of queries.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("account not found")]
    AccountNotFound,
    #[error("account was changed")]
    AccountChanged,
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

    /// Checks if the hash of an account was changed.
    /// Adds new entry if missing in the cache.
    pub fn changed(&mut self, address: &Pubkey, new_hash: Hash) -> bool {
        if let Some(hash) = self.cache.get(address).map(|d| d.hash) {
            return hash != new_hash;
        }
        self.cache.insert(*address, Data{ hash: new_hash });
        false
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Data {
    hash: Hash,
}
