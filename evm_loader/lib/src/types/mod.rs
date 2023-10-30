pub mod request_models;
pub mod tracer_ch_common;
mod tracer_ch_db;

pub use evm_loader::types::Address;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
pub use tracer_ch_db::ClickHouseDb as TracerDb;

use evm_loader::evm::tracing::TraceCallConfig;
use evm_loader::types::hexbytes::HexBytes;
use {
    ethnum::U256,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
};

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ChDbConfig {
    pub clickhouse_url: Vec<String>,
    pub clickhouse_user: Option<String>,
    pub clickhouse_password: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<HexBytes>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TxParams {
    pub nonce: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
    pub access_list: Option<Vec<AccessListItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionParams {
    pub data: Option<HexBytes>,
    pub trace_config: Option<TraceCallConfig>,
}

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
