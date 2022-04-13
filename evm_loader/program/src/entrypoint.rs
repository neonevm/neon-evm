//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

use std::{
    alloc::Layout,
    mem::size_of,
    ptr::null_mut,
    usize
};

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::{ProgramResult, HEAP_START_ADDRESS},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    instruction,
    instruction::EvmInstruction
};


const HEAP_LENGTH: usize = 256*1024;

/// Developers can implement their own heap by defining their own
/// `#[global_allocator]`.  The following implements a dummy for test purposes
/// but can be flushed out with whatever the developer sees fit.
pub struct BumpAllocator;

impl BumpAllocator {
    /// Get occupied memory
    #[inline]
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    #[allow(clippy::pedantic)]
    pub fn occupied() -> usize {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS as usize + HEAP_LENGTH;

        let pos = unsafe{*POS_PTR};
        if pos == 0 {0} else {TOP_ADDRESS-pos}
    }
}

unsafe impl std::alloc::GlobalAlloc for BumpAllocator {
    #[inline]
    #[allow(clippy::pedantic)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS as usize + HEAP_LENGTH;
        const BOTTOM_ADDRESS: usize = HEAP_START_ADDRESS as usize + size_of::<*mut u8>();

        let mut pos = *POS_PTR;
        if pos == 0 {
            // First time, set starting position
            pos = TOP_ADDRESS;
        }
        pos = pos.saturating_sub(layout.size());
        pos &= !(layout.align().saturating_sub(1));
        if pos < BOTTOM_ADDRESS {
            return null_mut();
        }

        *POS_PTR = pos;
        pos as *mut u8
    }
    #[inline]
    unsafe fn dealloc(&self, _: *mut u8, _layout: Layout) {
        // I'm a bump allocator, I don't free
    }
}

#[cfg(target_arch = "bpf")]
#[global_allocator]
static mut A: BumpAllocator = BumpAllocator;

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
        EvmInstruction::OnReturn | EvmInstruction::OnEvent => { Ok(()) },
        _ => Err!(ProgramError::InvalidInstructionData; "Invalid instruction"),
    };

    solana_program::msg!("Total memory occupied: {}", &BumpAllocator::occupied());
    result
}
