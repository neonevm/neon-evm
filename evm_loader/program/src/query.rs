//! `EVMLoader` query account cache.

use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use solana_program::pubkey::Pubkey;

/// Represents cache of queries to Solana accounts.
#[derive(Serialize, Deserialize, Debug)]
pub struct AccountCache {
    cache: BTreeMap<Key, Cache>,
}

impl AccountCache {
    pub fn new() -> Self {
        Self { cache: BTreeMap::new() }
    }

    pub fn get(&self, address: Pubkey, offset: usize, length: usize) -> Option<&Cache> {
        self.cache.get(&Key{address, offset, length})
    }

    pub fn set(&mut self, address: Pubkey, offset: usize, length: usize, cache: Cache) {
        *self.cache.entry(Key{address, offset, length}).or_insert_with(Cache::default) = cache;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, Debug)]
struct Key {
    address: Pubkey,
    offset: usize,
    length: usize,
}

#[derive(Default)]
#[derive(Serialize, Deserialize, Debug)]
pub struct Cache {
    pub owner: Pubkey,
    pub length: usize,
    pub data: Vec<u8>,
}
