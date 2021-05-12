//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

//use crate::{error::TokenError, processor::Processor};
//use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use std::convert::TryInto;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::{ProgramResult, HEAP_START_ADDRESS},
    program_error::{ProgramError}, pubkey::Pubkey,
    loader_instruction::LoaderInstruction,
    system_instruction::{create_account, create_account_with_seed},
    sysvar::instructions::{load_current_index, load_instruction_at}, 
    program::{invoke_signed, invoke},
    secp256k1_program,
    instruction::Instruction,
    sysvar::instructions,
};
use crate::{
//    bump_allocator::BumpAllocator,
    instruction::{EvmInstruction, on_return, on_event},
    account_data::{AccountData, Account, Contract},
    account_storage::ProgramAccountStorage, 
    solana_backend::{SolanaBackend, AccountStorage},
    solidity_account::SolidityAccount,
    utils::{keccak256_digest, solidity_address},
    transaction::{UnsignedTransaction, get_data, verify_tx_signature, make_secp256k1_instruction, check_secp256k1_instruction},
    executor::{ Machine },
    executor_state::{ ExecutorState, ExecutorSubstate },
    storage_account::{ StorageAccount }
};
use evm::{
    backend::{Backend},
    executor::{StackExecutor},
    CreateScheme,
    ExitReason, ExitFatal, ExitError, ExitSucceed,
};
use primitive_types::{H160, U256, H256};
use std::cell::RefCell;
use std::{alloc::Layout, mem::size_of, ptr::null_mut, usize};



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
        EvmInstruction::CreateAccount {lamports, space: _, ether, nonce} => {
            let funding_info = next_account_info(account_info_iter)?;
            let account_info = next_account_info(account_info_iter)?;

            debug_print!("Ether: {} {}", &(hex::encode(ether)), &hex::encode([nonce]));

            let expected_address = Pubkey::create_program_address(&[ether.as_bytes(), &[nonce]], program_id)?;
            if expected_address != *account_info.key {
                debug_print!("expected_address != *program_info.key");
                return Err(ProgramError::InvalidArgument);
            };

            let code_account_key = {
                let program_code = next_account_info(account_info_iter)?;
                if program_code.owner == program_id {
                    let contract_data = AccountData::Contract( Contract {owner: *account_info.key, code_size: 0u32} );
                    contract_data.pack(&mut program_code.data.borrow_mut())?;
    
                    *program_code.key
                } else {
                    Pubkey::new_from_array([0u8; 32])
                }
            };

            let account_data = AccountData::Account( Account {ether, nonce, trx_count: 0u64, signer: *funding_info.key, code_account: code_account_key, blocked: None} );

            let program_seeds = [ether.as_bytes(), &[nonce]];
            invoke_signed(
                &create_account(funding_info.key, account_info.key, lamports, account_data.size() as u64, program_id),
                &accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account done");

            account_data.pack(&mut account_info.data.borrow_mut())?;

            Ok(())
        },
        EvmInstruction::CreateAccountWithSeed {base, seed, lamports, space, owner} => {
            let funding_info = next_account_info(account_info_iter)?;
            let created_info = next_account_info(account_info_iter)?;
            let base_info = next_account_info(account_info_iter)?;

            //debug_print!(&("Ether:".to_owned()+&(hex::encode(ether))+" "+&hex::encode([nonce])));
            if base_info.owner != program_id {return Err(ProgramError::InvalidArgument);}
            let base_info_data = AccountData::unpack(&base_info.data.borrow())?;
            match base_info_data {
                AccountData::Account(_) => (),
                _ => return Err(ProgramError::InvalidAccountData),
            };
            let caller = SolidityAccount::new(base_info.key, (*base_info.lamports.borrow()).clone(), base_info_data, None)?;

            let (caller_ether, caller_nonce) = caller.get_seeds();
            let program_seeds = [caller_ether.as_bytes(), &[caller_nonce]];
            let seed = std::str::from_utf8(&seed).map_err(|_| ProgramError::InvalidArgument)?;
            debug_print!("{}", &lamports.to_string());
            debug_print!("{}", &space.to_string());
            invoke_signed(
                &create_account_with_seed(funding_info.key, created_info.key, &base, &seed, lamports, space, &owner),
                &accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account_with_seed done");

            Ok(())
        },
        EvmInstruction::Write {offset, bytes} => {
            let account_info = next_account_info(account_info_iter)?;
            if account_info.owner != program_id {
                return Err(ProgramError::InvalidArgument);
            }

            do_write(account_info, offset, &bytes)
        },
        EvmInstruction::Finalize => {
            do_finalize(program_id, accounts)
        },
        EvmInstruction::Call {bytes} => {
            do_call(program_id, accounts, &bytes, None)
        },
        EvmInstruction::ExecuteTrxFromAccountData => {
            debug_print!("Execute transaction from account data");

            let account_info_iter = &mut accounts.iter();
            let trx_info = next_account_info(account_info_iter)?;

            let (unsigned_msg, signature) = {
                let data = trx_info.data.borrow();
                let account_info_data = AccountData::unpack(&data)?;
                match account_info_data {
                    AccountData::Empty => (),
                    _ => return Err(ProgramError::InvalidAccountData),
                };

                let (acc_header, rest) = data.split_at(account_info_data.size());
                let (signature, rest) = rest.split_at(65);
                let (trx_len, rest) = rest.split_at(8);
                let trx_len = trx_len.try_into().ok().map(u64::from_le_bytes).unwrap();
                let (trx, _rest) = rest.split_at(trx_len as usize);
                (trx.to_vec(), signature.to_vec())
            };

            if let Err(e) = verify_tx_signature(&signature, &unsigned_msg) {
                debug_print!("{}", e);
                return Err(ProgramError::InvalidInstructionData);
            }
            let trx: UnsignedTransaction = rlp::decode(&unsigned_msg).map_err(|_| ProgramError::InvalidInstructionData)?;

            let mut account_storage = ProgramAccountStorage::new(program_id, &accounts[1..])?;
    
            let (exit_reason, result, applies_logs) = {
                let caller = account_storage.get_caller_account().ok_or(ProgramError::InvalidArgument)?;  
                if caller.get_nonce() != trx.nonce {
                    debug_print!("Invalid nonce: actual {}, expect {}", trx.nonce, caller.get_nonce());
                    return Err(ProgramError::InvalidInstructionData);
                }
                let caller_ether = caller.get_ether();
        
                let backend = SolanaBackend::new(&account_storage, Some(accounts));
                debug_print!("  backend initialized");

                if trx.chain_id != backend.chain_id() {
                    debug_print!("Invalid chain id: actual {}, expect {}", trx.chain_id, backend.chain_id());
                    return Err(ProgramError::InvalidInstructionData); 
                }
            
                let config = evm::Config::istanbul();
                let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
                debug_print!("Executor initialized");

                let exit_reason = match trx.to {
                    None => {
                        executor.transact_create(caller_ether, U256::zero(), trx.call_data, usize::max_value())
                    },
                    Some(contract) => {
                        debug_print!("Not supported");
                        ExitReason::Fatal(ExitFatal::NotSupported)
                    },
                };

                if exit_reason.is_succeed() {
                    debug_print!("Succeed execution");
                    let (applies, logs) = executor.deconstruct();
                    (exit_reason, Vec::new(), Some((applies, logs)))
                } else {
                    (exit_reason, Vec::new(), None)
                }
            };      

            if applies_logs.is_some() {
                let (applies, logs) = applies_logs.unwrap();

                account_storage.apply(applies, false)?;
                debug_print!("Applies done");
                for log in logs {
                    invoke(&on_event(program_id, log)?, &accounts)?;
                }
            }

            invoke_on_return(&program_id, &accounts, exit_reason, &result)?;

            Ok(())
        },
        EvmInstruction::ExecuteTrxFromAccountDataIterative{step_count} =>{
            debug_print!("Execute iterative transaction from account data");
            let account_info_iter = &mut accounts.iter();
            let holder_info = next_account_info(account_info_iter)?;
            let storage_info = next_account_info(account_info_iter)?;

            let  accounts = &accounts[1..];

            let (unsigned_msg, signature) = {
                let data = holder_info.data.borrow();
                let account_info_data = AccountData::unpack(&data)?;
                match account_info_data {
                    AccountData::Empty => (),
                    _ => return Err(ProgramError::InvalidAccountData),
                };

                let (acc_header, rest) = data.split_at(account_info_data.size());
                let (signature, rest) = rest.split_at(65);
                let (trx_len, rest) = rest.split_at(8);
                let trx_len = trx_len.try_into().ok().map(u64::from_le_bytes).unwrap();
                let (trx, _rest) = rest.split_at(trx_len as usize);
                (trx.to_vec(), signature.to_vec())
            };
            if let Err(e) = verify_tx_signature(&signature, &unsigned_msg) {
                debug_print!("{}", e);
                return Err(ProgramError::InvalidInstructionData);
            }
            let trx: UnsignedTransaction = rlp::decode(&unsigned_msg).map_err(|_| ProgramError::InvalidInstructionData)?;
            let nonce = trx.nonce;
            let data = trx.call_data;
            let to = trx.to;
            match to{
                Some(_) => {
                    debug_print!("This is not deploy contract transaction");
                    return Err(ProgramError::InvalidInstructionData);
                },
                None => {}
            }

            let mut account_storage = ProgramAccountStorage::new(program_id, &accounts[1..])?;

            let caller = account_storage.get_caller_account().ok_or(ProgramError::InvalidArgument)?;
            if caller.get_nonce() != nonce {
                debug_print!("Invalid nonce: actual {}, expect {}", nonce, caller.get_nonce());
                return Err(ProgramError::InvalidInstructionData);
            }
            let caller_ether = caller.get_ether();
            debug_print!("   caller: {}", &caller_ether.to_string());

            let mut storage = StorageAccount::new(storage_info, accounts, caller_ether, trx.nonce)?;

            let backend = SolanaBackend::new(&account_storage, Some(accounts));
            debug_print!("  backend initialized");

            if trx.chain_id != backend.chain_id() {
                debug_print!("Invalid chain id: actual {}, expect {}", trx.chain_id, backend.chain_id());
                return Err(ProgramError::InvalidInstructionData);
            }

            let executor_state = ExecutorState::new(ExecutorSubstate::new(), backend);
            let mut executor = Machine::new(executor_state);

            debug_print!("Executor initialized");

            executor.create_begin(caller_ether, data, u64::max_value());
            executor.execute_n_steps(step_count).unwrap();

            debug_print!("save");
            executor.save_into(&mut storage);
            storage.block_accounts(program_id, accounts)
        },

        EvmInstruction::CallFromRawEthereumTX  {from_addr, sign, unsigned_msg} => {
            let account_info_iter = &mut accounts.iter();
            let program_info = next_account_info(account_info_iter)?;
            let program_code = next_account_info(account_info_iter)?;
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
            let contract = contract.unwrap();

            let program_eth: H160 = keccak256_digest(&program_info.key.to_bytes()).into();
            let caller_eth: H160 = keccak256_digest(&caller_info.key.to_bytes()).into(); 

            do_call(program_id, accounts, &data, Some( (caller, nonce) ))
        },
        EvmInstruction::CheckEtheriumTX {from_addr, sign, unsigned_msg} => {    
            let account_info_iter = &mut accounts.iter();
            let program_info = next_account_info(account_info_iter)?;
            let program_code = next_account_info(account_info_iter)?;
            let caller_info = next_account_info(account_info_iter)?;
            let sysvar_info = next_account_info(account_info_iter)?;
            let clock_info = next_account_info(account_info_iter)?;

            let current_instruction = instructions::load_current_index(&sysvar_info.try_borrow_data()?);
            debug_print!(" current instruction: {}", &current_instruction); 

            let index = current_instruction - 1;
            debug_print!("index: {}", &index); 

            match load_instruction_at(index.try_into().unwrap(), &sysvar_info.try_borrow_data()?) {
                Ok(instr) => {
                    if instr.program_id == secp256k1_program::id() {
                        let sliced = instr.data.as_slice();

                        let reference_instruction = make_secp256k1_instruction(current_instruction, unsigned_msg.len(), 1u16);

                        if reference_instruction != instr.data {
                            debug_print!("wrong keccak instruction data");
                            debug_print!("instruction: {}", &hex::encode(&instr.data));    
                            debug_print!("reference: {}", &hex::encode(&reference_instruction));    
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
            let contract = contract.unwrap();

            let program_eth: H160 = keccak256_digest(&program_info.key.to_bytes()).into();
            // let caller_eth: H160 = keccak256_digest(&caller_info.key.to_bytes()).into();
            
            debug_print!("caller: {}", &caller.to_string());    
            debug_print!("contract: {}", &contract.to_string());
            debug_print!("program_eth: {}", &program_eth.to_string());
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
        EvmInstruction::OnReturn {status, bytes} => {
            Ok(())
        },
        EvmInstruction::OnEvent {address, topics, data} => {
            Ok(())
        },
        EvmInstruction::PartialCallFromRawEthereumTX {step_count, from_addr, sign, unsigned_msg} => {
            let account_info_iter = &mut accounts.iter();
            let storage_info = next_account_info(account_info_iter)?;
            let _program_info = next_account_info(account_info_iter)?;
            let _program_code = next_account_info(account_info_iter)?;
            let _caller_info = next_account_info(account_info_iter)?;
            let sysvar_info = next_account_info(account_info_iter)?;

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 9u16)?;

            let caller = H160::from_slice(from_addr);
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|_| ProgramError::InvalidInstructionData)?;

            let mut storage = StorageAccount::new(storage_info, accounts, caller, trx.nonce)?;

            do_partial_call(&mut storage, program_id, step_count, &accounts[1..], trx.call_data, Some( (caller, trx.nonce) ))?;

            storage.block_accounts(program_id, accounts)
        },
        EvmInstruction::Continue {step_count} => {
            let account_info_iter = &mut accounts.iter();
            let storage_info = next_account_info(account_info_iter)?;

            let mut storage = StorageAccount::restore(storage_info)?;
            storage.check_accounts(program_id, accounts)?;

            let caller_and_nonce = storage.caller_and_nonce()?;

            let exit_reason = do_continue(&mut storage, program_id, step_count, &accounts[1..], Some(caller_and_nonce))?;
            if exit_reason != None {
                storage.unblock_accounts_and_destroy(program_id, accounts)?;
            }

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

    debug_print!("Total memory occupied: {}", &BumpAllocator::occupied());
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

fn do_write(account_info: &AccountInfo, offset: u32, bytes: &[u8]) -> ProgramResult {
    let mut data = account_info.data.borrow_mut();

    let account_data = AccountData::unpack(&data)?;
    match account_data {
        AccountData::Contract(ref acc) => {
            if acc.code_size != 0 {
                return Err(ProgramError::InvalidAccountData);
            }
        },
        AccountData::Account(_) => return Err(ProgramError::InvalidAccountData),
        AccountData::Storage(_) => return Err(ProgramError::InvalidAccountData),
        AccountData::Empty => (),
    };

    let offset = account_data.size() + offset as usize;
    if data.len() < offset + bytes.len() {
        debug_print!("Account data too small");
        return Err(ProgramError::AccountDataTooSmall);
    }
    data[offset .. offset+bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

fn do_finalize<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>]) -> ProgramResult {
    debug_print!("do_finalize");

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;
    let program_code = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };

    let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;

    check_from_or_signer(program_id, account_storage.get_caller_account(), caller_info, signer_info, None)?;

    let (exit_reason, result, applies_logs) = {
        let backend = SolanaBackend::new(&account_storage, Some(accounts));
        debug_print!("  backend initialized");
        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
        debug_print!("  executor initialized");

        let code_data = {
            let data = program_code.data.borrow();
            let contract_info_data = AccountData::unpack(&data)?;
            match contract_info_data {
                AccountData::Contract (..) => (),
                _ => return Err(ProgramError::InvalidAccountData),
            };

            let (_contract_header, rest) = data.split_at(contract_info_data.size());
            let (code_len, rest) = rest.split_at(8);
            let code_len = code_len.try_into().ok().map(u64::from_le_bytes).unwrap();
            let (code, _rest) = rest.split_at(code_len as usize);
            code.to_vec()
        };
    
        // let program_account = SolidityAccount::new(program_info)?;
        debug_print!("Execute transact_create");
        let exit_reason = executor.transact_create2(
                account_storage.origin(),
                U256::zero(),
                code_data,
                H256::default(), usize::max_value()
            );
        debug_print!("  create2 done");   
        
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let (applies, logs) = executor.deconstruct();
            (exit_reason, Vec::new(), Some((applies, logs)))
        } else {
            (exit_reason, Vec::new(), None)
        }
    }; 

    if applies_logs.is_some() {
        let (applies, logs) = applies_logs.unwrap();
        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log)?, &accounts)?;
        }
    }

    invoke_on_return(&program_id, &accounts, exit_reason, &result)?;
    
    Ok(())
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
    let program_code = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };

    let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;

    check_from_or_signer(program_id, account_storage.get_caller_account(), caller_info, signer_info, from_info)?;

    debug_print!("   caller: {}", &account_storage.origin().to_string());
    debug_print!(" contract: {}", &account_storage.contract().to_string());

    let (exit_reason, result, applies_logs) = {
        let backend = SolanaBackend::new(&account_storage, Some(accounts));
        debug_print!("  backend initialized");

        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
        debug_print!("Executor initialized");

        let (exit_reason, result) = executor.transact_call(account_storage.origin(), account_storage.contract(), U256::zero(), instruction_data.to_vec(), usize::max_value());

        debug_print!("Call done");

        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let (applies, logs) = executor.deconstruct();
            (exit_reason, result, Some((applies, logs)))
        } else {
            (exit_reason, result, None)
        }
    };

    if applies_logs.is_some() {
        let (applies, logs) = applies_logs.unwrap();

        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log)?, &accounts)?;
        }
    }

    invoke_on_return(&program_id, &accounts, exit_reason, &result)?;

    Ok(())
}

