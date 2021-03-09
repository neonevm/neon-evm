//! Program entrypoint

#![cfg(feature = "program")]
#![cfg(not(feature = "no-entrypoint"))]

//use crate::{error::TokenError, processor::Processor};
//use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use std::convert::TryInto;
use solana_sdk::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::{ProgramResult},
    program_error::{ProgramError}, pubkey::Pubkey,
    program_utils::{limited_deserialize},
    loader_instruction::LoaderInstruction,
    system_instruction::{create_account, create_account_with_seed},
    sysvar::instructions::{load_current_index, load_instruction_at}, 
    program::{invoke_signed, invoke},
    secp256k1_program,
    instruction::Instruction,
    sysvar::instructions
};

//use crate::hamt::Hamt;
use crate::solana_backend::{
    SolanaBackend, solidity_address,
};

use crate::{
//    bump_allocator::BumpAllocator,
    instruction::{EvmInstruction, on_return, on_event},
    account_data::AccountData,
    solidity_account::SolidityAccount,
    transaction::{check_tx, get_check_fields, get_data, make_secp256k1_instruction},
};

use evm::{
//    backend::{MemoryVicinity, MemoryAccount, MemoryBackend, Apply},
    executor::{StackExecutor},
    ExitReason,
};
use primitive_types::{H160, U256};

use std::{alloc::Layout, mem::size_of, ptr::null_mut, usize};
use solana_sdk::entrypoint::HEAP_START_ADDRESS;


use sha3::{Keccak256, Digest};
use primitive_types::H256;
fn keccak256_digest(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(&data).as_slice())
}


const HEAP_LENGTH: usize = 1024*1024;

/// Developers can implement their own heap by defining their own
/// `#[global_allocator]`.  The following implements a dummy for test purposes
/// but can be flushed out with whatever the developer sees fit.
pub struct BumpAllocator;

