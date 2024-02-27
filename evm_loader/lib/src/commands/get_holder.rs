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

    #[serde_as(as = "Option<Vec<DisplayFromStr>>")]
    pub accounts: Option<Vec<Pubkey>>,
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
            let accounts = state.accounts().copied().collect();

            Ok(GetHolderResponse {
                status: Status::Active,
                len: Some(data_len),
                owner: Some(state.owner()),
                tx: Some(state.trx().hash()),
                chain_id: state.trx().chain_id(),
                accounts: Some(accounts),
            })
        }
        _ => Err(ProgramError::InvalidAccountData.into()),
    }
}

pub async fn execute(
    rpc: &impl Rpc,
    program_id: &Pubkey,
    address: Pubkey,
) -> NeonResult<GetHolderResponse> {
    let response = rpc.get_account(&address).await?;
    let Some(mut account) = response.value else {
        return Ok(GetHolderResponse::empty());
    };

    let info = account_info(&address, &mut account);
    Ok(read_holder(program_id, info).unwrap_or_else(GetHolderResponse::error))
}
