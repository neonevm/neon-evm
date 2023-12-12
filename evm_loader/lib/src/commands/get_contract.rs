use evm_loader::{
    account::{legacy::LegacyEtherData, ContractAccount},
    types::Address,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::{account_storage::account_info, rpc::Rpc, NeonResult};

use serde_with::{hex::Hex, serde_as, DisplayFromStr};

use super::get_config::{BuildConfigSimulator, ChainInfo};

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct GetContractResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub solana_address: Pubkey,
    pub chain_id: Option<u64>,
    #[serde_as(as = "Hex")]
    pub code: Vec<u8>,
}

impl GetContractResponse {
    pub fn empty(solana_address: Pubkey) -> Self {
        Self {
            solana_address,
            chain_id: None,
            code: vec![],
        }
    }
}

fn find_legacy_chain_id(chains: &[ChainInfo]) -> u64 {
    for chain in chains {
        if chain.name == "neon" {
            return chain.id;
        }
    }

    unreachable!()
}

fn read_account(
    program_id: &Pubkey,
    legacy_chain_id: u64,
    solana_address: Pubkey,
    account: Option<Account>,
) -> NeonResult<GetContractResponse> {
    let Some(mut account) = account else {
        return Ok(GetContractResponse::empty(solana_address));
    };

    let account_info = account_info(&solana_address, &mut account);
    let (chain_id, code) =
        if let Ok(contract) = ContractAccount::from_account(program_id, account_info.clone()) {
            (Some(contract.chain_id()), contract.code().to_vec())
        } else if let Ok(contract) = LegacyEtherData::from_account(program_id, &account_info) {
            if contract.code_size > 0 || contract.generation > 0 {
                let code = contract.read_code(&account_info);
                (Some(legacy_chain_id), code)
            } else {
                (None, vec![])
            }
        } else {
            (None, vec![])
        };

    Ok(GetContractResponse {
        solana_address,
        chain_id,
        code,
    })
}

pub async fn execute(
    rpc: &(impl Rpc + BuildConfigSimulator),
    program_id: &Pubkey,
    accounts: &[Address],
) -> NeonResult<Vec<GetContractResponse>> {
    let chain_ids = super::get_config::read_chains(rpc, *program_id).await?;
    let legacy_chain_id = find_legacy_chain_id(&chain_ids);

    let pubkeys: Vec<_> = accounts
        .iter()
        .map(|a| a.find_solana_address(program_id).0)
        .collect();

    let accounts = rpc.get_multiple_accounts(&pubkeys).await?;

    let mut result = Vec::with_capacity(accounts.len());
    for (key, account) in pubkeys.into_iter().zip(accounts) {
        let response = read_account(program_id, legacy_chain_id, key, account)?;
        result.push(response);
    }

    Ok(result)
}
