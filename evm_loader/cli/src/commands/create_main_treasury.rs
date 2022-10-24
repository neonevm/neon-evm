use log::{info};
use crate::{
    Config,
    errors::NeonCliError,
};

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
    bpf_loader_upgradeable,
    system_program,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use spl_token;
use spl_token::native_mint;

use evm_loader::account::MainTreasury;

pub fn execute(
    config: &Config,
) -> Result<(), NeonCliError> {
    let program_data = Pubkey::find_program_address(&[config.evm_loader.as_ref()], &bpf_loader_upgradeable::id()).0;
    let program_upgrade_auth = config.signer.pubkey();

    let main_balance_address = MainTreasury::address(&config.evm_loader).0;

    info!("Main pool balance: {}", main_balance_address);

    let mut message = Message::new(
        &[
            Instruction::new_with_bincode(
                config.evm_loader,
                &(0x29_u8),   // evm_loader::instruction::EvmInstruction::CreateMainTreasury
                vec![
                    AccountMeta::new(main_balance_address, false),
                    AccountMeta::new_readonly(program_data, false),
                    AccountMeta::new_readonly(program_upgrade_auth, false),
                    AccountMeta::new_readonly(spl_token::id(), false),
                    AccountMeta::new_readonly(system_program::id(), false),
                    AccountMeta::new_readonly(native_mint::id(), false),
                    AccountMeta::new(config.signer.pubkey(), true),
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

    Ok(())
}

