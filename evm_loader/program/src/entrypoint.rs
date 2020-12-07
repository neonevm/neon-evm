//! Program entrypoint

#![cfg(feature = "program")]
#![cfg(not(feature = "no-entrypoint"))]

//use crate::{error::TokenError, processor::Processor};
use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use std::convert::TryInto;
use solana_sdk::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::{ProgramResult, HEAP_START_ADDRESS},
    program_error::{ProgramError, PrintProgramError}, pubkey::Pubkey,
    program_utils::{limited_deserialize},
    loader_instruction::LoaderInstruction,
    info,
};
use std::{alloc::Layout, mem::size_of, ptr::null_mut, usize};

use crate::hamt::Hamt;
use crate::solana_backend::{
    SolanaBackend, solidity_address,
};

use evm::{
    backend::{MemoryVicinity, MemoryAccount, MemoryBackend, Apply},
    executor::{StackExecutor},
    ExitReason,
};
use primitive_types::{H160, H256, U256};
use std::collections::BTreeMap;


const HEAP_LENGTH: usize = 1024*1024;

/// Developers can implement their own heap by defining their own
/// `#[global_allocator]`.  The following implements a dummy for test purposes
/// but can be flushed out with whatever the developer sees fit.
struct BumpAllocator;

#[cfg(target_arch = "bpf")]
#[global_allocator]
static mut A: BumpAllocator = BumpAllocator;

impl BumpAllocator {
    #[inline]
    fn occupied() -> usize {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS + HEAP_LENGTH;
        const BOTTOM_ADDRESS: usize = HEAP_START_ADDRESS + size_of::<*mut u8>();

        let mut pos = unsafe{*POS_PTR};
        if pos == 0 {0} else {TOP_ADDRESS-pos}
    }
}

unsafe impl std::alloc::GlobalAlloc for BumpAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS + HEAP_LENGTH;
        const BOTTOM_ADDRESS: usize = HEAP_START_ADDRESS + size_of::<*mut u8>();

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
    unsafe fn dealloc(&self, _: *mut u8, layout: Layout) {
        // I'm a bump allocator, I don't free
    }
}

entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {


    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;

    let account_type = {program_info.data.borrow()[0]};

    let result = if account_type == 0 {
        let instruction: LoaderInstruction = limited_deserialize(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        match instruction {
            LoaderInstruction::Write {offset, bytes} => {
                do_write(program_info, offset, &bytes)
            },
            LoaderInstruction::Finalize => {
                info!("FinalizeInstruction");
                do_finalize(program_id, accounts, program_info)
            },
        }
    } else {
        info!("Execute");
        do_execute(program_id, accounts, instruction_data)
    };

    info!(&("Total memory occupied: ".to_owned() + &BumpAllocator::occupied().to_string()));
    result
}

fn do_write(program_info: &AccountInfo, offset: u32, bytes: &Vec<u8>) -> ProgramResult {
    let mut data = program_info.data.borrow_mut();
    let offset = offset as usize;
    if data.len() < offset+1 + bytes.len() {
        info!("Account data too small");
        return Err(ProgramError::AccountDataTooSmall);
    }
    data[offset+1..offset+1 + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

fn do_finalize<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>], program_info: &AccountInfo) -> ProgramResult {

    info!("do_finalize");
    let mut backend = SolanaBackend::new(program_id, accounts)?; //MemoryBackend::new(&vicinity, state);
    info!("  backend initialized");

    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    info!("  executor initialized");

    info!("Execute transact_create");

    let code_data = {
        let data = program_info.data.borrow();
        let (_unused, rest) = data.split_at(1);
        let (code_len, rest) = rest.split_at(8);
        let code_len = code_len.try_into().ok().map(u64::from_le_bytes).unwrap();
        let (code, _rest) = rest.split_at(code_len as usize);
        code.to_vec()
    };

    let exit_reason = executor.transact_create2(
            solidity_address(&accounts[1].key),
            U256::zero(),
            code_data,
            program_info.key.to_bytes().into(), usize::max_value()
        );
    info!("  create2 done");

    if exit_reason.is_succeed() {
        info!("Succeed execution");
        let (applies, logs) = executor.deconstruct();
        backend.apply(applies, logs, false)?;
        Ok(())
    } else {
        info!("Not succeed execution");
        Err(ProgramError::InvalidInstructionData)
    }
}

fn do_execute<'a>(
        program_id: &Pubkey,
        accounts: &'a [AccountInfo<'a>],
        instruction_data: &[u8],
    ) -> ProgramResult
{
    let mut backend = SolanaBackend::new(program_id, accounts)?;
    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    info!("Executor initialized");
    info!(&("   caller: ".to_owned() + &backend.get_address_by_index(1).to_string()));
    info!(&(" contract: ".to_owned() + &backend.get_address_by_index(0).to_string()));

    let (exit_reason, result) = executor.transact_call(
            backend.get_address_by_index(1),
            backend.get_address_by_index(0),
            U256::zero(),
            instruction_data.to_vec(),
            usize::max_value()
        );

    info!("Call done");
    info!(match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, logs) = executor.deconstruct();
            backend.apply(applies, logs, false)?;
            info!("Applies done");
            "succeed"
        },
        ExitReason::Error(_) => "error",
        ExitReason::Revert(_) => "revert",
        ExitReason::Fatal(_) => "fatal",
    });
    info!(&hex::encode(&result));
    
    if !exit_reason.is_succeed() {
        info!("Not succeed execution");
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(())
}

//  let mut keyed_accounts_iter = keyed_accounts.iter();
//  let keyed_account = next_keyed_account(&mut keyed_accounts_iter)?;

//    let vicinity = MemoryVicinity {
//        gas_price: U256::zero(),
//        origin: H160::default(),
//        chain_id: U256::zero(),
//        block_hashes: Vec::new(),
//        block_number: U256::zero(),
//        block_coinbase: H160::default(),
//        block_timestamp: U256::zero(),
//        block_difficulty: U256::zero(),
//        block_gas_limit: U256::zero(),
//    };
//    let mut state = BTreeMap::new();
//
//    trace!("Read accounts data");
//
//    let backend = MemoryBackend::new(&vicinity, state);
//    let config = evm::Config::istanbul();
//    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
//
//    trace!("Execute transact_create");

//    let data = Vec::new();
//    let exit_reason = executor.transact_create(H160::zero(), U256::zero(), data, usize::max_value());
//
//    let (_applies, _logs) = executor.deconstruct();
//    let gas = U256::zero();
//    Ok(())
//}




// Pull in syscall stubs when building for non-BPF targets
//#[cfg(not(target_arch = "bpf"))]
//solana_sdk::program_stubs!();

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{program_error::ProgramError, pubkey::Pubkey};

    #[test]
    fn test_write() {
        let program_id = Pubkey::new(&[0; 32]);

        let string = b"letters and such";
        assert_eq!(Ok(()), process_instruction(&program_id, &[], string));

        let emoji = "🐆".as_bytes();
        let bytes = [0xF0, 0x9F, 0x90, 0x86];
        assert_eq!(emoji, bytes);
        assert_eq!(Ok(()), process_instruction(&program_id, &[], &emoji));

        let mut bad_utf8 = bytes;
        bad_utf8[3] = 0xFF; // Invalid UTF-8 byte
        assert_eq!(
            Err(ProgramError::InvalidInstructionData),
            process_instruction(&program_id, &[], &bad_utf8)
        );
    }
}

