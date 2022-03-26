//! `EVMLoader` query account cache.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use solana_program::{clock::Epoch, pubkey::Pubkey};


const KB: usize = 1024;
pub const MAX_CHUNK_LEN: usize = 8 * KB;

/// Represents error states of queries.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("account not found")]
    AccountNotFound,
    #[error("invalid argument")]
    InvalidArgument,
}

/// Result type for queries.
pub type Result<T> = std::result::Result<T, Error>;

/// Represents cache of queries to Solana accounts.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct AccountCache {
    cache: BTreeMap<Pubkey, Value>,
}

impl AccountCache {
    /// Creates new instance of the cache object.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }

    /// Inserts or replaces entry into the cache.
    pub fn put(&mut self, address: Pubkey, value: Value) {
        self.cache.insert(address, value);
    }

    /// Returns owner of an account if found.
    pub fn owner(&self, address: &Pubkey) -> Result<Pubkey> {
        self.cache.get(address).map(|v| v.owner).ok_or(Error::AccountNotFound)
    }

    /// Returns length of an account's data if found.
    pub fn length(&self, address: &Pubkey) -> Result<usize> {
        self.cache.get(address).map(|v| v.length).ok_or(Error::AccountNotFound)
    }

    /// Returns lamports value of an account if found.
    pub fn lamports(&self, address: &Pubkey) -> Result<u64> {
        self.cache.get(address).map(|v| v.lamports).ok_or(Error::AccountNotFound)
    }

    /// Returns executable flag of an account if found.
    pub fn executable(&self, address: &Pubkey) -> Result<bool> {
        self.cache.get(address).map(|v| v.executable).ok_or(Error::AccountNotFound)
    }

    /// Returns rent epoch of an account if found.
    pub fn rent_epoch(&self, address: &Pubkey) -> Result<Epoch> {
        self.cache.get(address).map(|v| v.rent_epoch).ok_or(Error::AccountNotFound)
    }

    /// Returns chunk of data of an account if found and correct range.
    pub fn data(&self, address: &Pubkey, offset: usize, length: usize) -> Result<Vec<u8>> {
        match self.cache.get(address) {
            None => Err(Error::AccountNotFound),
            Some(v) => {
                if offset < v.offset || length == 0 {
                    return Err(Error::InvalidArgument);
                }
                match &v.data {
                    None => Err(Error::InvalidArgument),
                    Some(d) => clone_chunk(d, offset - v.offset, length).map_or_else(
                        || Err(Error::InvalidArgument),
                        Ok
                    ),
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Value {
    pub owner: Pubkey,
    pub length: usize,
    pub lamports: u64,
    pub executable: bool,
    pub rent_epoch: Epoch,
    pub offset: usize,
    #[serde(with = "serde_bytes")]
    pub data: Option<Vec<u8>>,
}

impl Value {
    /// Checks if account got data. Dataless accounts make no sense in the cache.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        self.data.is_some()
    }
}

/// Creates vector from a slice checking the range validity.
#[must_use]
pub fn clone_chunk(data: &[u8], offset: usize, length: usize) -> Option<Vec<u8>> {
    let right = offset.saturating_add(length);
    if offset >= data.len() || right > data.len() {
        None
    } else {
        Some(data[offset..right].to_owned())
    }
}
