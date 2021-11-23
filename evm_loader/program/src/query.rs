//! `EVMLoader` query account cache.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

/// Represents cache of queries to Solana accounts.
#[derive(Serialize, Deserialize, Debug)]
pub struct AccountCache {
    metadata_cache: BTreeMap<Pubkey, Metadata>,
    data_cache: BTreeMap<DataKey, Vec<u8>>,
}

impl AccountCache {
    /// Creates new instance of the cache object.
    pub fn new() -> Self {
        Self {
            metadata_cache: BTreeMap::new(),
            data_cache: BTreeMap::new(),
        }
    }

    /// Returns owner address and data length if account exists.
    pub fn get_metadata(&self, address: Pubkey) -> Option<(Pubkey, usize)> {
        self.metadata_cache.get(&address).map(|md| (md.owner, md.length))
    }

    /// Updates metadata for the address.
    pub fn set_metadata(&mut self, address: Pubkey, owner: Pubkey, length: usize) {
        *self.metadata_cache.entry(address).or_insert_with(Metadata::default) = Metadata{owner, length};
    }

    /// Returns account's data subset if account exists.
    pub fn get_data(&self, address: Pubkey, offset: usize, length: usize) -> Option<&Vec<u8>> {
        let key = DataKey{address, offset, length};
        self.data_cache.get(&key)
    }

    /// Updates data subset for combination of address, offset and length.
    pub fn set_data(&mut self, address: Pubkey, offset: usize, length: usize, data: &[u8]) {
        let key = DataKey{address, offset, length};
        *self.data_cache.entry(key).or_insert_with(Vec::default) = data.to_owned();
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, Debug)]
struct DataKey {
    address: Pubkey,
    offset: usize,
    length: usize,
}

#[derive(Default)]
#[derive(Serialize, Deserialize, Debug)]
struct Metadata {
    pub owner: Pubkey,
    pub length: usize,
}