fn do_partial_call<'a>(
    storage: &mut StorageAccount,
    program_id: &Pubkey,
    step_count: u64,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: Vec<u8>,
    from_info: Option<(H160, u64)>,
) -> ProgramResult
{
    debug_print!("do_partial_call");

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;
    let program_code = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };

    if program_info.owner != program_id {
        return Err(ProgramError::InvalidArgument);
    }

    let account_storage = ProgramAccountStorage::new(program_id, accounts)?;

    check_from_or_signer(program_id, account_storage.get_caller_account(), caller_info, signer_info, from_info)?;

    let backend = SolanaBackend::new(&account_storage, Some(accounts));
    debug_print!("  backend initialized");

    let executor_state = ExecutorState::new(ExecutorSubstate::new(), backend);
    let mut executor = Machine::new(executor_state);

    debug_print!("Executor initialized");

    debug_print!("   caller: {}", &account_storage.origin().to_string());
    debug_print!(" contract: {}", &account_storage.contract().to_string());

    executor.call_begin(account_storage.origin(), account_storage.contract(), instruction_data, u64::max_value());
    executor.execute_n_steps(step_count).unwrap();

    debug_print!("save");
    executor.save_into(storage);

    debug_print!("partial call complete");

    Ok(())
}

fn do_continue<'a>(
    storage: &mut StorageAccount,
    program_id: &Pubkey,
    step_count: u64,
    accounts: &'a [AccountInfo<'a>],
    from_info: Option<(H160, u64)>,
) -> Result<Option<ExitReason>, ProgramError>
{
    debug_print!("do_continue");

    let account_info_iter = &mut accounts.iter();
    let program_info = next_account_info(account_info_iter)?;
    let program_code = next_account_info(account_info_iter)?;
    let caller_info = next_account_info(account_info_iter)?;
    let signer_info = if caller_info.owner == program_id {
        next_account_info(account_info_iter)?
    } else {
        caller_info
    };

    let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;

    check_from_or_signer(program_id, account_storage.get_caller_account(), caller_info, signer_info, from_info)?;

    let (exit_reason, result, applies_logs) = {
        let backend = SolanaBackend::new(&account_storage, Some(accounts));
        debug_print!("  backend initialized");

        let mut executor = Machine::restore(storage, backend);
        debug_print!("Executor restored");

        let exit_reason = match executor.execute_n_steps(step_count) {
            Ok(()) => {
                executor.save_into(storage);
                debug_print!("{} steps executed", step_count);
                return Ok(None);
            }
            Err(reason) => reason
        };
        let result = executor.return_value();

        debug_print!("Call done");

        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let executor_state = executor.into_state();
            let (_, (applies, logs)) = executor_state.deconstruct();
            (exit_reason, result, Some((applies, logs)))
        } else {
            (exit_reason, result, None)
        }
    };

    if let Some((applies, logs)) = applies_logs {
        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log)?, &accounts)?;
        }
    }

    invoke_on_return(&program_id, &accounts, exit_reason.clone(), &result)?;

    Ok(Some(exit_reason))
}

