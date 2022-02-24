use log::{info};

use solana_sdk::{
    commitment_config::{CommitmentConfig},
    incinerator,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
    sysvar,
    compute_budget,
};

use evm_loader::{
    account_data::AccountData,
    config::{
        COMPUTE_BUDGET_UNITS,
        COMPUTE_BUDGET_HEAP_FRAME,
    }
};

use crate::{
    account_storage::{
        make_solana_program_address,
    },
    errors::NeonCliError,
    Config,
    NeonCliResult,
};


pub fn execute(
    config: &Config,
    storage_account: &Pubkey,
    token_mint: &Pubkey
) -> NeonCliResult {
    let storage = config.rpc_client.get_account_with_commitment(storage_account, CommitmentConfig::processed()).unwrap().value;

    if let Some(acc) = storage {
        if acc.owner != config.evm_loader {
            return Err(NeonCliError::InvalidStorageAccountOwner(acc.owner));
        }
        let data = AccountData::unpack(&acc.data)?;
        let data_end = data.size();
        let storage =
            if let AccountData::Storage(storage) = data {
                storage
            } else {
                return Err(NeonCliError::StorageAccountRequired(data));
            };

        let keys: Vec<Pubkey> = {
            info!("{:?}", storage);
            let accounts_begin = data_end;
            let accounts_end = accounts_begin + storage.accounts_len * 32;
            if acc.data.len() < accounts_end {
                return Err(NeonCliError::AccountDataTooSmall(acc.data.len(),accounts_end));
            };

            acc.data[accounts_begin..accounts_end].chunks_exact(32).map(Pubkey::new).collect()
        };

        let (caller_solana, _) = make_solana_program_address(&storage.caller, &config.evm_loader);
        let (trx_count, _caller_ether, caller_token) = crate::get_ether_account_nonce(config, &caller_solana, token_mint)?;

        let operator = &config.signer.pubkey();
        let operator_token = spl_associated_token_account::get_associated_token_address(operator, token_mint);

        let mut accounts_meta : Vec<AccountMeta> = vec![
            AccountMeta::new(*storage_account, false),              // Storage account
            AccountMeta::new(*operator, true),                      // Operator
            AccountMeta::new(operator_token, false),                // Operator token
            AccountMeta::new(caller_token, false),                  // Caller token
            AccountMeta::new(incinerator::id(), false),             // Incinerator
            AccountMeta::new_readonly(system_program::id(), false), // System
        ];

        let system_accounts : Vec<Pubkey> = vec![
            config.evm_loader,
            *token_mint,
            spl_token::id(),
            spl_associated_token_account::id(),
            sysvar::rent::id(),
            incinerator::id(),
            system_program::id(),
            sysvar::instructions::id(),
        ];

        for key in keys {
            let writable = if system_accounts.contains(&key) {false} else {
                let acc = config.rpc_client.get_account_with_commitment(&key, CommitmentConfig::processed()).unwrap().value;
                if let Some(acc) = acc {
                    if acc.owner == config.evm_loader {
                        matches!(AccountData::unpack(&acc.data)?, AccountData::Account(_))
                    } else {
                        false
                    }
                } else {false}
            };

            if writable {
                accounts_meta.push(AccountMeta::new(key, false));
            } else {
                accounts_meta.push(AccountMeta::new_readonly(key, false));
            }
        }
        for meta in &accounts_meta {
            info!("\t{:?}", meta);
        }

        let instructions = vec![
            compute_budget::request_units(COMPUTE_BUDGET_UNITS),
            compute_budget::request_heap_frame(COMPUTE_BUDGET_HEAP_FRAME),
            Instruction::new_with_bincode(
                config.evm_loader,
                &(21_u8, trx_count),
                accounts_meta
            )
        ];
        crate::send_transaction(config, &instructions)?;

    } else {
        return Err(NeonCliError::AccountNotFound(*storage_account));
    }
    Ok(())
}

