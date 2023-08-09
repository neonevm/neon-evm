use crate::types::{PubkeyBase58, TxParams};
use ethnum::U256;
use evm_loader::evm::tracing::event_listener::trace::{TraceCallConfig, TraceConfig};
use evm_loader::types::Address;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct GetEtherRequest {
    pub ether: Address,
    pub slot: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct GetStorageAtRequest {
    pub contract_id: Address,
    pub index: U256,
    pub slot: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct TxParamsRequestModel {
    pub sender: Address,
    pub contract: Option<Address>,
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
}

impl From<TxParamsRequestModel> for TxParams {
    fn from(model: TxParamsRequestModel) -> Self {
        Self {
            nonce: None,
            from: model.sender,
            to: model.contract,
            data: model.data,
            value: model.value,
            gas_limit: model.gas_limit,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct EmulationParamsRequestModel {
    pub token_mint: Option<PubkeyBase58>,
    pub chain_id: Option<u64>,
    pub max_steps_to_execute: u64,
    pub cached_accounts: Option<Vec<Address>>,
    pub solana_accounts: Option<Vec<PubkeyBase58>>,
}

impl EmulationParamsRequestModel {
    #[allow(unused)]
    pub fn new(
        token_mint: Option<Pubkey>,
        chain_id: Option<u64>,
        max_steps_to_execute: u64,
        cached_accounts: Option<Vec<Address>>,
        solana_accounts: Option<Vec<Pubkey>>,
    ) -> EmulationParamsRequestModel {
        let token_mint = token_mint.map(Into::into);
        let solana_accounts = solana_accounts.map(|vec| vec.into_iter().map(Into::into).collect());

        Self {
            token_mint,
            chain_id,
            max_steps_to_execute,
            cached_accounts,
            solana_accounts,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct EmulateRequestModel {
    #[serde(flatten)]
    pub tx_params: TxParamsRequestModel,
    #[serde(flatten)]
    pub emulation_params: EmulationParamsRequestModel,
    pub slot: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct EmulateHashRequestModel {
    #[serde(flatten)]
    pub emulation_params: EmulationParamsRequestModel,
    pub hash: String,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct TraceRequestModel {
    #[serde(flatten)]
    pub emulate_request: EmulateRequestModel,
    pub trace_call_config: Option<TraceCallConfig>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct TraceHashRequestModel {
    #[serde(flatten)]
    pub emulate_hash_request: EmulateHashRequestModel,
    pub trace_config: Option<TraceConfig>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct TraceNextBlockRequestModel {
    #[serde(flatten)]
    pub emulation_params: EmulationParamsRequestModel,
    pub slot: u64,
    pub trace_config: Option<TraceConfig>,
}
