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
        EvmInstruction::CreateAccountV02 => {
            instruction::account_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::HolderCreate => {
            instruction::account_holder_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::HolderDelete => {
            instruction::account_holder_delete::process(program_id, accounts, instruction)
        }
        EvmInstruction::HolderWrite => {
            instruction::account_holder_write::process(program_id, accounts, instruction)
        }
        EvmInstruction::ResizeContractAccount => {
            instruction::account_resize::process(program_id, accounts, instruction)
        }
        EvmInstruction::ERC20CreateTokenAccount => {
            instruction::erc20_account_create::process(program_id, accounts, instruction)
        }
        EvmInstruction::Deposit => {
            instruction::neon_tokens_deposit::process(program_id, accounts, instruction)
        }
        EvmInstruction::MigrateAccount => {
            instruction::migrate_account::process(program_id, accounts, instruction)
        }
        EvmInstruction::UpdateValidsTable => {
            instruction::update_valids_table::process(program_id, accounts, instruction)
        }
        EvmInstruction::Cancel => {
            instruction::transaction_cancel::process(program_id, accounts, instruction)
        }
        EvmInstruction::TransactionExecuteFromInstruction => {
            instruction::transaction_execute_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::TransactionStepFromInstruction => {
            instruction::transaction_step_from_instruction::process(program_id, accounts, instruction)
        },
        EvmInstruction::TransactionStepFromAccount => {
            instruction::transaction_step_from_account::process(program_id, accounts, instruction)
        },
        EvmInstruction::TransactionStepFromAccountNoChainId => {
            instruction::transaction_step_from_account_no_chainid::process(program_id, accounts, instruction)
        },
        EvmInstruction::WriteValueToDistributedStorage => {
            instruction::storage_to_v2::write_value_to_distributed_storage::process(program_id, accounts, instruction)
        },
        EvmInstruction::ConvertDataAccountFromV1ToV2 => {
            instruction::storage_to_v2::convert_data_account_from_v1_to_v2::process(program_id, accounts, instruction)
        },
        EvmInstruction::CollectTreasure => {
            instruction::collect_treasury::process(program_id, accounts, instruction)
        }
    };

    solana_program::msg!("Total memory occupied: {}", BumpAllocator::occupied());
    result
}
