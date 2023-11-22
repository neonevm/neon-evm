use crate::{
    commands::get_neon_elf::get_elf_parameter,
    types::tracer_ch_common::{AccountRow, ChError, RevisionRow, SlotParent, ROOT_BLOCK_DELAY},
};

use super::{
    tracer_ch_common::{ChResult, EthSyncStatus, EthSyncing, RevisionMap, SlotParentRooted},
    ChDbConfig,
};

use clickhouse::Client;
use log::{debug, error, info};
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
    time::Instant,
};

#[derive(Clone)]
pub struct ClickHouseDb {
    pub client: Client,
}

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

        ClickHouseDb { client }
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

    pub async fn get_earliest_rooted_slot(&self) -> ChResult<u64> {
        let time_start = Instant::now();
        let query = "SELECT min(slot) FROM events.rooted_slots";
        let result = self
            .client
            .query(query)
            .fetch_one::<u64>()
            .await
            .map_err(std::convert::Into::into);
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_earliest_rooted_slot sql returned {result:?}, time: {} sec",
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

    pub async fn get_account_at(
        &self,
        pubkey: &Pubkey,
        slot: u64,
        tx_index_in_block: Option<u64>,
    ) -> ChResult<Option<Account>> {
        if let Some(tx_index_in_block) = tx_index_in_block {
            return if let Some(account) = self
                .get_account_at_index_in_block(pubkey, slot, tx_index_in_block)
                .await?
            {
                Ok(Some(account))
            } else {
                self.get_account_at_slot(pubkey, slot - 1).await
            };
        }

        self.get_account_at_slot(pubkey, slot).await
    }

    async fn get_account_at_slot(
        &self,
        pubkey: &Pubkey,
        slot: u64,
    ) -> Result<Option<Account>, ChError> {
        info!("get_account_at_slot {{ pubkey: {pubkey}, slot: {slot} }}");
        let (first, mut branch) = self.get_branch_slots(Some(slot)).await.map_err(|e| {
            error!("get_branch_slots error: {:?}", e);
            e
        })?;

        let pubkey_str = format!("{:?}", pubkey.to_bytes());

        if let Some(rooted_slot) = self
            .get_account_rooted_slot(&pubkey_str, first)
            .await
            .map_err(|e| {
                error!("get_account_rooted_slot error: {:?}", e);
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
                    .bind(branch.as_slice())
                    .fetch_one::<AccountRow>()
                    .await,
            )
            .map_err(|e| {
                error!("get_account_at_slot error: {e}");
                ChError::Db(e)
            })?;
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at_slot {{ pubkey: {pubkey}, slot: {slot} }} sql(1) returned {row:?}, time: {} sec",
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

        let result = row
            .map(|a| a.try_into())
            .transpose()
            .map_err(|e| ChError::Db(clickhouse::error::Error::Custom(e)));

        info!("get_account_at_slot {{ pubkey: {pubkey}, slot: {slot} }} -> {result:?}");

        result
    }

    async fn get_account_at_index_in_block(
        &self,
        pubkey: &Pubkey,
        slot: u64,
        tx_index_in_block: u64,
    ) -> ChResult<Option<Account>> {
        info!(
            "get_account_at_index_in_block {{ pubkey: {pubkey}, slot: {slot}, tx_index_in_block: {tx_index_in_block} }}"
        );

        let query = r#"
            SELECT owner, lamports, executable, rent_epoch, data, txn_signature
            FROM events.update_account_distributed
            WHERE pubkey = ?
              AND slot = ?
              AND write_version <= ?
            ORDER BY write_version DESC
            LIMIT 1
        "#;

        let time_start = Instant::now();

        let account = Self::row_opt(
            self.client
                .query(query)
                .bind(format!("{:?}", pubkey.to_bytes()))
                .bind(slot)
                .bind(tx_index_in_block)
                .fetch_one::<AccountRow>()
                .await,
        )
        .map_err(|e| {
            error!("get_account_at_index_in_block error: {e}");
            ChError::Db(e)
        })?
        .map(|a| a.try_into())
        .transpose()
        .map_err(|e| ChError::Db(clickhouse::error::Error::Custom(e)))?;

        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_account_at_index_in_block {{ pubkey: {pubkey}, slot: {slot}, tx_index_in_block: {tx_index_in_block} }} sql(1) returned {account:?}, time: {} sec",
            execution_time.as_secs_f64()
        );

        Ok(account)
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
            self.get_account_at(pubkey, parent, None).await
        } else {
            Ok(None)
        }
    }

    pub async fn get_neon_revision(&self, slot: Slot, pubkey: &Pubkey) -> ChResult<String> {
        let query = r#"SELECT data
        FROM events.update_account_distributed
        WHERE
            pubkey = ? AND slot <= ?
        ORDER BY
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

    pub async fn get_neon_revisions(&self, pubkey: &Pubkey) -> ChResult<RevisionMap> {
        let query = r#"SELECT slot, data
        FROM events.update_account_distributed
        WHERE
            pubkey = ?
        ORDER BY
            slot ASC,
            write_version ASC"#;

        let pubkey_str = format!("{:?}", pubkey.to_bytes());
        let rows: Vec<RevisionRow> = self
            .client
            .query(query)
            .bind(pubkey_str)
            .fetch_all()
            .await?;

        let mut results: Vec<(u64, String)> = Vec::new();

        for row in rows {
            let neon_revision = get_elf_parameter(&row.data, "NEON_REVISION").map_err(|e| {
                ChError::Db(clickhouse::error::Error::Custom(format!(
                    "Failed to get NEON_REVISION, error: {:?}",
                    e
                )))
            })?;
            results.push((row.slot, neon_revision));
        }
        let ranges = RevisionMap::build_ranges(results);

        Ok(RevisionMap::new(ranges))
    }

    pub async fn get_slot_by_blockhash(&self, blockhash: &str) -> ChResult<u64> {
        let query = r#"SELECT slot
        FROM events.notify_block_distributed
        WHERE hash = ?
        LIMIT 1
        "#;

        let slot = Self::row_opt(
            self.client
                .query(query)
                .bind(blockhash)
                .fetch_one::<u64>()
                .await,
        )?;

        match slot {
            Some(slot) => Ok(slot),
            None => Err(ChError::Db(clickhouse::error::Error::Custom(
                "get_slot_by_blockhash: no data available".to_string(),
            ))),
        }
    }

    pub async fn get_sync_status(&self) -> ChResult<EthSyncStatus> {
        let query_is_startup = r#"SELECT is_startup
        FROM events.update_account_distributed
        WHERE slot = (
          SELECT MAX(slot)
          FROM events.update_account_distributed
        )
        LIMIT 1
        "#;

        let is_startup = Self::row_opt(
            self.client
                .query(query_is_startup)
                .fetch_one::<bool>()
                .await,
        )?;

        if let Some(true) = is_startup {
            let query = r#"SELECT slot
            FROM (
              (SELECT MIN(slot) as slot FROM events.notify_block_distributed)
              UNION ALL
              (SELECT MAX(slot) as slot FROM events.notify_block_distributed)
              UNION ALL
              (SELECT MAX(slot) as slot FROM events.notify_block_distributed)
            )
            ORDER BY slot ASC
            "#;

            let data = Self::row_opt(self.client.query(query).fetch_one::<EthSyncing>().await)?;

            return match data {
                Some(data) => Ok(EthSyncStatus::new(Some(data))),
                None => Err(ChError::Db(clickhouse::error::Error::Custom(
                    "get_sync_status: no data available".to_string(),
                ))),
            };
        }

        Ok(EthSyncStatus::new(None))
    }

    fn row_opt<T>(result: clickhouse::error::Result<T>) -> clickhouse::error::Result<Option<T>> {
        match result {
            Ok(row) => Ok(Some(row)),
            Err(clickhouse::error::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
