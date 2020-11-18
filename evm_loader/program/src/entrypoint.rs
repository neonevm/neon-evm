//! Program entrypoint

#![cfg(feature = "program")]
#![cfg(not(feature = "no-entrypoint"))]

//use crate::{error::TokenError, processor::Processor};
use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use std::convert::TryInto;
use solana_sdk::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::ProgramResult,
    program_error::{ProgramError, PrintProgramError}, pubkey::Pubkey,
    program_utils::{limited_deserialize},
    loader_instruction::LoaderInstruction,
    info,
};

use crate::hamt::Hamt;
use crate::solana_backend::SolanaBackend;

use evm::backend::{MemoryVicinity, MemoryAccount, MemoryBackend, Apply};
use evm::executor::StackExecutor;
use primitive_types::{H160, H256, U256};
use std::collections::BTreeMap;

fn unpack_loader_instruction(data: &[u8]) -> LoaderInstruction {
    LoaderInstruction::Finalize
}

//fn pubkey_to_address(key: &Pubkey) -> H160 {
//    H256::from_slice(key.as_ref()).into()
//}




entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {


    let instruction: LoaderInstruction = limited_deserialize(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;

    let mut data = program_info.data.borrow_mut();

    if data[0] == 0 {
        match instruction {
            LoaderInstruction::Write {offset, bytes} => {
                return do_write(program_info, &mut data, offset, &bytes);
            },
            LoaderInstruction::Finalize => {
                info!("FinalizeInstruction");
                return do_finalize(accounts, program_info, &mut data);
            },
        }
    } else {
        return do_execute();
    }
    Ok(())
}

fn do_write(program_info: &AccountInfo, data: &mut [u8], offset: u32, bytes: &Vec<u8>) -> ProgramResult {
    let offset = offset as usize;
    if data.len() < offset+1 + bytes.len() {
        info!("Account data too small");
        return Err(ProgramError::AccountDataTooSmall);
    }
    data[offset+1..offset+1 + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

fn do_finalize<'a>(accounts: &'a [AccountInfo<'a>], program_info: &AccountInfo, data: &mut [u8]) -> ProgramResult {

    let backend = SolanaBackend::new(accounts); //MemoryBackend::new(&vicinity, state);
    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);

    //trace!("Execute transact_create");

    //let data = Vec::new();
    //let _ = executor.transact_call(H160::default(), H160::default(), U256::zero(), Vec::new(), usize::max_value(),);
    let exit_reason = executor.transact_create(H160::zero(), U256::zero(), data[1..].to_vec(), usize::max_value());
    if exit_reason.is_succeed() {
        info!("Succeed execution");
    } else {
        info!("Not succeed execution");
    }

    let (_applies, _logs) = executor.deconstruct();
    let hamt = Hamt::new(data);

    Ok(())
}

fn do_execute() -> ProgramResult {
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

