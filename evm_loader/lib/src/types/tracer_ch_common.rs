use std::fmt;

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::collections::BTreeMap;
use std::time::Instant;
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

// NEON_REVISION row
#[derive(Row, Deserialize)]
pub struct RevisionRow {
    pub slot: u64,
    pub data: Vec<u8>,
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

pub struct RevisionMap {
    map: BTreeMap<u64, String>,
    pub last_update: Instant,
}

impl RevisionMap {
    pub fn new(neon_revision_ranges: Vec<(u64, u64, String)>) -> Self {
        let mut map = BTreeMap::new();

        for (start, end, value) in neon_revision_ranges {
            map.insert(start, value.clone());
            map.insert(end, value);
        }

        let last_update = std::time::Instant::now();

        RevisionMap { map, last_update }
    }

    // When deploying a program for the first time it is now only available in the next slot (the slot after the one the deployment transaction landed in).
    // When undeploying / closing a program the change is visible immediately and the very next instruction even within the transaction can not access it anymore.
    // When redeploying the program becomes temporarily closed immediately and will reopen with the new version in the next slot.
    pub fn build_ranges(input: Vec<(u64, String)>) -> Vec<(u64, u64, String)> {
        let mut ranges = Vec::new();

        for i in 0..input.len() {
            let (start, rev) = input[i].clone();
            let end = if i < input.len() - 1 {
                input[i + 1].0 - 1
            } else {
                start
            };

            match i {
                0 => ranges.push((start, end + 1, rev.clone())),
                _ if i == input.len() - 1 => ranges.push((start + 1, end + 1, rev.clone())),
                _ => ranges.push((start + 1, end + 1, rev.clone())),
            }
        }
        ranges
    }

    pub fn get(&self, slot: u64) -> Option<String> {
        // Check if slot is less than the starting range or
        // greater than the ending range
        let (start, _) = self.map.iter().next()?;
        let (end, _) = self.map.iter().last()?;

        if slot < *start || slot > *end {
            return None;
        }

        let value = self.map.range(..=slot).next_back();

        value.map(|(_, v)| v.clone())
    }
}
