use ethnum::U256;
use evm_loader::account::{
    legacy::{
        LegacyFinalizedData, LegacyHolderData, TAG_HOLDER_DEPRECATED,
        TAG_STATE_FINALIZED_DEPRECATED,
    },
    Holder, StateAccount, StateFinalizedAccount, TAG_HOLDER, TAG_STATE, TAG_STATE_FINALIZED,
};
use serde::Serialize;
use solana_sdk::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use std::fmt::Display;

use crate::{account_storage::account_info, rpc::Rpc, NeonResult};

use crate::rpc::RpcEnum;
use serde_with::{hex::Hex, serde_as, skip_serializing_none, DisplayFromStr};

#[derive(Debug, Default, Serialize)]
pub enum Status {
    #[default]
    Empty,
    Error(String),
    Holder,
    Active,
    Finalized,
}

#[serde_as]
#[derive(Debug, Default, Serialize)]
pub struct AccountMeta {
    pub is_writable: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub key: Pubkey,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct GetHolderResponse {
    pub status: Status,
    pub len: Option<usize>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub owner: Option<Pubkey>,

    #[serde_as(as = "Option<Hex>")]
    pub tx: Option<[u8; 32]>,
    pub chain_id: Option<u64>,

    pub gas_price: Option<U256>,
    pub gas_limit: Option<U256>,
    pub gas_used: Option<U256>,

    pub accounts: Option<Vec<AccountMeta>>,
}

impl GetHolderResponse {
    pub fn empty() -> Self {
        Self {
            status: Status::Empty,
            ..Self::default()
        }
    }

    pub fn error<T: Display>(error: T) -> Self {
        Self {
            status: Status::Error(error.to_string()),
            ..Self::default()
        }
    }
}

pub fn read_holder(program_id: &Pubkey, info: AccountInfo) -> NeonResult<GetHolderResponse> {
    let data_len = info.data_len();

    match evm_loader::account::tag(program_id, &info)? {
        TAG_HOLDER => {
            let holder = Holder::from_account(program_id, info)?;
            Ok(GetHolderResponse {
                status: Status::Holder,
                len: Some(data_len),
                owner: Some(holder.owner()),
                tx: Some(holder.transaction_hash()),
                ..GetHolderResponse::default()
            })
        }
        TAG_HOLDER_DEPRECATED => {
            let holder = LegacyHolderData::from_account(program_id, &info)?;
            Ok(GetHolderResponse {
                status: Status::Holder,
                len: Some(data_len),
                owner: Some(holder.owner),
                tx: Some([0u8; 32]),
                ..GetHolderResponse::default()
            })
        }
        TAG_STATE_FINALIZED => {
            let state = StateFinalizedAccount::from_account(program_id, info)?;
            Ok(GetHolderResponse {
                status: Status::Finalized,
                len: Some(data_len),
                owner: Some(state.owner()),
                tx: Some(state.trx_hash()),
                ..GetHolderResponse::default()
            })
        }
        TAG_STATE_FINALIZED_DEPRECATED => {
            let state = LegacyFinalizedData::from_account(program_id, &info)?;
            Ok(GetHolderResponse {
                status: Status::Finalized,
                len: Some(data_len),
                owner: Some(state.owner),
                tx: Some(state.transaction_hash),
                ..GetHolderResponse::default()
            })
        }
        TAG_STATE => {
            let state = StateAccount::from_account(program_id, info)?;
            let accounts = state
                .blocked_accounts()
                .iter()
                .map(|a| AccountMeta {
                    is_writable: a.is_writable,
                    key: a.key,
                })
                .collect();

            Ok(GetHolderResponse {
                status: Status::Active,
                len: Some(data_len),
                owner: Some(state.owner()),
                tx: Some(state.trx_hash()),
                chain_id: Some(state.trx_chain_id()),
                gas_limit: Some(state.trx_gas_limit()),
                gas_price: Some(state.trx_gas_price()),
                gas_used: Some(state.gas_used()),
                accounts: Some(accounts),
            })
        }
        _ => Err(ProgramError::InvalidAccountData.into()),
    }
}

pub async fn execute(
    rpc: &RpcEnum,
    program_id: &Pubkey,
    address: Pubkey,
) -> NeonResult<GetHolderResponse> {
    let response = rpc.get_account(&address).await?;
    let Some(mut account) = response.value else {
        return Ok(GetHolderResponse::empty())
    };

    let info = account_info(&address, &mut account);
    Ok(read_holder(program_id, info).unwrap_or_else(GetHolderResponse::error))
}
