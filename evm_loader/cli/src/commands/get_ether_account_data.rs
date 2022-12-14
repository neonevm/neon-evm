use evm::{H160};

use evm_loader::account::EthereumAccount;

use crate::{
    account_storage::{
        EmulatorAccountStorage,
        account_info,
    },
    Config, NeonCliResult, errors::NeonCliError,
};


pub fn execute (
    config: &Config,
    ether_address: &H160,
) -> NeonCliResult {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        (solana_address, Some(mut acc)) => {
            let acc_info = account_info(&solana_address, &mut acc);
            let account_data = EthereumAccount::from_account(&config.evm_loader, &acc_info).unwrap();
            let contract_code = account_data.contract_data().map_or_else(Vec::new, |c| c.code().to_vec());

            Ok(serde_json::json!({
                "solana_address": solana_address.to_string(),
                "address": account_data.address,
                "bump_seed": account_data.bump_seed,
                "trx_count": account_data.trx_count,
                "rw_blocked": account_data.rw_blocked,
                "balance": account_data.balance.to_string(),
                "generation": account_data.generation,
                "code_size": account_data.code_size,
                "code": hex::encode(contract_code)
            }))
        },
        (solana_address, None) => {
            Err(NeonCliError::AccountNotFound(solana_address))
        }
    }
}