fn invoke_on_return<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    exit_reason: ExitReason,
    result: &Vec<u8>,) -> ProgramResult
{    
    let exit_status = match exit_reason {
        ExitReason::Succeed(success_code) => { 
            debug_print!("Succeed");
            match success_code {
                ExitSucceed::Stopped => { debug_print!("Machine encountered an explict stop."); 0x11},
                ExitSucceed::Returned => { debug_print!("Machine encountered an explict return."); 0x12},
                ExitSucceed::Suicided => { debug_print!("Machine encountered an explict suicide."); 0x13},
            }
        },
        ExitReason::Error(error_code) => { 
            debug_print!("Error");
            match error_code {
                ExitError::StackUnderflow => { debug_print!("Trying to pop from an empty stack."); 0xe1},
                ExitError::StackOverflow => { debug_print!("Trying to push into a stack over stack limit."); 0xe2},
                ExitError::InvalidJump => { debug_print!("Jump destination is invalid."); 0xe3},
                ExitError::InvalidRange => { debug_print!("An opcode accesses memory region, but the region is invalid."); 0xe4},
                ExitError::DesignatedInvalid => { debug_print!("Encountered the designated invalid opcode."); 0xe5},
                ExitError::CallTooDeep => { debug_print!("Call stack is too deep (runtime)."); 0xe6},
                ExitError::CreateCollision => { debug_print!("Create opcode encountered collision (runtime)."); 0xe7},
                ExitError::CreateContractLimit => { debug_print!("Create init code exceeds limit (runtime)."); 0xe8},
                ExitError::OutOfOffset => { debug_print!("An opcode accesses external information, but the request is off offset limit (runtime)."); 0xe9},
                ExitError::OutOfGas => { debug_print!("Execution runs out of gas (runtime)."); 0xea},
                ExitError::OutOfFund => { debug_print!("Not enough fund to start the execution (runtime)."); 0xeb},
                ExitError::PCUnderflow => { debug_print!("PC underflowed (unused)."); 0xec},
                ExitError::CreateEmpty => { debug_print!("Attempt to create an empty account (runtime, unused)."); 0xed},
                ExitError::Other(_) => { debug_print!("Other normal errors."); 0xee},
            }
        },
        ExitReason::Revert(_) => { debug_print!("Revert"); 0xd0},
        ExitReason::Fatal(fatal_code) => {             
            debug_print!("Fatal");
            match fatal_code {
                ExitFatal::NotSupported => { debug_print!("The operation is not supported."); 0xf1},
                ExitFatal::UnhandledInterrupt => { debug_print!("The trap (interrupt) is unhandled."); 0xf2},
                ExitFatal::CallErrorAsFatal(_) => { debug_print!("The environment explictly set call errors as fatal error."); 0xf3},
                ExitFatal::Other(_) => { debug_print!("Other fatal errors."); 0xf4},
            }
        },
    };

    debug_print!("{}", &hex::encode(&result));

    let ix = on_return(program_id, exit_status, &result).unwrap();
    invoke(
        &ix,
        &accounts
    )?;

    Ok(())
}