impl BumpAllocator {
    /// Get occupied memory
    #[inline]
    pub fn occupied() -> usize {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS + HEAP_LENGTH;

        let pos = unsafe{*POS_PTR};
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
    unsafe fn dealloc(&self, _: *mut u8, _layout: Layout) {
        // I'm a bump allocator, I don't free
    }
}


#[cfg(target_arch = "bpf")]
#[global_allocator]
static mut A: BumpAllocator = BumpAllocator;

// Is't need to save for account:
// 1. ether: [u8;20]
// 2. nonce: u8
// 3. trx_count: u128
// 4. signer: pubkey
// 5. code_size: u32
// 6. storage (all remaining space, if code_size not equal zero)

entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {

    let account_info_iter = &mut accounts.iter();

    let instruction = EvmInstruction::unpack(instruction_data)?;
    debug_print!("Instruction parsed");

    let result = match instruction {
        EvmInstruction::CreateAccount {lamports, space, ether, nonce} => {
            let funding_info = next_account_info(account_info_iter)?;
            let program_info = next_account_info(account_info_iter)?;

            debug_print!(&("Ether:".to_owned()+&(hex::encode(ether))+" "+&hex::encode([nonce])));

            let expected_address = Pubkey::create_program_address(&[ether.as_bytes(), &[nonce]], program_id)?;
            if expected_address != *program_info.key {
                return Err(ProgramError::InvalidArgument);
            };

            let program_seeds = [ether.as_bytes(), &[nonce]];
            invoke_signed(
                &create_account(funding_info.key, program_info.key, lamports, AccountData::SIZE as u64 + space, program_id),
                &accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account done");
            
            let mut data = program_info.data.borrow_mut();
            let account_data = AccountData {ether, nonce, trx_count: 0u64, signer: *funding_info.key, code_size: 0u32};
            account_data.pack(&mut data)?;
            Ok(())
        },
        EvmInstruction::CreateAccountWithSeed {base, seed, lamports, space, owner} => {
            let funding_info = next_account_info(account_info_iter)?;
            let created_info = next_account_info(account_info_iter)?;
            let base_info = next_account_info(account_info_iter)?;

            //debug_print!(&("Ether:".to_owned()+&(hex::encode(ether))+" "+&hex::encode([nonce])));
            if base_info.owner != program_id {return Err(ProgramError::InvalidArgument);}
            let caller = SolidityAccount::new(base_info)?;

            let program_seeds = [caller.account_data.ether.as_bytes(), &[caller.account_data.nonce]];
            let seed = std::str::from_utf8(&seed).map_err(|_| ProgramError::InvalidArgument)?;
            debug_print!(&lamports.to_string());
            debug_print!(&space.to_string());
            invoke_signed(
                &create_account_with_seed(funding_info.key, created_info.key, &base, &seed, lamports, space, &owner),
                &accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account_with_seed done");

            Ok(())
        },
        EvmInstruction::Write {offset, bytes} => {
            let program_info = next_account_info(account_info_iter)?;
            if program_info.owner != program_id {
                return Err(ProgramError::InvalidArgument);
            }
            do_write(program_info, offset, &bytes)
        },
        EvmInstruction::Finalize => {
            do_finalize(program_id, accounts)
        },
        EvmInstruction::Call {bytes} => {
            do_call(program_id, accounts, &bytes, None)
        },
        EvmInstruction::CallFromRawEthereumTX  {from_addr, sign, unsigned_msg} => {
            let account_info_iter = &mut accounts.iter();
            let program_info = next_account_info(account_info_iter)?;
            let caller_info = next_account_info(account_info_iter)?;
            let sysvar_info = next_account_info(account_info_iter)?;
            let clock_info = next_account_info(account_info_iter)?;

            let current_instruction = instructions::load_current_index(&sysvar_info.try_borrow_data()?);
            let index = current_instruction - 1;

            match load_instruction_at(index.try_into().unwrap(), &sysvar_info.try_borrow_data()?) {
                Ok(instr) => {
                    if instr.program_id == secp256k1_program::id() {
                        const CHECK_COUNT: u8 = 1;
                        const DATA_START: u16 = 1;
                        const ETH_SIZE: u16 = 20;
                        const SIGN_SIZE: u16 = 65;
                        const ETH_OFFSET: u16 = DATA_START;
                        const SIGN_OFFSET: u16 = ETH_OFFSET + ETH_SIZE;
                        const MSG_OFFSET: u16 = SIGN_OFFSET + SIGN_SIZE;
                    } else {
                        return Err(ProgramError::IncorrectProgramId);
                    }
                },
                Err(err) => {
                    debug_print!("ERR");                    
                    return Err(ProgramError::MissingRequiredSignature);
                }
            }

            let caller = H160::from_slice(from_addr);
            let (nonce, contract, data) = get_data(unsigned_msg);

            let program_eth: H160 = H256::from_slice(Keccak256::digest(&program_info.key.to_bytes()).as_slice()).into();
            let caller_eth: H160 = H256::from_slice(Keccak256::digest(&caller_info.key.to_bytes()).as_slice()).into(); 

            do_call(program_id, accounts, &data, Some( (caller, nonce) ))
        },
        EvmInstruction::CheckEtheriumTX {from_addr, sign, unsigned_msg} => {    
            let account_info_iter = &mut accounts.iter();
            let program_info = next_account_info(account_info_iter)?;
            let caller_info = next_account_info(account_info_iter)?;
            let sysvar_info = next_account_info(account_info_iter)?;
            let clock_info = next_account_info(account_info_iter)?;

            let current_instruction = instructions::load_current_index(&sysvar_info.try_borrow_data()?);
            debug_print!(&(" current instruction: ".to_owned() + &current_instruction.to_string())); 

            let index = current_instruction - 1;
            debug_print!(&("index: ".to_owned() + &index.to_string())); 

            match load_instruction_at(index.try_into().unwrap(), &sysvar_info.try_borrow_data()?) {
                Ok(instr) => {
                    if instr.program_id == secp256k1_program::id() {
                        let sliced = instr.data.as_slice();

                        let reference_instruction = make_secp256k1_instruction(current_instruction, unsigned_msg.len());

                        if reference_instruction != instr.data {
                            debug_print!("wrong keccak instruction data");
                            debug_print!(&("instruction: ".to_owned() + &hex::encode(&instr.data)));    
                            debug_print!(&("reference: ".to_owned() + &hex::encode(&reference_instruction)));    
                            return Err(ProgramError::InvalidInstructionData);
                        }                    
                    } else {
                        debug_print!("wrong program id");
                        return Err(ProgramError::IncorrectProgramId);
                    }
                },
                Err(err) => {
                    debug_print!("Invalid or no instruction to verify the signature");                    
                    return Err(ProgramError::MissingRequiredSignature);
                }
            }

            let caller = H160::from_slice(from_addr);
            let (nonce, contract, data) = get_data(unsigned_msg);

            let program_eth: H160 = H256::from_slice(Keccak256::digest(&program_info.key.to_bytes()).as_slice()).into();
            // let caller_eth: H160 = H256::from_slice(Keccak256::digest(&caller_info.key.to_bytes()).as_slice()).into();
            
            debug_print!(&("caller: ".to_owned() + &caller.to_string()));    
            debug_print!(&("contract: ".to_owned() + &contract.to_string()));
            debug_print!(&("program_eth: ".to_owned() + &program_eth.to_string()));
            // debug_print!(&("caller_eth: ".to_owned() + &caller_eth.to_string()));
            // debug_print!(&format!("caller: {}", &caller.to_string()));
            // debug_print!(&format!("contract: {}", &contract.to_string()));
            // debug_print!(&format!("program_eth: {}", &program_eth.to_string()));
            // debug_print!(&format!("caller_eth: {}", &caller_eth.to_string()));

            if program_eth != contract {
                debug_print!("Add valid account signer");
                return Err(ProgramError::InvalidAccountData);
            }

            if caller_info.owner != program_id {
                debug_print!("Add valid account signer");
                return Err(ProgramError::InvalidAccountData);
            }    

            do_call(program_id, accounts, &data, Some( (caller, nonce) ))
        },
        EvmInstruction::OnReturn {bytes} => {
            Ok(())
        },
        EvmInstruction::OnEvent {address, topics, data} => {
            Ok(())
        },
    };

/*    let result = if program_lamports == 0 {
        do_create_account(program_id, accounts, instruction_data)
    } else {
        let account_type = {program_info.data.borrow()[0]};
        if account_type == 0 {
            let instruction: LoaderInstruction = limited_deserialize(instruction_data)
                .map_err(|_| ProgramError::InvalidInstructionData)?;

            match instruction {
                LoaderInstruction::Write {offset, bytes} => {
                    do_write(program_info, offset, &bytes)
                },
                LoaderInstruction::Finalize => {
                    debug_print!("FinalizeInstruction");
                    do_finalize(program_id, accounts, program_info)
                },
            }
        } else {
            debug_print!("Execute");
            do_execute(program_id, accounts, instruction_data)
        }
    };*/

    debug_print!(&("Total memory occupied: ".to_owned() + &BumpAllocator::occupied().to_string()));
    result
}

fn do_create_account<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>], instruction_data: &[u8]) -> ProgramResult {
    debug_print!("initialize account");
/*
    // If account not initialized - we can only create it...
    let instruction: EvmInstruction = limited_deserialize(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
    match instruction {
        EvmInstruction::CreateAccount {lamports, space, ether, nonce } => {
            let account_info_iter = &mut accounts.iter();
            let program_info = next_account_info(account_info_iter)?;
            let funding_info = next_account_info(account_info_iter)?;
            let expected_address = Pubkey::create_program_address(&[&ether, &[nonce]], program_id)?;
            if expected_address != *program_info.key {
                return Err(ProgramError::InvalidArgument);
            };
            let empty_seeds = [];
            let program_seeds = [&ether[..], &[nonce]];
            invoke_signed(
                &create_account(funding_info.key, program_info.key, lamports, space, program_id),
                &accounts, &[&empty_seeds[..], &program_seeds[..]]
            )?;
            Ok(())
        },
        _ => {Err(ProgramError::InvalidInstructionData)}
    }*/
    Err(ProgramError::InvalidInstructionData)
}

fn do_write(program_info: &AccountInfo, offset: u32, bytes: &[u8]) -> ProgramResult {
    let mut data = program_info.data.borrow_mut();
    let offset = offset as usize;
    if data.len() < offset+AccountData::SIZE + bytes.len() {
        debug_print!("Account data too small");
        return Err(ProgramError::AccountDataTooSmall);
    }
    data[offset+AccountData::SIZE..offset+AccountData::SIZE + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

fn do_finalize<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>]) -> ProgramResult {
    debug_print!("do_finalize");

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };
    let clock_info = next_account_info(account_info_iter)?;
    let rent_info = next_account_info(account_info_iter)?;

