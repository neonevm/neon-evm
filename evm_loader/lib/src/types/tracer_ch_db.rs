use crate::commands::get_neon_elf::get_elf_parameter;

use super::ChDbConfig;
use clickhouse::{Client, Row};
use log::{debug, info};
use rand::Rng;
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    pubkey::Pubkey,
};
use std::{
    cmp::{
        Ord,
        Ordering::{Equal, Greater, Less},
    },
    convert::TryFrom,
    fmt,
    sync::Arc,
    time::Instant,
};
use thiserror::Error;

const ROOT_BLOCK_DELAY: u8 = 100;

#[derive(Error, Debug)]
pub enum ChError {
    #[error("clickhouse: {}", .0)]
    Db(#[from] clickhouse::error::Error),
}

pub type ChResult<T> = std::result::Result<T, ChError>;

#[allow(dead_code)]
#[derive(Clone)]
pub struct ClickHouseDb {
    pub client: Arc<Client>,
}

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
    fn is_rooted(&self) -> bool {
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
            .field(
                "owner",
                &format!("{}", bs58::encode(&self.owner).into_string()),
            )
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

#[allow(dead_code)]
impl ClickHouseDb {
    pub fn new(config: &ChDbConfig) -> Self {
        let url_id = rand::thread_rng().gen_range(0..config.clickhouse_url.len());
        let url = config.clickhouse_url.get(url_id).unwrap();

        let client = match (&config.clickhouse_user, &config.clickhouse_password) {
            (None, None | Some(_)) => Client::default().with_url(url),
            (Some(user), None) => Client::default().with_url(url).with_user(user),
            (Some(user), Some(password)) => Client::default()
                .with_url(url)
                .with_user(user)
                .with_password(password),
        };

        ClickHouseDb {
            client: Arc::new(client),
        }
    }

    // return value is not used for tracer methods
    pub async fn get_block_time(&self, slot: Slot) -> ChResult<UnixTimestamp> {
        let time_start = Instant::now();
        let query =
            "SELECT JSONExtractInt(notify_block_json, 'block_time') FROM events.notify_block_distributed WHERE slot = ? LIMIT 1";
        let result = self
            .client
            .query(query)
            .bind(slot)
            .fetch_one::<UnixTimestamp>()
            .await
            .map_err(std::convert::Into::into);
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_block_time sql time: {} sec",
            execution_time.as_secs_f64()
        );
        result
    }

    pub async fn get_latest_block(&self) -> ChResult<u64> {
        let time_start = Instant::now();
        let query = "SELECT max(slot) FROM events.update_slot";
        let result = self
            .client
            .query(query)
            .fetch_one::<u64>()
            .await
            .map_err(std::convert::Into::into);
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_latest_block sql returned {result:?}, time: {} sec",
            execution_time.as_secs_f64()
        );
        result
    }

    async fn get_branch_slots(&self, slot: Option<u64>) -> ChResult<(u64, Vec<u64>)> {
        fn branch_from(
            rows: Vec<SlotParent>,
            test_start: &dyn Fn(&SlotParent) -> bool,
        ) -> Vec<u64> {
            let mut branch = vec![];
            let mut last_parent_opt = None;
            for row in rows {
                if let Some(ref last_parent) = last_parent_opt {
                    if row.slot == *last_parent {
                        branch.push(row.slot);
                        last_parent_opt = row.parent;
                    }
                } else if test_start(&row) {
                    branch.push(row.slot);
                    last_parent_opt = row.parent;
                }
            }
            branch
        }

        info!("get_branch_slots {{ slot: {slot:?} }}");

        let query = r#"
            SELECT DISTINCT ON (slot, parent) slot, parent, status
            FROM events.update_slot
            WHERE slot >= (
                    SELECT slot - ?
                    FROM events.rooted_slots
                    ORDER BY slot DESC
                    LIMIT 1
                )
                AND isNotNull(parent)
            ORDER BY slot DESC, status DESC
            "#;
        let time_start = Instant::now();
        let mut rows = self
            .client
            .query(query)
            .bind(ROOT_BLOCK_DELAY)
            .fetch_all::<SlotParent>()
            .await?;

        let first = if let Some(first) = rows.pop() {
            first
        } else {
            let err = clickhouse::error::Error::Custom("Rooted slot not found".to_string());
            return Err(ChError::Db(err));
        };

        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_branch_slots {{ slot: {slot:?} }} sql(1) returned {} row(s), time: {} sec",
            rows.len(),
            execution_time.as_secs_f64(),
        );

