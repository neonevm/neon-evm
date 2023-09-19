use std::fmt;

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use solana_sdk::{account::Account, pubkey::Pubkey};
use thiserror::Error;

pub const ROOT_BLOCK_DELAY: u8 = 100;

#[derive(Error, Debug)]
pub enum ChError {
    #[error("clickhouse: {}", .0)]
    Db(#[from] clickhouse::error::Error),
}

pub type ChResult<T> = std::result::Result<T, ChError>;

pub enum SlotStatus {
    #[allow(unused)]
    Confirmed = 1,
    #[allow(unused)]
    Processed = 2,
    Rooted = 3,
}

#[derive(Debug, Row, serde::Deserialize, Clone)]
pub struct SlotParent {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: u8,
}

#[derive(Debug, Row, serde::Deserialize, Clone)]
pub struct SlotParentRooted {
    pub slot: u64,
    pub parent: Option<u64>,
}

impl From<SlotParentRooted> for SlotParent {
    fn from(slot_parent_rooted: SlotParentRooted) -> Self {
        SlotParent {
            slot: slot_parent_rooted.slot,
            parent: slot_parent_rooted.parent,
            status: SlotStatus::Rooted as u8,
        }
    }
}

impl SlotParent {
    pub fn is_rooted(&self) -> bool {
        self.status == SlotStatus::Rooted as u8
    }
}

#[derive(Row, serde::Deserialize, Clone)]
pub struct AccountRow {
    pub owner: Vec<u8>,
    pub lamports: u64,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub txn_signature: Vec<Option<u8>>,
}

impl fmt::Display for AccountRow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AccountRow {{\n    owner: {},\n    lamports: {},\n    executable: {},\n    rent_epoch: {},\n}}",
            bs58::encode(&self.owner).into_string(),
            self.lamports,
            self.executable,
            self.rent_epoch,
        )
    }
}

impl fmt::Debug for AccountRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("owner", &bs58::encode(&self.owner).into_string())
            .field("lamports", &self.lamports)
            .field("executable", &self.executable)
            .field("rent_epoch", &self.rent_epoch)
            .finish()
    }
}

impl TryInto<Account> for AccountRow {
    type Error = String;

    fn try_into(self) -> Result<Account, Self::Error> {
        let owner = Pubkey::try_from(self.owner).map_err(|src| {
            format!(
                "Incorrect slice length ({}) while converting owner from: {src:?}",
                src.len(),
            )
        })?;

        Ok(Account {
            lamports: self.lamports,
            data: self.data,
            owner,
            rent_epoch: self.rent_epoch,
            executable: self.executable,
        })
    }
}

pub enum EthSyncStatus {
    Syncing(EthSyncing),
    Synced,
}

impl EthSyncStatus {
    pub fn new(syncing_status: Option<EthSyncing>) -> Self {
        if let Some(syncing_status) = syncing_status {
            Self::Syncing(syncing_status)
        } else {
            Self::Synced
        }
    }
}

#[derive(Row, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EthSyncing {
    pub starting_block: u64,
    pub current_block: u64,
    pub highest_block: u64,
}