    if program_info.owner != program_id {
        return Err(ProgramError::InvalidArgument);
    }

    let mut backend = SolanaBackend::new(program_id, accounts, clock_info)?;
    debug_print!("  backend initialized");

    let caller_ether = get_ether_address(program_id, backend.get_account_by_index(1), caller_info, signer_info, None).ok_or(ProgramError::InvalidArgument)?;

    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    debug_print!("  executor initialized");

    debug_print!("Execute transact_create");

    let code_data = {
        let data = program_info.data.borrow();
        let (_unused, rest) = data.split_at(AccountData::SIZE);
        let (code_len, rest) = rest.split_at(8);
        let code_len = code_len.try_into().ok().map(u64::from_le_bytes).unwrap();
        let (code, _rest) = rest.split_at(code_len as usize);
        code.to_vec()
    };

    let program_account = SolidityAccount::new(program_info)?;

    let exit_reason = executor.transact_create2(
            caller_ether.0,
            U256::zero(),
            code_data,
            H256::default(), usize::max_value()
        );
    debug_print!("  create2 done");

    if exit_reason.is_succeed() {
        debug_print!("Succeed execution");
        let (applies, logs) = executor.deconstruct();
        backend.apply(applies,false, Some(caller_ether))?;
        for log in logs {
            let ix = on_event(program_id, log)?;
            invoke(
                &ix,
                &accounts
            )?;
        }
        Ok(())
    } else {
        debug_print!("Not succeed execution");
        Err(ProgramError::InvalidInstructionData)
    }
}

