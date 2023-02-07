use clickhouse::Client;
use super::block;
use std::sync::Arc;
use solana_sdk::clock::{UnixTimestamp, Slot};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChError {
    #[error("clickhouse: {}", .0)]
    Db(#[from] clickhouse::error::Error),
    // #[error("Custom: {0}")]
    // Custom (String),
}

pub type ChResult<T> = std::result::Result<T, ChError>;

#[allow(dead_code)]
pub struct ClickHouseDb {
    client: Arc<Client>,
}

#[allow(dead_code)]
impl ClickHouseDb {
    pub fn _new(
        server_url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> ClickHouseDb {
        let client = match (username, password) {
            (None, None | Some(_)) => Client::default().with_url(server_url),
            (Some(user), None) => Client::default().with_url(server_url).with_user(user),
            (Some(user), Some(password)) => Client::default()
                .with_url(server_url)
                .with_user(user)
                .with_password(password),
        };

        ClickHouseDb { client: Arc::new(client) }
    }

    pub fn get_block_time(&self, slot: Slot) -> ChResult<UnixTimestamp> {
        block(|| async {
            let query = "SELECT JSONExtractInt(notify_block_json, 'block_time') FROM events.notify_block_local WHERE (slot = toUInt64(?))";
            self.client
                .query(query)
                .bind(slot)
                .fetch_one::<UnixTimestamp>()
                .await
                .map_err(std::convert::Into::into)
        })
    }

    pub fn get_latest_blockhash(&self) -> ChResult<String> {
        block(|| async {
            let query =
                "SELECT hash FROM events.notify_block_local ORDER BY retrieved_time DESC LIMIT 1";
            self
                .client
                .query(query)
                .fetch_one::<String>()
                .await
                .map_err(std::convert::Into::into)
        })
    }
}