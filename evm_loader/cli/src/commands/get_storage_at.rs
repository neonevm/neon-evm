use log::{ trace };

use solana_sdk::{ pubkey::Pubkey };

use evm::{H160, U256};

use evm_loader::{
    account::{EthereumContract},
};

use crate::{
    account_storage::{
        account_info,
        EmulatorAccountStorage,
    },
    errors::NeonCliError,
    Config,
    NeonCliResult,
};

pub fn execute(
    config: &Config,
    ether_address: H160,
    index: &U256
) -> NeonCliResult {
    trace!("Enter execution for address {:?}", ether_address);
    match EmulatorAccountStorage::get_account_from_solana(config, &ether_address) {
        Some((_acc, code_account)) => {
            if let Some(mut code_account) = code_account {
                let code_key = Pubkey::default();
                let code_info = account_info(&code_key, &mut code_account);

                let contract = EthereumContract::from_account(&config.evm_loader, &code_info)?;
                let value = contract.extension.storage.find(*index).unwrap_or_default();
                print!("{:#x}", value);
                Ok(())
            } else {
                Err(NeonCliError::CodeAccountRequired(ether_address))
            }
        },
        None => {
            Err(NeonCliError::AccountNotFoundAtAddress(ether_address))
        }
    }
}