fn do_call<'a>(
        program_id: &Pubkey,
        accounts: &'a [AccountInfo<'a>],
        instruction_data: &[u8],
        from_info: Option<(H160, u64)>,
    ) -> ProgramResult
{
    debug_print!("do_call");

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };
    // let sysvar_info = next_account_info(account_info_iter)?;
    // let clock_info = next_account_info(account_info_iter)?;

    if program_info.owner != program_id {
        return Err(ProgramError::InvalidArgument);
    }

    let mut backend = SolanaBackend::new(program_id, accounts, accounts.last().unwrap())?;
    debug_print!("  backend initialized");

    let caller_ether = get_ether_address(program_id, backend.get_account_by_index(1), caller_info, signer_info, from_info).ok_or(ProgramError::InvalidArgument)?;

    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    debug_print!("Executor initialized");
    let contract = backend.get_account_by_index(0).ok_or(ProgramError::InvalidArgument)?;

    debug_print!(&("   caller: ".to_owned() + &caller_ether.0.to_string()));
    debug_print!(&(" contract: ".to_owned() + &contract.get_ether().to_string()));

    let (exit_reason, result) = executor.transact_call(
            caller_ether.0,
            contract.get_ether(),
            U256::zero(),
            instruction_data.to_vec(),
            usize::max_value()
        );

    debug_print!("Call done");
    debug_print!(match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, logs) = executor.deconstruct();
            backend.apply(applies,false, Some(caller_ether))?;
            for log in logs {
                let ix = on_event(program_id, log)?;
                invoke(
                    &ix,
                    &accounts
                )?;
            }
            debug_print!("Applies done");
            "succeed"
        },
        ExitReason::Error(_) => "error",
        ExitReason::Revert(_) => "revert",
        ExitReason::Fatal(_) => "fatal",
    });
    debug_print!(&hex::encode(&result));
    
    if !exit_reason.is_succeed() {
        debug_print!("Not succeed execution");
        return Err(ProgramError::InvalidInstructionData);
    }

    let ix = on_return(program_id, result)?;
    invoke(
        &ix,
        &accounts
    )?;

    Ok(())
}


fn get_ether_address<'a>(
    program_id: &Pubkey,
    caller_opt: Option<&SolidityAccount<'a>>,
    caller_info: &'a AccountInfo<'a>,
    signer_info: &'a AccountInfo<'a>,
    from_info: Option<(H160, u64)>,
) ->  Option<(H160, bool)>
{

    if caller_info.owner == program_id {
        if caller_opt.is_some() {
            let caller = caller_opt.unwrap();

            let caller_ether = caller.get_ether();
            let caller_nonce = caller.get_nonce();
        
            if from_info.is_none() {
                if caller.account_data.signer != *signer_info.key || !signer_info.is_signer {
                    debug_print!("Add valid account signer");
                    debug_print!(&("   caller signer: ".to_owned() + &caller.account_data.signer.to_string()));
                    debug_print!(&("   signer pubkey: ".to_owned() + &signer_info.key.to_string()));
                    debug_print!(&("is signer signer: ".to_owned() + &signer_info.is_signer.to_string()));
        
                    return None
                }
            } else {
                let (from, nonce) = from_info.unwrap();
                if caller_ether != from {
                    debug_print!("Invalin caller account");
                    debug_print!(&("   caller addres: ".to_owned() + &caller_ether.to_string()));
                    debug_print!(&("     from addres: ".to_owned() + &from.to_string()));
        
                    return None
                }
                if caller_nonce != nonce {
                    debug_print!("Invalin Ethereum transaction nonce");
                    debug_print!(&("     tx nonce: ".to_owned() + &nonce.to_string()));
                    debug_print!(&("    acc nonce: ".to_owned() + &caller_nonce.to_string()));

                    return None
                }
            }
    
            Some ( ( caller_ether, true) )

        } else {
            None
        }
    } else {
        if from_info.is_some() {
            debug_print!("Sender must be Ethereum account. This method is not allowed for Solana accounts.");

            return None
        }
        if !caller_info.is_signer {
            debug_print!("Caller mast be signer");
            debug_print!(&("Caller pubkey: ".to_owned() + &caller_info.key.to_string()));

            return None
        }

        Some ( ( H256::from_slice(Keccak256::digest(&caller_info.key.to_bytes()).as_slice()).into(), false) )
    }
}

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

