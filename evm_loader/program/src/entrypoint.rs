//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

#[cfg(not(feature = "emergency"))]
use crate::{instruction, instruction::EvmInstruction};

entrypoint!(process_instruction);

#[cfg(feature = "emergency")]
fn process_instruction<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction_data: &[u8],
) -> ProgramResult {
    Err!(ProgramError::InvalidInstructionData; "Emergency image: all instructions are rejected")
}

#[cfg(not(feature = "emergency"))]
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    let (tag, instruction) = instruction_data.split_first().ok_or_else(
        || E!(ProgramError::InvalidInstructionData; "Invalid instruction - {:?}", instruction_data),
    )?;

    match EvmInstruction::parse(tag)? {
        EvmInstruction::HolderCreate => {
            instruction::account_holder_create::process(program_id, accounts, instruction)
                .map_err(ProgramError::from)
        }
        EvmInstruction::HolderDelete => {
            instruction::account_holder_delete::process(program_id, accounts, instruction)
                .map_err(ProgramError::from)
        }
        EvmInstruction::HolderWrite => {
            instruction::account_holder_write::process(program_id, accounts, instruction)
                .map_err(ProgramError::from)
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
            .map_err(ProgramError::from)
        }
        EvmInstruction::TransactionExecuteFromAccount => {
            instruction::transaction_execute_from_account::process(
                program_id,
                accounts,
                instruction,
            )
            .map_err(ProgramError::from)
        }
        EvmInstruction::TransactionStepFromInstruction => {
            instruction::transaction_step_from_instruction::process(
                program_id,
                accounts,
                instruction,
            )
            .map_err(ProgramError::from)
        }
        EvmInstruction::TransactionStepFromAccount => {
            instruction::transaction_step_from_account::process(program_id, accounts, instruction)
                .map_err(ProgramError::from)
        }
        EvmInstruction::TransactionStepFromAccountNoChainId => {
            instruction::transaction_step_from_account_no_chainid::process(
                program_id,
                accounts,
                instruction,
            )
            .map_err(ProgramError::from)
        }
        EvmInstruction::CreateAccountV03 => {
            instruction::account_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::CollectTreasure => {
            instruction::collect_treasury::process(program_id, accounts, instruction)
        }
        EvmInstruction::CreateMainTreasury => {
            instruction::create_main_treasury::process(program_id, accounts, instruction)
        }
        EvmInstruction::AccountBlockAdd => {
            instruction::account_block_add::process(program_id, accounts, instruction)
                .map_err(ProgramError::from)
        }
    }
}
