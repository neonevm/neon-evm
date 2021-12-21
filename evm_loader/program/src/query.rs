//! `EVMLoader` query account cache.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use solana_program::{clock::Epoch, pubkey::Pubkey};
use crate::solana_backend::AccountStorageInfo;

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

impl Drop for AccountCache {
    fn drop(&mut self) {
        debug_print!("drop AccountCache");
    }
}

impl AccountCache {
    /// Creates new instance of the cache object.
    pub fn new() -> Self {
        debug_print!("new AccountCache");
        Self {
            cache: BTreeMap::new(),
        }
    }

    /// Inserts or replaces entry into the cache.
    pub fn put(&mut self, address: Pubkey, value: Value) {
        self.cache.remove(&address);
        self.cache.insert(address, value);
    }

    /// Returns owner of an account if found.
    pub fn owner(&self, address: &Pubkey) -> Option<Pubkey> {
        self.cache.get(address).map(|v| v.owner)
    }

    /// Returns length of an account's data if found.
    pub fn length(&self, address: &Pubkey) -> Option<usize> {
        self.cache.get(address).map(|v| v.length)
    }

    /// Returns lamports value of an account if found.
    pub fn lamports(&self, address: &Pubkey) -> Option<u64> {
        self.cache.get(address).map(|v| v.lamports)
    }

    /// Returns executable flag of an account if found.
    pub fn executable(&self, address: &Pubkey) -> Option<bool> {
        self.cache.get(address).map(|v| v.executable)
    }

    /// Returns rent epoch of an account if found.
    pub fn rent_epoch(&self, address: &Pubkey) -> Option<Epoch> {
        self.cache.get(address).map(|v| v.rent_epoch)
    }

    /// Returns chunk of data of an account if found and correct range.
    pub fn data(&self, address: &Pubkey, offset: usize, length: usize) -> Result<Vec<u8>> {
        match self.cache.get(address) {
            None => Err(Error::AccountNotFound),
            Some(v) => {
                if offset < v.offset {
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
    owner: Pubkey,
    length: usize,
    lamports: u64,
    executable: bool,
    rent_epoch: Epoch,
    offset: usize,
    data: Option<Vec<u8>>,
}

impl Value {
    /// Constructs a cache entry value from corresponding account info.
    pub fn from(info: &AccountStorageInfo, offset: usize, length: usize) -> Self {
        Self {
            owner: *info.owner,
            length: info.data.borrow().len(),
            lamports: info.lamports,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
            offset,
            data: clone_chunk(&info.data.borrow(), offset, length),
        }
    }

    /// Checks if account got data. Dataless accounts make no sense in the cache.
    pub const fn has_data(&self) -> bool {
        self.data.is_some()
    }
}

/// Creates vector from a slice checking the range validity.
fn clone_chunk(data: &[u8], offset: usize, length: usize) -> Option<Vec<u8>> {
    let right = offset.saturating_add(length);
    if offset >= data.len() || right > data.len() {
        None
    } else {
        Some(data[offset..right].to_owned())
    }
}
