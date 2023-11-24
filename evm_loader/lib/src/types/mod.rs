pub mod tracer_ch_common;
mod tracer_ch_db;

pub use evm_loader::types::Address;
use evm_loader::types::{StorageKey, Transaction};
use evm_loader::{
    account_storage::AccountStorage,
    types::{AccessListTx, LegacyTx, TransactionPayload},
};
use serde_with::skip_serializing_none;
use solana_sdk::pubkey::Pubkey;
pub use tracer_ch_db::ClickHouseDb as TracerDb;

use crate::tracing::TraceCallConfig;

use ethnum::U256;
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, serde_as, DisplayFromStr, OneOrMany};

use crate::commands::get_config::ChainInfo;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ChDbConfig {
    pub clickhouse_url: Vec<String>,
    pub clickhouse_user: Option<String>,
    pub clickhouse_password: Option<String>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AccessListItem {
    pub address: Address,
    #[serde(rename = "storageKeys")]
    #[serde_as(as = "Vec<Hex>")]
    pub storage_keys: Vec<StorageKey>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Serialize, Deserialize)]
pub struct TxParams {
    pub nonce: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    #[serde_as(as = "Option<Hex>")]
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
    pub gas_price: Option<U256>,
    pub access_list: Option<Vec<AccessListItem>>,
    pub chain_id: Option<u64>,
}

impl TxParams {
    pub async fn into_transaction(self, backend: &impl AccountStorage) -> (Address, Transaction) {
        let chain_id = self.chain_id.unwrap_or_else(|| backend.default_chain_id());

        let origin_nonce = backend.nonce(self.from, chain_id).await;
        let nonce = self.nonce.unwrap_or(origin_nonce);

        let payload = if let Some(access_list) = self.access_list {
            let access_list: Vec<_> = access_list
                .into_iter()
                .map(|a| (a.address, a.storage_keys))
                .collect();

            let access_list_tx = AccessListTx {
                nonce,
                gas_price: U256::ZERO,
                gas_limit: self.gas_limit.unwrap_or(U256::MAX),
                target: self.to,
                value: self.value.unwrap_or_default(),
                call_data: self.data.unwrap_or_default(),
                chain_id: U256::from(chain_id),
                access_list,
                r: U256::ZERO,
                s: U256::ZERO,
                recovery_id: 0,
            };
            TransactionPayload::AccessList(access_list_tx)
        } else {
            let legacy_tx = LegacyTx {
                nonce,
                gas_price: U256::ZERO,
                gas_limit: self.gas_limit.unwrap_or(U256::MAX),
                target: self.to,
                value: self.value.unwrap_or_default(),
                call_data: self.data.unwrap_or_default(),
                chain_id: self.chain_id.map(U256::from),
                v: U256::ZERO,
                r: U256::ZERO,
                s: U256::ZERO,
                recovery_id: 0,
            };
            TransactionPayload::Legacy(legacy_tx)
        };

        let tx = Transaction {
            transaction: payload,
            byte_len: 0,
            hash: [0; 32],
            signed_hash: [0; 32],
        };

        (self.from, tx)
    }
}

impl std::fmt::Debug for TxParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;

        f.write_str(&json)
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateRequest {
    pub tx: TxParams,
    pub step_limit: Option<u64>,
    pub chains: Option<Vec<ChainInfo>>,
    pub trace_config: Option<TraceCallConfig>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub accounts: Vec<Pubkey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateApiRequest {
    #[serde(flatten)]
    pub body: EmulateRequest,
    pub slot: Option<u64>,
    pub tx_index_in_block: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct BalanceAddress {
    pub address: Address,
    pub chain_id: u64,
}

impl BalanceAddress {
    pub fn find_pubkey(&self, program_id: &Pubkey) -> Pubkey {
        self.address
            .find_balance_address(program_id, self.chain_id)
            .0
    }

    pub fn find_contract_pubkey(&self, program_id: &Pubkey) -> Pubkey {
        self.address.find_solana_address(program_id).0
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GetBalanceRequest {
    #[serde_as(as = "OneOrMany<_>")]
    pub account: Vec<BalanceAddress>,
    pub slot: Option<u64>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GetContractRequest {
    #[serde_as(as = "OneOrMany<_>")]
    pub contract: Vec<Address>,
    pub slot: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GetStorageAtRequest {
    pub contract: Address,
    pub index: U256,
    pub slot: Option<u64>,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct GetHolderRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub pubkey: Pubkey,
    pub slot: Option<u64>,
}

#[cfg(test)]
mod tests {
    use crate::types::tracer_ch_common::RevisionMap;

    #[test]
    fn test_build_ranges_empty() {
        let results = Vec::new();
        let exp = Vec::new();
        let res = RevisionMap::build_ranges(results);
        assert_eq!(res, exp);
    }

    #[test]
    fn test_build_ranges_single_element() {
        let results = vec![(1u64, String::from("Rev1"))];
        let exp = vec![(1u64, 2u64, String::from("Rev1"))];
        let res = RevisionMap::build_ranges(results);
        assert_eq!(res, exp);
    }

    #[test]
    fn test_build_ranges_multiple_elements_different_revision() {
        let results = vec![
            (222222222u64, String::from("Rev1")),
            (333333333u64, String::from("Rev2")),
            (444444444u64, String::from("Rev3")),
        ];

        let exp = vec![
            (222222222u64, 333333333u64, String::from("Rev1")),
            (333333334u64, 444444444u64, String::from("Rev2")),
            (444444445u64, 444444445u64, String::from("Rev3")),
        ];
        let res = RevisionMap::build_ranges(results);

        assert_eq!(res, exp);
    }

    #[test]
    fn test_rangemap() {
        let ranges = vec![
            (123456780, 123456788, String::from("Rev1")),
            (123456789, 123456793, String::from("Rev2")),
            (123456794, 123456799, String::from("Rev3")),
        ];
        let map = RevisionMap::new(ranges);

        assert_eq!(map.get(123456779), None); // Below the bottom bound of the first range

        assert_eq!(map.get(123456780), Some(String::from("Rev1"))); // The bottom bound of the first range
        assert_eq!(map.get(123456785), Some(String::from("Rev1"))); // Within the first range
        assert_eq!(map.get(123456788), Some(String::from("Rev1"))); // The top bound of the first range

        assert_eq!(map.get(123456793), Some(String::from("Rev2"))); // The bottom bound of the second range
        assert_eq!(map.get(123456790), Some(String::from("Rev2"))); // Within the second range
        assert_eq!(map.get(123456793), Some(String::from("Rev2"))); // The top bound of the second range

        assert_eq!(map.get(123456799), Some(String::from("Rev3"))); // The bottom bound of the third range
        assert_eq!(map.get(123456795), Some(String::from("Rev3"))); // Within the third range
        assert_eq!(map.get(123456799), Some(String::from("Rev3"))); // The top bound of the third range

        assert_eq!(map.get(123456800), None); // Beyond the top end of the last range
    }
}
