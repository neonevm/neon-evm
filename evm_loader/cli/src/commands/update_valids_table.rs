use log::{ debug, info, trace };

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};

use solana_cli::{
    checks::check_account_for_fee,
};

use evm::{H160};

use evm_loader::{
    account_data::AccountData,
};

use crate::{
    account_storage::{
        EmulatorAccountStorage,
    },
    errors::NeonCliError,
    Config,
    NeonCliResult,
};


pub fn execute(config: &Config, ether_address: H160) -> NeonCliResult {
    trace!("Enter execution for address {:?}", ether_address);

    EmulatorAccountStorage::get_account_from_solana(config, &ether_address)
        .ok_or(NeonCliError::AccountNotFoundAtAddress(ether_address))
        .and_then(|(account,_,_)| {
            info!("account: {:?}", account);

            AccountData::unpack(&account.data)
                .map_err(NeonCliError::from)
        })
        .and_then(|account_data| {
            info!("account data: {:?}", account_data);

            account_data.get_account()
                .map(|account|account.code_account)
                .map_err(NeonCliError::from)
        })
        .and_then(|code_account|
            if code_account == Pubkey::new_from_array([0_u8; 32]) {
                Err(NeonCliError::CodeAccountNotFound(ether_address))
            } else {
                Ok(code_account)
            }
        )
        .and_then(|code_account| {
            info!("code account: {:?}", code_account);

            let instruction: Instruction =
                Instruction::new_with_bincode(
                    config.evm_loader,
                    &(23),
                    vec![AccountMeta::new(code_account, false)]
                );

            let finalize_message = Message::new(&[instruction], Some(&config.signer.pubkey()));

            config.rpc_client.get_recent_blockhash()
                .map(|(blockhash,fee_calculator)|(finalize_message,blockhash,fee_calculator))
                .map_err(NeonCliError::from)
        })
        .and_then(|(finalize_message,blockhash,fee_calculator)| {
            info!("fee_calculator: {:?}", fee_calculator);

            check_account_for_fee(&config.rpc_client, &config.signer.pubkey(), &fee_calculator, &finalize_message)
                .map(|_|(finalize_message,blockhash))
                .map_err(NeonCliError::from)
        })
        .and_then(|(finalize_message,blockhash)| {
            info!("blockhash: {:?}", blockhash);

            let mut finalize_tx = Transaction::new_unsigned(finalize_message);
            finalize_tx.try_sign(&[&*config.signer], blockhash)
                .map(|_|finalize_tx)
                .map_err(NeonCliError::from)
        })
        .and_then(|finalize_tx| {
            debug!("signed: {:x?}", finalize_tx);

            config.rpc_client
                .send_and_confirm_transaction_with_spinner(&finalize_tx)
                .map(|_|())
                .map_err(NeonCliError::from)
        })
}

