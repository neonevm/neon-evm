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
    cache: BTreeMap<Pubkey, Value>,
}

impl AccountCache {
    /// Creates new instance of the cache object.
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, address: Pubkey, value: Value) -> Result<()> {
        if self.cache.insert(address, value).is_some() {
            return Err(Error::AccountAlreadyCached);
        }
        Ok(())
    }

    pub fn owner(&self, address: &Pubkey) -> Option<Pubkey> {
        self.cache.get(address).map(|v| v.owner)
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Value {
    owner: Pubkey,
    length: usize,
    lamports: u64,
    executable: bool,
    rent_epoch: Epoch,
    data: Option<Vec<u8>>,
}

impl Value {
    pub fn from(info: &AccountStorageInfo, offset: usize, length: usize) -> Self {
        Value {
            owner: *info.owner,
            length: info.data.borrow().len(),
            lamports: info.lamports,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
            data: clone_chunk(&info.data.borrow(), offset, length)
        }
    }

    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }
}

fn clone_chunk(data: &[u8], offset: usize, length: usize) -> Option<Vec<u8>> {
    if offset >= data.len() || offset + length > data.len() {
        None
    } else {
        Some(data[offset..offset + length].to_owned())
    }
}
