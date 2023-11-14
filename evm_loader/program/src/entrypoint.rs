//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{instruction, instruction::EvmInstruction};

entrypoint!(process_instruction);

#[cfg(feature = "emergency")]
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    assert!(crate::check_id(program_id));

    let (tag, instruction) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match EvmInstruction::parse(tag)? {
        EvmInstruction::ConfigGetChainCount => {
            instruction::config_get_chain_count::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetChainInfo => {
            instruction::config_get_chain_info::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetEnvironment => {
            instruction::config_get_environment::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyCount => {
            instruction::config_get_property_count::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyByIndex => {
            instruction::config_get_property_by_index::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyByName => {
            instruction::config_get_property_by_name::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetStatus => {
            instruction::config_get_status::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetVersion => {
            instruction::config_get_version::process(program_id, accounts, instruction)
        }
        _ => {
            solana_program::msg!("Emergency image: all instructions are rejected");
            Err(ProgramError::InvalidInstructionData.into())
        }
    }
    .map_err(ProgramError::from)
}

#[cfg(not(feature = "emergency"))]
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    use crate::error::Error;

    assert!(crate::check_id(program_id));

    let (tag, instruction) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match EvmInstruction::parse(tag)? {
        EvmInstruction::HolderCreate => {
            instruction::account_holder_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::HolderDelete => {
            instruction::account_holder_delete::process(program_id, accounts, instruction)
        }
        EvmInstruction::HolderWrite => {
            instruction::account_holder_write::process(program_id, accounts, instruction)
        }
        EvmInstruction::DepositV03 => {
            instruction::neon_tokens_deposit::process(program_id, accounts, instruction)
        }
        EvmInstruction::Cancel => {
            instruction::transaction_cancel::process(program_id, accounts, instruction)
        }
        EvmInstruction::TransactionExecuteFromInstruction => {
            instruction::transaction_execute_from_instruction::process(
                program_id,
                accounts,
                instruction,
            )
        }
        EvmInstruction::TransactionExecuteFromAccount => {
            instruction::transaction_execute_from_account::process(
                program_id,
                accounts,
                instruction,
            )
        }
        EvmInstruction::TransactionStepFromInstruction => {
            instruction::transaction_step_from_instruction::process(
                program_id,
                accounts,
                instruction,
            )
        }
        EvmInstruction::TransactionStepFromAccount => {
            instruction::transaction_step_from_account::process(program_id, accounts, instruction)
        }
        EvmInstruction::TransactionStepFromAccountNoChainId => {
            instruction::transaction_step_from_account_no_chainid::process(
                program_id,
                accounts,
                instruction,
            )
        }
        EvmInstruction::CollectTreasure => {
            instruction::collect_treasury::process(program_id, accounts, instruction)
                .map_err(Error::from)
        }
        EvmInstruction::CreateMainTreasury => {
            instruction::create_main_treasury::process(program_id, accounts, instruction)
                .map_err(Error::from)
        }
        EvmInstruction::AccountBlockAdd => {
            instruction::account_block_add::process(program_id, accounts, instruction)
        }
        EvmInstruction::AccountCreateBalance => {
            instruction::account_create_balance::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetChainCount => {
            instruction::config_get_chain_count::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetChainInfo => {
            instruction::config_get_chain_info::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetEnvironment => {
            instruction::config_get_environment::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyCount => {
            instruction::config_get_property_count::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyByIndex => {
            instruction::config_get_property_by_index::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetPropertyByName => {
            instruction::config_get_property_by_name::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetStatus => {
            instruction::config_get_status::process(program_id, accounts, instruction)
        }
        EvmInstruction::ConfigGetVersion => {
            instruction::config_get_version::process(program_id, accounts, instruction)
        }
    }
    .map_err(ProgramError::from)
}
