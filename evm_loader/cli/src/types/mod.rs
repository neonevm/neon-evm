mod indexer_db;
#[allow(clippy::all)]
pub mod trace;
mod tracer_ch_db;

pub use indexer_db::IndexerDb;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tokio::{runtime::Runtime, task::block_in_place};
pub use tracer_ch_db::{ChError, ChResult, ClickHouseDb as TracerDb};

use {
    ethnum::U256,
    evm_loader::types::Address,
    postgres::NoTls,
    thiserror::Error,
    // tokio::task::block_in_place,
    tokio_postgres::{connect, Client},
};

type Bytes = Vec<u8>;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ChDbConfig {
    pub clickhouse_url: Vec<String>,
    pub clickhouse_user: Option<String>,
    pub clickhouse_password: Option<String>,
    pub indexer_host: String,
    pub indexer_port: String,
    pub indexer_database: String,
    pub indexer_user: String,
    pub indexer_password: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TxParams {
    pub from: Address,
    pub to: Option<Address>,
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
}

pub fn do_connect(
    host: &String,
    port: &String,
    db: &String,
    user: &String,
    pass: &String,
) -> Client {
    let authority = format!("host={host} port={port} dbname={db} user={user} password={pass}");

    let mut attempt = 0;
    let mut result = None;

    while attempt < 3 {
        result = block(|| async { connect(&authority, NoTls).await }).ok();
        if result.is_some() {
            break;
        }
        attempt += 1;
    }

    let (client, connection) = result.expect("error to set DB connection");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });
    client
}

lazy_static! {
    pub static ref RT: Runtime = tokio::runtime::Runtime::new().unwrap();
}

pub fn block<F, Fu, R>(f: F) -> R
where
    F: FnOnce() -> Fu,
    Fu: std::future::Future<Output = R>,
{
    RT.block_on(f())
}

#[derive(Error, Debug)]
pub enum PgError {
    #[error("postgres: {}", .0)]
    Db(#[from] tokio_postgres::Error),
    #[error("Custom: {0}")]
    Custom(String),
}

pub type PgResult<T> = std::result::Result<T, PgError>;
