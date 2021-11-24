//! `EVMLoader` query account cache.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

/// Represents cache of queries to Solana accounts.
#[derive(Serialize, Deserialize, Debug)]
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

    /// Returns owner address if account exists.
    pub fn get_owner(&self, address: Pubkey) -> Option<Pubkey> {
        self.cache.get(&address).map(|d| d.owner)
    }

    /// Returns data length of account if it exists.
    pub fn get_length(&self, address: Pubkey) -> Option<usize> {
        self.cache.get(&address).map(|d| d.contents.len())
    }

    /// Returns reference to the account's data if it exists.
    pub fn get_data(&self, address: Pubkey) -> Option<&[u8]> {
        self.cache.get(&address).map(|d| d.contents.as_ref())
    }

    /// Inserts owner and data of an account.
    pub fn insert(&mut self, address: Pubkey, owner: Pubkey, contents: Vec<u8>) {
        self.cache.insert(address, Data{ owner, contents });
    }
}

#[derive(Default, Debug)]
#[derive(Serialize, Deserialize)]
struct Data {
    owner: Pubkey,
    contents: Vec<u8>,
}
