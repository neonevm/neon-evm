//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]


use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::{ProgramResult},
    program_error::ProgramError,
    pubkey::Pubkey,
};


#[cfg(not(feature = "emergency"))]
use crate::{
    instruction,
    instruction::EvmInstruction,
    allocator::BumpAllocator,
};



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
    let (tag, instruction) = instruction_data.split_first()
        .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Invalid instruction - {:?}", instruction_data))?;

    let result = match EvmInstruction::parse(tag)? {
        EvmInstruction::DeleteHolderOrStorageAccount => {
            instruction::account_delete_holder_storage::process(program_id, accounts, instruction)
        }
        EvmInstruction::ERC20CreateTokenAccount => {
            instruction::erc20_account_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::DepositV03 => {
            instruction::neon_tokens_deposit::process(program_id, accounts, instruction)
        }
        EvmInstruction::WriteHolder => {
            instruction::transaction_write_to_holder::process(program_id, accounts, instruction)
        }
        EvmInstruction::CancelWithNonce => {
            instruction::transaction_cancel::process(program_id, accounts, instruction)
        }
        EvmInstruction::CallFromRawEthereumTX => {
            instruction::transaction_execute_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::PartialCallFromRawEthereumTxV03 => {
            instruction::transaction_begin_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::ExecuteTrxFromAccountDataIterativeV03 => {
            instruction::transaction_begin_from_account::process(program_id, accounts, instruction)
        }
        EvmInstruction::ContinueV03 => {
            instruction::transaction_continue::process(program_id, accounts, instruction)
        }
        EvmInstruction::PartialCallOrContinueFromRawEthereumTX => {
            instruction::transaction_step_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinue => {
            instruction::transaction_step_from_account::process(program_id, accounts, instruction)
        }
        EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinueNoChainId => {
            instruction::transaction_step_from_account_no_chainid::process(program_id, accounts, instruction)
        }
        EvmInstruction::CreateAccountV03 => {
            instruction::account_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::CollectTreasure => {
            instruction::collect_treasury::process(program_id, accounts, instruction)
        }

        _ => Err!(ProgramError::InvalidInstructionData; "Invalid instruction"),
    };

    solana_program::msg!("Total memory occupied: {}", BumpAllocator::occupied());
    result
}