fn check_from_or_signer<'a>(
    program_id: &Pubkey,
    caller_opt: Option<&SolidityAccount<'a>>,
    caller_info: &'a AccountInfo<'a>,
    signer_info: &'a AccountInfo<'a>,
    from_info: Option<(H160, u64)>,
) ->  ProgramResult
{
    if caller_info.owner == program_id {
        if caller_opt.is_some() {
            let caller = caller_opt.unwrap();

            let caller_signer = caller.get_signer();
            let caller_ether = caller.get_ether();
            let caller_nonce = caller.get_nonce();

            if from_info.is_none() {
                if caller_signer != *signer_info.key || !signer_info.is_signer {
                    debug_print!("Add valid account signer");
                    debug_print!("   caller signer: {}", &caller_signer.to_string());
                    debug_print!("   signer pubkey: {}", &signer_info.key.to_string());
                    debug_print!("is signer signer: {}", &signer_info.is_signer.to_string());

                    return Err(ProgramError::InvalidArgument);
                }
            } else {
                let (from, nonce) = from_info.unwrap();
                if caller_ether != from {
                    debug_print!("Invalid caller account");
                    debug_print!("   caller addres: {}", &caller_ether.to_string());
                    debug_print!("     from addres: {}", &from.to_string());

                    return Err(ProgramError::InvalidArgument);
                }
                if caller_nonce != nonce {
                    debug_print!("Invalid Ethereum transaction nonce");
                    debug_print!("     tx nonce: {}", &nonce.to_string());
                    debug_print!("    acc nonce: {}", &caller_nonce.to_string());

                    return Err(ProgramError::InvalidArgument);
                }
            }
        } else {
            return Err(ProgramError::InvalidArgument);
        }
    }

    Ok(())
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

