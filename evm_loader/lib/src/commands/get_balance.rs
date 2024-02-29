use ethnum::U256;
use evm_loader::account::legacy::LegacyEtherData;
use evm_loader::account::BalanceAccount;
use serde::{Deserialize, Serialize};
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::{account_storage::account_info, rpc::Rpc, types::BalanceAddress, NeonResult};

use serde_with::{serde_as, DisplayFromStr};

use super::get_config::{BuildConfigSimulator, ChainInfo};

#[derive(Debug, Serialize, Deserialize)]
pub enum BalanceStatus {
    Ok,
    Legacy,
    Empty,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct GetBalanceResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub solana_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub contract_solana_address: Pubkey,
    pub trx_count: u64,
    pub balance: U256,
    pub status: BalanceStatus,
}

impl GetBalanceResponse {
    pub fn empty(program_id: &Pubkey, address: &BalanceAddress) -> Self {
        Self {
            solana_address: address.find_pubkey(program_id),
            contract_solana_address: address.find_contract_pubkey(program_id),
            trx_count: 0,
            balance: U256::ZERO,
            status: BalanceStatus::Empty,
        }
    }
}

fn read_account(
    program_id: &Pubkey,
    address: &BalanceAddress,
    mut account: Account,
) -> NeonResult<GetBalanceResponse> {
    let solana_address = address.find_pubkey(program_id);

    let account_info = account_info(&solana_address, &mut account);
    let balance_account = BalanceAccount::from_account(program_id, account_info)?;

    Ok(GetBalanceResponse {
        solana_address,
        contract_solana_address: address.find_contract_pubkey(program_id),
        trx_count: balance_account.nonce(),
        balance: balance_account.balance(),
        status: BalanceStatus::Ok,
    })
}

fn read_legacy_account(
    program_id: &Pubkey,
    address: &BalanceAddress,
    mut account: Account,
) -> NeonResult<GetBalanceResponse> {
    let solana_address = address.find_pubkey(program_id);
    let contract_solana_address = address.find_contract_pubkey(program_id);

    let account_info = account_info(&contract_solana_address, &mut account);
    let balance_account = LegacyEtherData::from_account(program_id, &account_info)?;

    Ok(GetBalanceResponse {
        solana_address,
        contract_solana_address,
        trx_count: balance_account.trx_count,
        balance: balance_account.balance,
        status: BalanceStatus::Legacy,
    })
}

fn is_legacy_chain_id(id: u64, chains: &[ChainInfo]) -> bool {
    for chain in chains {
        if chain.name == "neon" {
            return id == chain.id;
        }
    }

    false
}

pub async fn execute(
    rpc: &(impl Rpc + BuildConfigSimulator),
    program_id: &Pubkey,
    address: &[BalanceAddress],
) -> NeonResult<Vec<GetBalanceResponse>> {
    let chain_ids = super::get_config::read_chains(rpc, *program_id).await?;

    let pubkeys: Vec<_> = address.iter().map(|a| a.find_pubkey(program_id)).collect();
    let accounts = rpc.get_multiple_accounts(&pubkeys).await?;

    let mut result = Vec::with_capacity(accounts.len());
    for (key, account) in address.iter().zip(accounts) {
        let response = if let Some(account) = account {
            read_account(program_id, key, account)?
        } else if is_legacy_chain_id(key.chain_id, &chain_ids) {
            let contract_pubkey = key.find_contract_pubkey(program_id);
            if let Some(contract_account) = rpc.get_account(&contract_pubkey).await?.value {
                read_legacy_account(program_id, key, contract_account)?
            } else {
                GetBalanceResponse::empty(program_id, key)
            }
        } else {
            GetBalanceResponse::empty(program_id, key)
        };

        result.push(response);
    }

    Ok(result)
}
