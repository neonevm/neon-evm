//! `EVMLoader` query account cache.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use solana_program::{pubkey::Pubkey, clock::Epoch};

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

    pub fn insert(&mut self,
                  address: Pubkey,
                  owner: Pubkey,
                  data_length: usize,
                  lamports: u64,
                  executable: bool,
                  rent_epoch: Epoch,
                  data: Vec<u8>,
                  ) -> Result<()> {
        let value = Value{
            owner,
            length: data_length,
            lamports,
            executable,
            rent_epoch,
            data,
        };
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
struct Value {
    owner: Pubkey,
    length: usize,
    lamports: u64,
    executable: bool,
    rent_epoch: Epoch,
    data: Vec<u8>,
}
