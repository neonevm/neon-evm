use super::{block, ChDbConfig};
use clickhouse::{Client, Row};
use log::info;
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

#[derive(Row, serde::Deserialize, Clone)]
pub struct SlotParent {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: String,
}

#[derive(Row, serde::Deserialize, Clone)]
pub struct AccountRow {
    owner: Vec<u8>,
    lamports: u64,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
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
    pub fn get_block_time(&self, slot: Slot) -> ChResult<UnixTimestamp> {
        let time_start = Instant::now();
        let result = block(|| async {
            let query =
                "SELECT JSONExtractInt(notify_block_json, 'block_time') FROM events.notify_block_distributed WHERE slot = ? LIMIT 1";
            self.client
                .query(query)
                .bind(slot)
                .fetch_one::<UnixTimestamp>()
                .await
                .map_err(std::convert::Into::into)
        });
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_block_time sql time: {} sec",
            execution_time.as_secs_f64()
        );
        result
    }

    pub fn get_latest_block(&self) -> ChResult<u64> {
        let time_start = Instant::now();
        let result = block(|| async {
            let query = "SELECT max(slot) FROM events.update_slot";
            self.client
                .query(query)
                .fetch_one::<u64>()
                .await
                .map_err(std::convert::Into::into)
        });
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_latest_block sql time: {} sec",
            execution_time.as_secs_f64()
        );
        result
    }

    fn get_branch_slots(&self, slot: u64) -> ChResult<(u64, Vec<u64>)> {
        let query = r#"
            SELECT distinct on (slot) slot, parent, status FROM events.update_slot
            WHERE slot >=
                (
                    SELECT slot from events.update_slot WHERE status = 'Rooted' and slot <=
                    (
	                    SELECT slot - ? FROM events.update_slot WHERE status = 'Rooted' ORDER BY slot DESC LIMIT 1
                    )
                    ORDER BY slot DESC LIMIT 1
                )
                and isNotNull(parent)
            ORDER BY slot DESC, status DESC
            "#;
        let time_start = Instant::now();
        let rows = block(|| async {
            self.client
                .query(query)
                .bind(ROOT_BLOCK_DELAY)
                .fetch_all::<SlotParent>()
                .await
        })?;
        let execution_time = Instant::now().duration_since(time_start);
        info!(
            "get_branch_slot sql(1) time: {} sec",
            execution_time.as_secs_f64()
        );

        let (root, rows) = rows.split_last().ok_or_else(|| {
            let err = clickhouse::error::Error::Custom("Rooted slot not found".to_string());
            ChError::Db(err)
        })?;

        match slot.cmp(&root.slot) {
            Less | Equal => Ok((slot, vec![])),
            Greater => {
                let mut branch: Vec<SlotParent> = vec![];

                let mut parent_finalized: Option<u64> = None;

                for row in rows {
                    if parent_finalized.is_some() {
                        if row.slot == parent_finalized.unwrap() {
                            parent_finalized = row.parent;
                        } else {
                            if row.status == "Rooted" {
                                let err = clickhouse::error::Error::Custom(
                                    "There are several branchs with rooted slots".to_string(),
                                );
                                return Err(ChError::Db(err));
                            }
                            continue;
                        }
                    } else if row.status == "Rooted" {
                        parent_finalized = row.parent;
                    }

                    if branch.is_empty() {
                        if row.slot == slot {
                            branch.push(row.clone());
                        }
                    } else if row.slot == branch.last().unwrap().parent.unwrap() {
                        branch.push(row.clone());
                    }
                }

                if branch.is_empty() {
                    let err = clickhouse::error::Error::Custom(format!(
                        "requested slot not found {}",
                        slot
                    ));
                    Err(ChError::Db(err))
                } else if branch.last().unwrap().parent.unwrap() == root.slot {
                    let branch = branch.iter().map(|row| row.slot).collect();
                    Ok((root.slot, branch))
                } else {
                    let err = clickhouse::error::Error::Custom(format!(
                        "requested slot is not on working branch {}",
                        slot
                    ));
                    Err(ChError::Db(err))
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn get_account_at(&self, key: &Pubkey, slot: u64) -> ChResult<Option<Account>> {
        let (root, branch) = self.get_branch_slots(slot).map_err(|e| {
            println!("get_branch_slots error: {:?}", e);
            e
        })?;

        let key_ = format!("{:?}", key.to_bytes());

        let mut row: Option<AccountRow> = if branch.is_empty() {
            None
        } else {
            let mut slots = format!("toUInt64({})", branch.first().unwrap());
            for slot in &branch[1..] {
                slots = format!("{}, toUInt64({})", slots, slot);
            }

            let time_start = Instant::now();
            let result = block(|| async {
                let query = format!(
                    r#"
                    SELECT owner, lamports, executable, rent_epoch, data
                    FROM events.update_account_distributed
                    WHERE
                        pubkey = ?
                        AND slot IN (SELECT arrayJoin([{}]))
                    ORDER BY pubkey DESC, slot DESC, write_version DESC
                    LIMIT 1
                    "#,
                    slots
                );
                self.client
                    .query(query.as_str())
                    .bind(key_.clone())
                    .fetch_one::<AccountRow>()
                    .await
            });
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at sql(1) time: {} sec",
                execution_time.as_secs_f64()
            );

            match result {
                Ok(row) => Some(row),
                Err(clickhouse::error::Error::RowNotFound) => None,
                Err(e) => {
                    println!("get_account_at error: {}", e);
                    return Err(ChError::Db(e));
                }
            }
        };

        if row.is_none() {
            let time_start = Instant::now();
            let result = block(|| async {
                let query = r#"
                    SELECT owner, lamports, executable, rent_epoch, data
                    FROM events.update_account_distributed
                    WHERE pubkey = ?
                        AND slot = (
                        SELECT max(b.slot)
                        FROM events.update_slot AS b
                        WHERE (b.slot IN (
                            SELECT a.slot
                            FROM events.update_account_distributed AS a
                            WHERE (a.pubkey = ?)
                                AND (a.slot <= ?)
                            ORDER BY a.slot DESC
                            LIMIT 1000
                        )) AND (b.status = 'Rooted')
                    )
                "#;
                self.client
                    .query(query)
                    .bind(key_.clone())
                    .bind(key_.clone())
                    .bind(root)
                    .fetch_one::<AccountRow>()
                    .await
            });
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at sql(2) time: {} sec",
                execution_time.as_secs_f64()
            );

            row = match result {
                Ok(row) => Some(row),
                Err(clickhouse::error::Error::RowNotFound) => None,
                Err(e) => {
                    println!("get_account_at error: {}", e);
                    return Err(ChError::Db(e));
                }
            };
        }

        if row.is_none() {
            let time_start = Instant::now();
            let result = block(|| async {
                let query = r#"
                SELECT owner, lamports, executable, rent_epoch, data
                FROM events.older_account_distributed
                WHERE pubkey = ?
                ORDER BY slot DESC LIMIT 1
                "#;
                self.client
                    .query(query)
                    .bind(key_)
                    .fetch_one::<AccountRow>()
                    .await
            });
            let execution_time = Instant::now().duration_since(time_start);
            info!(
                "get_account_at sql(3) time: {} sec",
                execution_time.as_secs_f64()
            );

            row = match result {
                Ok(row) => Some(row),
                Err(clickhouse::error::Error::RowNotFound) => None,
                Err(e) => {
                    println!("get_account_at error: {}", e);
                    return Err(ChError::Db(e));
                }
            };
        }

        if let Some(acc) = row {
            let owner = Pubkey::try_from(acc.owner).map_err(|_| {
                let err = clickhouse::error::Error::Custom(format!(
                    "error convert owner of key: {}",
                    key
                ));
                println!("get_account_at error: {}", err);
                ChError::Db(err)
            })?;

            Ok(Some(Account {
                lamports: acc.lamports,
                data: acc.data,
                owner,
                rent_epoch: acc.rent_epoch,
                executable: acc.executable,
            }))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::unused_self)]
    pub fn get_account_by_sol_sig(
        &self,
        _pubkey: &Pubkey,
        _sol_sig: &[u8; 64],
    ) -> ChResult<Option<Account>> {
        panic!("get_account_by_sol_sig() is not implemented for ClickHouse usage");
    }
}