        debug!("get_branch_slots {{ slot: {slot:?} }} sql(1) result:\n{rows:?}");

        let result = if let Some(slot) = slot {
            match slot.cmp(&first.slot) {
                Less | Equal => Ok((slot, vec![])),
                Greater => {
                    let branch = branch_from(rows, &|row| row.slot == slot);
                    if branch.is_empty() {
                        let err = clickhouse::error::Error::Custom(format!(
                            "requested slot not found {slot}",
                        ));
                        return Err(ChError::Db(err));
                    }
                    Ok((first.slot, branch))
                }
            }
        } else {
            let branch = branch_from(rows, &SlotParent::is_rooted);
            Ok((first.slot, branch))
        };

        debug!("get_branch_slots {{ slot: {slot:?} }} -> {result:?}");

        result
    }

    async fn get_account_rooted_slot(&self, key: &str, slot: u64) -> ChResult<Option<u64>> {
        info!("get_account_rooted_slot {{ key: {key}, slot: {slot} }}");

        let query = r#"
        SELECT DISTINCT uad.slot
        FROM events.update_account_distributed AS uad
        WHERE uad.pubkey = ?
          AND uad.slot <= ?
          AND (
            SELECT COUNT(slot)
            FROM events.rooted_slots
            WHERE slot = ?
          ) >= 1
        ORDER BY uad.slot DESC
        LIMIT 1
        "#;

        let time_start = Instant::now();
        let slot_opt = Self::row_opt(
            self.client
                .query(query)
                .bind(key)
                .bind(slot)
                .bind(slot)
                .fetch_one::<u64>()
                .await,
        )?;

        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_account_rooted_slot {{ key: {key}, slot: {slot} }} sql(1) returned {slot_opt:?}, time: {} sec",
            execution_time.as_secs_f64(),
        );

        Ok(slot_opt)
    }

    #[allow(clippy::too_many_lines)]
    pub async fn get_account_at(&self, pubkey: &Pubkey, slot: u64) -> ChResult<Option<Account>> {
        info!("get_account_at {{ pubkey: {pubkey}, slot: {slot} }}");
        let (first, mut branch) = self.get_branch_slots(Some(slot)).await.map_err(|e| {
            println!("get_branch_slots error: {:?}", e);
            e
        })?;

        let pubkey_str = format!("{:?}", pubkey.to_bytes());

        if let Some(rooted_slot) = self
            .get_account_rooted_slot(&pubkey_str, first)
            .await
            .map_err(|e| {
                println!("get_account_rooted_slot error: {:?}", e);
                e
            })?
        {
            branch.push(rooted_slot);
        }

        let mut row = if branch.is_empty() {
            None
        } else {
            let query = r#"
                SELECT owner, lamports, executable, rent_epoch, data, txn_signature
                FROM events.update_account_distributed
                WHERE pubkey = ?
                  AND slot IN ?
                ORDER BY pubkey, slot DESC, write_version DESC
                LIMIT 1
            "#;

            let time_start = Instant::now();
            let row = Self::row_opt(
                self.client
                    .query(query)
                    .bind(pubkey_str.clone())
                    .bind(&branch.as_slice())
                    .fetch_one::<AccountRow>()
                    .await,
            )
            .map_err(|e| {
                println!("get_account_at error: {e}");
                ChError::Db(e)
            })?;
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at {{ pubkey: {pubkey}, slot: {slot} }} sql(1) returned {row:?}, time: {} sec",
                execution_time.as_secs_f64()
            );

            row
        };

        if row.is_none() {
            let time_start = Instant::now();
            row = self.get_older_account_row_at(&pubkey_str, slot).await?;
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at {{ pubkey: {pubkey}, slot: {slot} }} sql(2) returned {row:?}, time: {} sec",
                execution_time.as_secs_f64()
            );
        }

        let result = if let Some(acc) = row {
            acc.try_into()
                .map(Some)
                .map_err(|err| ChError::Db(clickhouse::error::Error::Custom(err)))
        } else {
            Ok(None)
        };

        info!("get_account_at {{ pubkey: {pubkey}, slot: {slot} }} -> {result:?}");

        result
    }

    async fn get_older_account_row_at(
        &self,
        pubkey: &str,
        slot: u64,
    ) -> ChResult<Option<AccountRow>> {
        let query = r#"
            SELECT owner, lamports, executable, rent_epoch, data, txn_signature
            FROM events.older_account_distributed FINAL
            WHERE pubkey = ? AND slot <= ?
            ORDER BY slot DESC
            LIMIT 1
        "#;
        Self::row_opt(
            self.client
                .query(query)
                .bind(pubkey)
                .bind(slot)
                .fetch_one::<AccountRow>()
                .await,
        )
        .map_err(|e| {
            println!("get_last_older_account_row error: {e}");
            ChError::Db(e)
        })
    }

    async fn get_sol_sig_rooted_slot(&self, sol_sig: &[u8; 64]) -> ChResult<Option<SlotParent>> {
        let query = r#"
            SELECT slot, parent
            FROM events.rooted_slots
            WHERE slot IN (
                    SELECT slot
                    FROM events.notify_transaction_distributed
                    WHERE signature = ?
                )
            ORDER BY slot DESC
            LIMIT 1
        "#;

        Self::row_opt(
            self.client
                .query(query)
                .bind(sol_sig.as_slice())
                .fetch_one::<SlotParentRooted>()
                .await,
        )
        .map(|slot_parent_rooted_opt| {
            slot_parent_rooted_opt.map(|slot_parent_rooted| slot_parent_rooted.into())
        })
        .map_err(|e| {
            println!("get_sol_sig_rooted_slot error: {e}");
            ChError::Db(e)
        })
    }

    async fn get_sol_sig_confirmed_slot(&self, sol_sig: &[u8; 64]) -> ChResult<Option<SlotParent>> {
        let (_, slot_vec) = self.get_branch_slots(None).await?;
        let query = r#"
            SELECT slot, parent, status
            FROM events.update_slot
            WHERE slot IN ?
                AND slot IN (
                    SELECT slot
                    FROM events.notify_transaction_distributed
                    WHERE signature = ?
                )
            ORDER BY slot DESC
            LIMIT 1
        "#;

        Self::row_opt(
            self.client
                .query(query)
                .bind(slot_vec.as_slice())
                .bind(sol_sig.as_slice())
                .fetch_one::<SlotParent>()
                .await,
        )
        .map_err(|e| {
            println!("get_sol_sig_confirmed_slot error: {e}");
            ChError::Db(e)
        })
    }

    #[allow(clippy::unused_self)]
    pub async fn get_account_by_sol_sig(
        &self,
        pubkey: &Pubkey,
        sol_sig: &[u8; 64],
    ) -> ChResult<Option<Account>> {
        let sol_sig_str = bs58::encode(sol_sig).into_string();
        info!("get_account_by_sol_sig {{ pubkey: {pubkey}, sol_sig: {sol_sig_str} }}");
        let time_start = Instant::now();
        let mut slot_opt = self.get_sol_sig_rooted_slot(sol_sig).await?;
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_sol_sig_rooted_slot({sol_sig_str}) -> {slot_opt:?}, time: {} sec",
            execution_time.as_secs_f64()
        );

        if slot_opt.is_none() {
            let time_start = Instant::now();
            slot_opt = self.get_sol_sig_confirmed_slot(sol_sig).await?;
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_sol_sig_confirmed_slot({sol_sig_str}) -> {slot_opt:?}, time: {} sec",
                execution_time.as_secs_f64()
            );
        }

        let slot = if let Some(slot) = slot_opt {
            slot
        } else {
            return Ok(None);
        };

        // Try to find account changes within the given slot.
        let query = r#"
            SELECT DISTINCT ON (pubkey, txn_signature, write_version)
                   owner, lamports, executable, rent_epoch, data, txn_signature
            FROM events.update_account_distributed
            WHERE slot = ? AND pubkey = ?
            ORDER BY write_version DESC
        "#;

        let pubkey_str = format!("{:?}", pubkey.to_bytes());
        let time_start = Instant::now();
        let rows = self
            .client
            .query(query)
            .bind(slot.slot)
            .bind(pubkey_str)
            .fetch_all::<AccountRow>()
            .await?;
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_account_by_sol_sig {{ pubkey: {pubkey}, sol_sig: {sol_sig_str} }} \
                sql(1) returned {} row(s), time: {} sec",
            rows.len(),
            execution_time.as_secs_f64(),
        );

        debug!(
            "get_account_by_sol_sig {{ pubkey: {pubkey}, sol_sig: {sol_sig_str} }} \
            sql(1) returned:\n{rows:?}"
        );

        let row_found = rows
            .into_iter()
            .skip_while(|row| {
                row.txn_signature
                    .iter()
                    .filter_map(|v| *v)
                    .collect::<Vec<u8>>()
                    .as_slice()
                    != sol_sig.as_slice()
            })
            .nth(1);

        info!("get_account_by_sol_sig {{ pubkey: {pubkey}, sol_sig: {sol_sig_str} }}, row_found: {row_found:?}");

        if row_found.is_some() {
            return row_found
                .map(|row| {
                    row.try_into()
                        .map_err(|err| ChError::Db(clickhouse::error::Error::Custom(err)))
                })
                .transpose();
        }

        // If not found, get closest account state in one of previous slots
        if let Some(parent) = slot.parent {
            self.get_account_at(pubkey, parent).await
        } else {
            Ok(None)
        }
    }

    pub async fn get_neon_revision(&self, slot: Slot, pubkey: &Pubkey) -> ChResult<String> {
        let query = r#"SELECT data
        FROM events.update_account_distributed
        WHERE
            pubkey = ?
        ORDER BY
            abs(? - slot) ASC,
            pubkey ASC,
            slot ASC,
            write_version ASC
        LIMIT 1
        "#;

        let pubkey_str = format!("{:?}", pubkey.to_bytes());

        let data = Self::row_opt(
            self.client
                .query(query)
                .bind(pubkey_str)
                .bind(slot)
                .fetch_one::<Vec<u8>>()
                .await,
        )?;

        match data {
            Some(data) => {
                let neon_revision =
                    get_elf_parameter(data.as_slice(), "NEON_REVISION").map_err(|e| {
                        ChError::Db(clickhouse::error::Error::Custom(format!(
                            "Failed to get NEON_REVISION, error: {e:?}",
                        )))
                    })?;
                Ok(neon_revision)
            }
            None => {
                let err = clickhouse::error::Error::Custom(format!(
                    "get_neon_revision: for slot {slot} and pubkey {pubkey} not found",
                ));
                Err(ChError::Db(err))
            }
        }
    }

    fn row_opt<T>(result: clickhouse::error::Result<T>) -> clickhouse::error::Result<Option<T>> {
        match result {
            Ok(row) => Ok(Some(row)),
            Err(clickhouse::error::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
