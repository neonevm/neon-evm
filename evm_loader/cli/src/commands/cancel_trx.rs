use log::{info};

use solana_sdk::{
    incinerator,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use evm_loader::{
    account::State,
};

use crate::{
    account_storage::account_info,
    Config,
    NeonCliResult,
};


pub fn execute(
    config: &Config,
    storage_account: &Pubkey,
) -> NeonCliResult {
    let mut acc = config.rpc_client.get_account(storage_account)?;
    let storage_info = account_info(storage_account, &mut acc);
    let storage = State::from_account(&config.evm_loader, &storage_info)?;

    let operator = &config.signer.pubkey();

    let mut accounts_meta : Vec<AccountMeta> = vec![
        AccountMeta::new(*storage_account, false),              // State account
        AccountMeta::new(*operator, true),                      // Operator
        AccountMeta::new(incinerator::id(), false),             // Incinerator
    ];

    let remaining_accounts = storage.accounts()?;
    for (writable, key) in remaining_accounts {
        if writable {
            accounts_meta.push(AccountMeta::new(key, false));
        } else {
            accounts_meta.push(AccountMeta::new_readonly(key, false));
        }
    }
    for meta in &accounts_meta {
        info!("\t{:?}", meta);
    }

    let cancel_with_nonce_instruction = Instruction::new_with_bincode(
        config.evm_loader, &(0x15_u8, storage.transaction_hash), accounts_meta
    );

    let instructions = vec![cancel_with_nonce_instruction];

    crate::send_transaction(config, &instructions)?;

    Ok(())
}

