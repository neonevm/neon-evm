use log::{ debug, info, trace };

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    transaction::Transaction,
    compute_budget::ComputeBudgetInstruction,
};

use solana_cli::{
    checks::check_account_for_fee,
};

use evm::{H160};

use evm_loader::{
    account::{EthereumAccount},
    config::{
        COMPUTE_BUDGET_UNITS,
        COMPUTE_BUDGET_HEAP_FRAME,
        REQUEST_UNITS_ADDITIONAL_FEE,
    }
};

use crate::{
    account_storage::{
        EmulatorAccountStorage,
        make_solana_program_address,
        account_info,
    },
    errors::NeonCliError,
    Config,
    NeonCliResult,
};


pub fn execute(config: &Config, ether_address: H160) -> NeonCliResult {
    trace!("Enter execution for address {:?}", ether_address);

    let (mut account, _) = EmulatorAccountStorage::get_account_from_solana(config, &ether_address)
        .ok_or(NeonCliError::AccountNotFoundAtAddress(ether_address))?;
    info!("account: {:?}", account);

    let code_account = {
        let (key, _) = make_solana_program_address(&ether_address, &config.evm_loader);
        let info = account_info(&key, &mut account);
        EthereumAccount::from_account(&config.evm_loader, &info)
            .map_err(NeonCliError::from)
            .map(|a| a.code_account)?
            .ok_or(NeonCliError::CodeAccountNotFound(ether_address))?
    };
    info!("code account: {:?}", code_account);

    let update_valids_table_instruction =
        Instruction::new_with_bincode(
            config.evm_loader,
            &(23_u8), // TODO remove magic number
            vec![AccountMeta::new(code_account, false)]
        );

    let instructions = vec![
        ComputeBudgetInstruction::request_units(COMPUTE_BUDGET_UNITS, REQUEST_UNITS_ADDITIONAL_FEE),
        ComputeBudgetInstruction::request_heap_frame(COMPUTE_BUDGET_HEAP_FRAME),
        update_valids_table_instruction
    ];

    let mut finalize_message = Message::new(&instructions, Some(&config.signer.pubkey()));
    let blockhash = config.rpc_client.get_latest_blockhash()?;
    info!("blockhash: {:?}", blockhash);
    finalize_message.recent_blockhash = blockhash;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &finalize_message
    )?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    Ok(())
}

