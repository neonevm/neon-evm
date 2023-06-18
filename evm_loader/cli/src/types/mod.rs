mod indexer_db;
pub mod request_models;
#[allow(clippy::all)]
pub mod trace;
mod tracer_ch_db;

pub use indexer_db::IndexerDb;
use lazy_static::lazy_static;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::{runtime::Runtime, task::block_in_place};
pub use tracer_ch_db::{ChError, ChResult, ClickHouseDb as TracerDb};

use {
    ethnum::U256,
    evm_loader::types::Address,
    postgres::NoTls,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
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
    block_in_place(|| RT.block_on(f()))
}

#[derive(Error, Debug)]
pub enum PgError {
    #[error("postgres: {}", .0)]
    Db(#[from] tokio_postgres::Error),
    #[error("Custom: {0}")]
    Custom(String),
}

pub type PgResult<T> = std::result::Result<T, PgError>;

#[derive(Debug, Default, Clone, Copy)]
pub struct PubkeyBase58(pub Pubkey);

impl AsRef<Pubkey> for PubkeyBase58 {
    fn as_ref(&self) -> &Pubkey {
        &self.0
    }
}

impl From<Pubkey> for PubkeyBase58 {
    fn from(value: Pubkey) -> Self {
        Self(value)
    }
}

impl From<&Pubkey> for PubkeyBase58 {
    fn from(value: &Pubkey) -> Self {
        Self(*value)
    }
}

impl From<PubkeyBase58> for Pubkey {
    fn from(value: PubkeyBase58) -> Self {
        value.0
    }
}

impl Serialize for PubkeyBase58 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bs58 = bs58::encode(&self.0).into_string();
        serializer.serialize_str(&bs58)
    }
}

impl<'de> Deserialize<'de> for PubkeyBase58 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor;

        impl<'de> serde::de::Visitor<'de> for StringVisitor {
            type Value = Pubkey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string containing json data")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Pubkey::from_str(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(StringVisitor).map(Self)
    }
}
