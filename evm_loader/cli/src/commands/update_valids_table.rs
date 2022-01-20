#[allow(clippy::module_name_repetitions)]
use log::{ debug };

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use evm::{H160};

use evm_loader::{
    account_data::AccountData,
};

use crate::{
    account_storage::{
        EmulatorAccountStorage,
    },
    Config,
    CommandResult,
};


pub fn command_update_valids_table(
    config: &Config,
    ether_address: &H160,
) -> CommandResult {
    let account_data = if let Some((account, _, _)) = EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        AccountData::unpack(&account.data)?
    } else {
        return Err(format!("Account not found {:#x}", ether_address).into());
    };

    let code_account = account_data.get_account()?.code_account;
    if code_account == Pubkey::new_from_array([0_u8; 32]) {
        return Err(format!("Code account not found {:#x}", ether_address).into());
    }

    let instruction = Instruction::new_with_bincode(
        config.evm_loader,
        &(23),
        vec![AccountMeta::new(code_account, false)]
    );

    let finalize_message = Message::new(&[instruction], Some(&config.signer.pubkey()));
    let (blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &fee_calculator,
        &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);
    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    Ok(())
}

