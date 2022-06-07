//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]


use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::{ProgramResult},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    instruction,
    instruction::EvmInstruction
};



entrypoint!(process_instruction);

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
        EvmInstruction::DeleteHolderOrStorageAccount => {
            instruction::account_delete_holder_storage::process(program_id, accounts, instruction)
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
        EvmInstruction::WriteHolder => {
            instruction::transaction_write_to_holder::process(program_id, accounts, instruction)
        }
        EvmInstruction::CancelWithNonce => {
            instruction::transaction_cancel::process(program_id, accounts, instruction)
        }
        EvmInstruction::CallFromRawEthereumTX => {
            instruction::transaction_execute_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::PartialCallFromRawEthereumTXv02 => {
            instruction::transaction_begin_from_instruction::process(program_id, accounts, instruction)
        }
        EvmInstruction::ExecuteTrxFromAccountDataIterativeV02 => {
            instruction::transaction_begin_from_account::process(program_id, accounts, instruction)
        },
        EvmInstruction::ContinueV02 => {
            instruction::transaction_continue::process(program_id, accounts, instruction)
        },
        EvmInstruction::PartialCallOrContinueFromRawEthereumTX => {
            instruction::transaction_step_from_instruction::process(program_id, accounts, instruction)
        },
        EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinue => {
            instruction::transaction_step_from_account::process(program_id, accounts, instruction)
        },
        EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinueNoChainId => {
            instruction::transaction_step_from_account_no_chainid::process(program_id, accounts, instruction)
        },
        EvmInstruction::WriteValueToDistributedStorage => {
            instruction::storage_to_v2::write_value_to_distributed_storage::process(program_id, accounts, instruction)
        },
        EvmInstruction::ConvertDataAccountFromV1ToV2 => {
            instruction::storage_to_v2::convert_data_account_from_v1_to_v2::process(program_id, accounts, instruction)
        },
        EvmInstruction::OnReturn | EvmInstruction::OnEvent => { Ok(()) },
        _ => Err!(ProgramError::InvalidInstructionData; "Invalid instruction"),
    };

    solana_program::msg!("Total memory occupied: {}", crate::allocator::BumpAllocator::occupied());
    result
}
