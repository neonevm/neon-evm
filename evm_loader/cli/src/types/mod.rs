#[allow(clippy::all)]
pub mod trace;
mod indexer_db;
mod tracer_pg_db;
mod tracer_ch_db;

pub use indexer_db::IndexerDb;
// pub use tracer_pg_db::TracerDb;
pub use tracer_ch_db::ClickHouseDb as TracerDb;

use {
    tokio_postgres::{ connect, Client},
    postgres::NoTls,
    tokio::task::block_in_place,
    ethnum::U256,
    evm_loader::types::Address,
    thiserror::Error,
};

type Bytes = Vec<u8>;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DbConfig{
    pub tracer_host: String,
    pub tracer_port: String,
    pub tracer_database: String,
    pub tracer_user: String,
    pub tracer_password: String,
    pub indexer_host: String,
    pub indexer_port: String,
    pub indexer_database: String,
    pub indexer_user: String,
    pub indexer_password: String,
}

#[derive(Clone)]
pub struct TxParams {
    pub from: Address,
    pub to: Option<Address>,
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
}

pub fn do_connect(host: &String, port: &String, db: &String, user: &String, pass: &String) -> Client {
    let authority= format!(
        "host={} port={} dbname={} user={} password={}", host, port, db, user, pass
    );

    let mut attempt = 0;
    let mut result = None;

    while attempt < 3 {
        result = block(|| async {
            connect(&authority, NoTls).await
        }).ok();
        if result.is_some() {
            break;
        }
        attempt += 1;
    }

    let (client, connection) = result.expect("error to set DB connection");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    client
}

pub fn block<F, Fu, R>(f: F) -> R
    where
        F: FnOnce() -> Fu,
        Fu: std::future::Future<Output = R>,
{
    block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(f())
    })
}

#[derive(Error, Debug)]
pub enum PgError {
    #[error("postgres: {}", .0)]
    Db(#[from] tokio_postgres::Error),
    #[error("Custom: {0}")]
    Custom (String),
}

pub type PgResult<T> = std::result::Result<T, PgError>;

