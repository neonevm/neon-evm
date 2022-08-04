use log::{info, warn};
use crate::{
    Config,
    commands::get_neon_elf::read_elf_parameters_from_account,
    errors::NeonCliError,
};

use std::str::FromStr;

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use evm_loader::config::collateral_pool_base;

pub fn execute(
    config: &Config,
) -> Result<(), NeonCliError> {
    let neon_params = read_elf_parameters_from_account(config)?;

    let pool_base = neon_params.get("NEON_POOL_BASE")
        .and_then(|value| Pubkey::from_str(value.as_str()).ok())
        .ok_or(NeonCliError::IncorrectProgram(config.evm_loader))?;

    let pool_count: u32 = neon_params.get("NEON_POOL_COUNT")
        .and_then(|value| value.parse().ok())
        .ok_or(NeonCliError::IncorrectProgram(config.evm_loader))?;

    let main_balance_address = Pubkey::create_with_seed(&pool_base, collateral_pool_base::MAIN_BALANCE_SEED, &spl_token::id())?;

    info!("NEON_POOL_BASE: {}", pool_base);
    info!("Main pool balance: {}", main_balance_address);

    for i in 0..pool_count {
        let aux_balance_seed = format!("{}{}", collateral_pool_base::PREFIX, i);
        let aux_balance_address = Pubkey::create_with_seed(&pool_base, &aux_balance_seed, &config.evm_loader)?;

        if let Some(aux_balance_account) = config.rpc_client.get_account_with_commitment(&aux_balance_address, config.commitment)?.value {
            let minimal_balance = config.rpc_client.get_minimum_balance_for_rent_exemption(aux_balance_account.data.len())?;
            let available_lamports = aux_balance_account.lamports.saturating_sub(minimal_balance);
            if available_lamports > 0 {
                info!("{:4}: collect {} lamports from {}", i, available_lamports, aux_balance_address);
                let mut message = Message::new(
                    &[
                        Instruction::new_with_bincode(
                            config.evm_loader,
                            &(30_u8, i),
                            vec![
                                AccountMeta::new(main_balance_address, false),
                                AccountMeta::new(aux_balance_address, false),
                            ],
                        ),
                    ],
                    Some(&config.signer.pubkey())
                );
                let blockhash = config.rpc_client.get_latest_blockhash()?;
                message.recent_blockhash = blockhash;

                check_account_for_fee(&config.rpc_client, &config.signer.pubkey(), &message)?;

                let mut trx = Transaction::new_unsigned(message);
                trx.try_sign(&[&*config.signer], blockhash)?;
                config.rpc_client.send_and_confirm_transaction_with_spinner(&trx)?;
            } else {
                info!("{:4}: skip account {}", i, aux_balance_address);
            }
        } else {
            warn!("{:4}: not found account {}", i, aux_balance_address);
        }
    }

    Ok(())
}

