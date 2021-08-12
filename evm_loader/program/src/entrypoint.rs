//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

//use crate::{error::TokenError, processor::Processor};
//use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use std::convert::{TryInto, TryFrom};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::{ProgramResult, HEAP_START_ADDRESS},
    program_error::{ProgramError}, pubkey::Pubkey,
    system_instruction::{create_account, create_account_with_seed},
    program::{invoke_signed, invoke},
    rent::Rent,
    sysvar::Sysvar
};
use crate::{
//    bump_allocator::BumpAllocator,
    instruction::{EvmInstruction, on_return, on_event},
    account_data::{AccountData, Account, Contract},
    account_storage::{ProgramAccountStorage, Sender},
    solana_backend::{SolanaBackend, AccountStorage},
    solidity_account::SolidityAccount,
    transaction::{UnsignedTransaction, verify_tx_signature, check_secp256k1_instruction, find_sysvar_info, find_rent_info},
    executor::{ Machine },
    executor_state::{ ExecutorState, ExecutorSubstate },
    storage_account::{ StorageAccount },
    error::EvmLoaderError,
    token::{token_mint, create_associated_token_account},
};
use evm::{
    ExitReason, ExitFatal, ExitError, ExitSucceed,
    H160, U256,
};
use std::{alloc::Layout, mem::size_of, ptr::null_mut, usize};

const HEAP_LENGTH: usize = 1024*1024;

/// Developers can implement their own heap by defining their own
/// `#[global_allocator]`.  The following implements a dummy for test purposes
/// but can be flushed out with whatever the developer sees fit.
pub struct BumpAllocator;

impl BumpAllocator {
    /// Get occupied memory
    #[inline]
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
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
// 4. code_size: u32
// 5. storage (all remaining space, if code_size not equal zero)

entrypoint!(process_instruction);

#[allow(clippy::too_many_lines)]
fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {

    let account_info_iter = &mut accounts.iter();

    let instruction = EvmInstruction::unpack(instruction_data)?;
    debug_print!("Instruction parsed");

    #[allow(clippy::match_same_arms)]
    let result = match instruction {
        EvmInstruction::CreateAccount {lamports, space: _, ether, nonce} => {
            let rent_info = find_rent_info(accounts)?;
            let rent = Rent::from_account_info(rent_info)?;

            let funding_info = next_account_info(account_info_iter)?;
            let account_info = next_account_info(account_info_iter)?;
            let token_account_info = next_account_info(account_info_iter)?;

            debug_print!("Ether: {} {}", &(hex::encode(ether)), &hex::encode([nonce]));

            let expected_address = Pubkey::create_program_address(&[ether.as_bytes(), &[nonce]], program_id)?;
            if expected_address != *account_info.key {
                return Err!(ProgramError::InvalidArgument; "expected_address<{:?}> != *account_info.key<{:?}>", expected_address, *account_info.key);
            };

            let code_account_key = {
                let program_code = next_account_info(account_info_iter)?;
                if program_code.owner == program_id {
                    if !rent.is_exempt(program_code.lamports(), program_code.data_len()) {
                        return Err!(ProgramError::InvalidArgument; "Code account is not rent exempt. lamports={:?}, data_len={:?}", program_code.lamports(), program_code.data_len());
                    }

                    let contract_data = AccountData::Contract( Contract {owner: *account_info.key, code_size: 0_u32} );
                    contract_data.pack(&mut program_code.data.borrow_mut())?;

                    *program_code.key
                } else {
                    Pubkey::new_from_array([0_u8; 32])
                }
            };

            let account_data = AccountData::Account(Account {
                ether,
                nonce,
                trx_count: 0_u64,
                code_account: code_account_key,
                blocked: None,
                eth_token_account: *token_account_info.key,
            });

            let account_lamports = rent.minimum_balance(account_data.size()) + lamports;

            let program_seeds = [ether.as_bytes(), &[nonce]];
            invoke_signed(
                &create_account(funding_info.key, account_info.key, account_lamports, account_data.size() as u64, program_id),
                accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account done");

            invoke_signed(
                &create_associated_token_account(funding_info.key, account_info.key, token_account_info.key, &token_mint::id()),
                accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_associated_token_account done");

            account_data.pack(&mut account_info.data.borrow_mut())?;

            Ok(())
        },
        EvmInstruction::CreateAccountWithSeed {base, seed, lamports, space, owner} => {
            let rent_info = find_rent_info(accounts)?;
            let rent = Rent::from_account_info(rent_info)?;

            let funding_info = next_account_info(account_info_iter)?;
            let created_info = next_account_info(account_info_iter)?;
            let base_info = next_account_info(account_info_iter)?;

            //debug_print!(&("Ether:".to_owned()+&(hex::encode(ether))+" "+&hex::encode([nonce])));
            if base_info.owner != program_id {return Err!(ProgramError::InvalidArgument; "base_info.owner<{:?}> != program_id<{:?}>", base_info.owner, program_id);}
            let base_info_data = AccountData::unpack(&base_info.data.borrow())?;
            match base_info_data {
                AccountData::Account(_) => (),
                _ => return Err!(ProgramError::InvalidAccountData),
            };
            let caller = SolidityAccount::new(base_info.key, base_info.lamports(), base_info_data, None);

            let space_as_usize = usize::try_from(space).map_err(|e| E!(ProgramError::InvalidArgument; "e={:?}", e))?;
            let account_lamports = rent.minimum_balance(space_as_usize) + lamports;

            let (caller_ether, caller_nonce) = caller.get_seeds();
            let program_seeds = [caller_ether.as_bytes(), &[caller_nonce]];
            let seed = std::str::from_utf8(&seed).map_err(|e| E!(ProgramError::InvalidArgument; "Utf8Error={:?}", e))?;
            debug_print!("{}", account_lamports);
            debug_print!("{}", space);
            invoke_signed(
                &create_account_with_seed(funding_info.key, created_info.key, &base, seed, account_lamports, space, &owner),
                accounts, &[&program_seeds[..]]
            )?;
            debug_print!("create_account_with_seed done");

            Ok(())
        },
        EvmInstruction::Write {offset, bytes} => {
            let account_info = next_account_info(account_info_iter)?;
            if account_info.owner != program_id {
                return Err!(ProgramError::InvalidArgument; "account_info.owner<{:?}> != program_id<{:?}>", account_info.owner, program_id);
            }

            do_write(account_info, offset, bytes)
        },
        EvmInstruction::Finalize => {
            do_finalize(program_id, accounts)
        },
        EvmInstruction::Call {bytes} => {
            let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;
            if let Sender::Solana(_addr) = account_storage.get_sender() {
                // Success execution
            } else {
                return Err!(ProgramError::InvalidArgument; "This method should used with Solana sender");
            }

            do_call(program_id, &mut account_storage, accounts, bytes.to_vec(), U256::zero(), u64::MAX)
        },
        EvmInstruction::ExecuteTrxFromAccountDataIterative{step_count} =>{
            debug_print!("Execute iterative transaction from account data");
            let holder_info = next_account_info(account_info_iter)?;
            let storage_info = next_account_info(account_info_iter)?;

            let  accounts = &accounts[1..];

            let holder_data = holder_info.data.borrow();
            let (unsigned_msg, signature) = get_transaction_from_data(&holder_data)?;

            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            let account_storage = ProgramAccountStorage::new(program_id, &accounts[1..])?;
            let from_addr = verify_tx_signature(signature, unsigned_msg).map_err(|e| E!(ProgramError::MissingRequiredSignature; "Error={:?}", e))?;
            check_ethereum_authority(
                account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                &from_addr, trx.nonce, &trx.chain_id)?;

            let mut storage = StorageAccount::new(storage_info, accounts, from_addr, trx.nonce)?;

            let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|s| E!(ProgramError::InvalidInstructionData; "s={:?}", s))?;
            if trx.to.is_some() {
                do_partial_call(&mut storage, step_count, &account_storage, accounts, trx.call_data, trx.value, trx_gas_limit)?;
            }
            else {
                do_partial_create(&mut storage, step_count, &account_storage, accounts, trx.call_data, trx.value, trx_gas_limit)?;
            }

            storage.block_accounts(program_id, accounts)
        },

        EvmInstruction::CallFromRawEthereumTX  {from_addr, sign: _, unsigned_msg} => {
            let sysvar_info = find_sysvar_info(accounts)?;

            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;
            let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 1_u16)?;
            check_ethereum_authority(
                account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                &H160::from_slice(from_addr), trx.nonce, &trx.chain_id)?;

            let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|s| E!(ProgramError::InvalidInstructionData; "s={:?}", s))?;
            do_call(program_id, &mut account_storage, accounts, trx.call_data, trx.value, trx_gas_limit)
        },
        EvmInstruction::OnReturn {status: _, bytes: _} => {
            Ok(())
        },
        EvmInstruction::OnEvent {address: _, topics: _, data: _} => {
            Ok(())
        },
        EvmInstruction::PartialCallFromRawEthereumTX {step_count, from_addr, sign: _, unsigned_msg} => {
            let storage_info = next_account_info(account_info_iter)?;
            let sysvar_info = find_sysvar_info(accounts)?;

            let caller = H160::from_slice(from_addr);
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            let mut storage = StorageAccount::new(storage_info, accounts, caller, trx.nonce)?;
            let account_storage = ProgramAccountStorage::new(program_id, &accounts[1..])?;

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 9_u16)?;
            check_ethereum_authority(
                account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                &caller, trx.nonce, &trx.chain_id)?;

            let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|s| E!(ProgramError::InvalidInstructionData; "s={:?}", s))?;
            do_partial_call(&mut storage, step_count, &account_storage, &accounts[1..], trx.call_data, trx.value, trx_gas_limit)?;

            storage.block_accounts(program_id, accounts)
        },
        EvmInstruction::Continue {step_count} => {
            let storage_info = next_account_info(account_info_iter)?;

            let mut storage = StorageAccount::restore(storage_info).map_err(|err| {
                if err == ProgramError::InvalidAccountData {EvmLoaderError::StorageAccountUninitialized.into()}
                else {err}
            })?;
            storage.check_accounts(program_id, accounts)?;

            let mut account_storage = ProgramAccountStorage::new(program_id, &accounts[1..])?;

            let exit_reason = do_continue(&mut storage, program_id, step_count, &mut account_storage, &accounts[1..])?;
            if exit_reason != None {
                storage.unblock_accounts_and_destroy(program_id, accounts)?;
            }

            Ok(())
        },
        EvmInstruction::Cancel => {
            let storage_info = next_account_info(account_info_iter)?;
            let _program_info = next_account_info(account_info_iter)?;
            let _program_token = next_account_info(account_info_iter)?;
            let _program_code = next_account_info(account_info_iter)?;
            let caller_info = next_account_info(account_info_iter)?;
            let _caller_token = next_account_info(account_info_iter)?;
            let _sysvar_info = next_account_info(account_info_iter)?;

            let storage = StorageAccount::restore(storage_info)?;
            storage.check_accounts(program_id, accounts)?;

            { // Increment nonce for caller on cancel
                let mut caller_info_data = AccountData::unpack(&caller_info.data.borrow())?;
                match caller_info_data {
                    AccountData::Account(ref mut acc) => {
                        let (caller, nonce) = storage.caller_and_nonce()?;
                        if acc.ether != caller {
                            return Err!(ProgramError::InvalidAccountData; "acc.ether<{:?}> != caller<{:?}>", acc.ether, caller);
                        }
                        if acc.trx_count != nonce {
                            return Err!(ProgramError::InvalidAccountData; "acc.trx_count<{:?}> != nonce<{:?}>", acc.trx_count, nonce);
                        }
                        acc.trx_count += 1;
                    },
                    _ => return Err!(ProgramError::InvalidAccountData),
                };
                caller_info_data.pack(&mut caller_info.data.borrow_mut())?;
            }

            storage.unblock_accounts_and_destroy(program_id, accounts)?;

            Ok(())
        },
    };

    solana_program::msg!("Total memory occupied: {}", &BumpAllocator::occupied());
    result
}

fn get_transaction_from_data(
    data: &[u8]
) -> Result<(&[u8], &[u8]), ProgramError>
{
    let account_info_data = AccountData::unpack(data)?;
    match account_info_data {
        AccountData::Empty => (),
            _ => return Err!(ProgramError::InvalidAccountData),
    };

    let (_header, rest) = data.split_at(account_info_data.size());
    let (signature, rest) = rest.split_at(65);
    let (trx_len, rest) = rest.split_at(8);
    let trx_len = trx_len.try_into().ok().map(u64::from_le_bytes).unwrap();
    let trx_len = usize::try_from(trx_len).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
    let (trx, _rest) = rest.split_at(trx_len as usize);

    Ok((trx, signature))
}

fn do_write(account_info: &AccountInfo, offset: u32, bytes: &[u8]) -> ProgramResult {
    let mut data = account_info.data.borrow_mut();

    let account_data = AccountData::unpack(&data)?;
    match account_data {
        AccountData::Account(_) | AccountData::Storage(_) => {
            return Err!(ProgramError::InvalidAccountData);
        },
        AccountData::Contract(acc) if acc.code_size != 0 => {
            return Err!(ProgramError::InvalidAccountData);
        },
        AccountData::Contract(_) | AccountData::Empty => { },
    };

    let offset = account_data.size() + offset as usize;
    if data.len() < offset + bytes.len() {
        return Err!(ProgramError::AccountDataTooSmall; "Account data too small data.len()={:?}, offset={:?}, bytes.len()={:?}", data.len(), offset, bytes.len());
    }
    data[offset .. offset+bytes.len()].copy_from_slice(bytes);
    Ok(())
}

fn do_finalize<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>]) -> ProgramResult {
    debug_print!("do_finalize");

    let account_info_iter = &mut accounts.iter();
    let _program_info = next_account_info(account_info_iter)?;
    let program_code = next_account_info(account_info_iter)?;

    let mut account_storage = ProgramAccountStorage::new(program_id, accounts)?;

    let (exit_reason, used_gas, result, applies_logs_transfers) = {
        let backend = SolanaBackend::new(&account_storage, Some(accounts));
        debug_print!("  backend initialized");

        let code_data = {
            let data = program_code.data.borrow();
            let contract_info_data = AccountData::unpack(&data)?;
            match contract_info_data {
                AccountData::Contract (..) => (),
                _ => return Err!(ProgramError::InvalidAccountData),
            };

            let (_contract_header, rest) = data.split_at(contract_info_data.size());
            let (code_len, rest) = rest.split_at(8);
            let code_len = code_len.try_into().ok().map(u64::from_le_bytes).unwrap();
            let code_len = usize::try_from(code_len).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
            let (code, _rest) = rest.split_at(code_len);
            code.to_vec()
        };

        let executor_state = ExecutorState::new(ExecutorSubstate::new(u64::MAX), backend);
        let mut executor = Machine::new(executor_state);

        debug_print!("Executor initialized");
        executor.create_begin(account_storage.origin(), code_data, U256::zero(), u64::MAX)?;
        let (result, exit_reason) = executor.execute();
        debug_print!("Call done");

        let executor_state = executor.into_state();
        let used_gas = executor_state.substate().metadata().gasometer().used_gas();
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let (_, (applies, logs, transfers)) = executor_state.deconstruct();
            (exit_reason, used_gas, result, Some((applies, logs, transfers)))
        } else {
            (exit_reason, used_gas, result, None)
        }
    };

    if let Some((applies, logs, transfers)) = applies_logs_transfers {
        account_storage.apply_transfers(accounts, transfers)?;
        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log), accounts)?;
        }
    }

    invoke_on_return(program_id, accounts, exit_reason, used_gas, &result[..])?;
    Ok(())
}

fn do_call<'a>(
    program_id: &Pubkey,
    account_storage: &mut ProgramAccountStorage,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> ProgramResult
{
    debug_print!("do_call");

    debug_print!("   caller: {}", account_storage.origin());
    debug_print!(" contract: {}", account_storage.contract());

    let (exit_reason, used_gas, result, applies_logs_transfers) = {
        let backend = SolanaBackend::new(account_storage, Some(accounts));
        debug_print!("  backend initialized");

        let executor_state = ExecutorState::new(ExecutorSubstate::new(gas_limit), backend);
        let mut executor = Machine::new(executor_state);

        debug_print!("Executor initialized");

	    executor.call_begin(
            account_storage.origin(),
            account_storage.contract(),
            instruction_data,
            transfer_value,
            gas_limit,
        )?;

        let (result, exit_reason) = executor.execute();

        debug_print!("Call done");

        let executor_state = executor.into_state();
        let used_gas = executor_state.substate().metadata().gasometer().used_gas();
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let (_, (applies, logs, transfers)) = executor_state.deconstruct();
            (exit_reason, used_gas, result, Some((applies, logs, transfers)))
        } else {
            (exit_reason, used_gas, result, None)
        }
    };

    if let Some((applies, logs, transfers)) = applies_logs_transfers {
        account_storage.apply_transfers(accounts, transfers)?;
        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log), accounts)?;
        }
    }

    invoke_on_return(program_id, accounts, exit_reason, used_gas, &result[..])?;
    Ok(())
}

fn do_partial_call<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &ProgramAccountStorage,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> ProgramResult
{
    debug_print!("do_partial_call");

    let backend = SolanaBackend::new(account_storage, Some(accounts));
    debug_print!("  backend initialized");

    let executor_state = ExecutorState::new(ExecutorSubstate::new(gas_limit), backend);
    let mut executor = Machine::new(executor_state);

    debug_print!("Executor initialized");

    debug_print!("   caller: {}", account_storage.origin());
    debug_print!(" contract: {}", account_storage.contract());

    executor.call_begin(
        account_storage.origin(),
        account_storage.contract(),
        instruction_data,
        transfer_value,
        gas_limit,
    )?;

    executor.execute_n_steps(step_count).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;

    debug_print!("save");
    executor.save_into(storage);

    debug_print!("partial call complete");
    Ok(())
}

fn do_partial_create<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &ProgramAccountStorage,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> ProgramResult
{
    debug_print!("do_partial_create gas_limit={}", gas_limit);

    let backend = SolanaBackend::new(account_storage, Some(accounts));
    debug_print!("  backend initialized");

    let executor_state = ExecutorState::new(ExecutorSubstate::new(gas_limit), backend);
    let mut executor = Machine::new(executor_state);

    debug_print!("Executor initialized");

    executor.create_begin(account_storage.origin(), instruction_data, transfer_value, gas_limit)?;
    executor.execute_n_steps(step_count).unwrap();

    debug_print!("save");
    executor.save_into(storage);

    debug_print!("partial create complete");
    Ok(())
}

fn do_continue<'a>(
    storage: &mut StorageAccount,
    program_id: &Pubkey,
    step_count: u64,
    account_storage: &mut ProgramAccountStorage,
    accounts: &'a [AccountInfo<'a>],
) -> Result<Option<ExitReason>, ProgramError>
{
    debug_print!("do_continue");

    let (exit_reason, used_gas, result, applies_logs_transfers) = {
        let backend = SolanaBackend::new(account_storage, Some(accounts));
        debug_print!("  backend initialized");

        let mut executor = Machine::restore(storage, backend);
        debug_print!("Executor restored");

        let (result, exit_reason) = match executor.execute_n_steps(step_count) {
            Ok(()) => {
                executor.save_into(storage);
                debug_print!("{} steps executed", step_count);
                return Ok(None);
            }
            Err((result, reason)) => (result, reason)
        };

        debug_print!("Call done");

        let executor_state = executor.into_state();
        let used_gas = executor_state.substate().metadata().gasometer().used_gas();
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let (_, (applies, logs, transfers)) = executor_state.deconstruct();
            (exit_reason, used_gas, result, Some((applies, logs, transfers)))
        } else {
            (exit_reason, used_gas, result, None)
        }
    };

    if let Some((applies, logs, transfers)) = applies_logs_transfers {
        account_storage.apply_transfers(accounts, transfers)?;
        account_storage.apply(applies, false)?;
        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log), accounts)?;
        }
    }

    invoke_on_return(program_id, accounts, exit_reason, used_gas, &result[..])?;
    Ok(Some(exit_reason))
}

fn invoke_on_return<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    exit_reason: ExitReason,
    used_gas: u64,
    result: &[u8],
) -> ProgramResult
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
            }
        },
        ExitReason::Revert(_) => { debug_print!("Revert"); 0xd0},
        ExitReason::Fatal(fatal_code) => {
            debug_print!("Fatal");
            match fatal_code {
                ExitFatal::NotSupported => { debug_print!("The operation is not supported."); 0xf1},
                ExitFatal::UnhandledInterrupt => { debug_print!("The trap (interrupt) is unhandled."); 0xf2},
                ExitFatal::CallErrorAsFatal(_) => { debug_print!("The environment explictly set call errors as fatal error."); 0xf3},
            }
        },
        ExitReason::StepLimitReached => unreachable!(),
    };

    debug_print!("exit status {}", exit_status);
    debug_print!("used gas {}", used_gas);
    debug_print!("result {}", &hex::encode(&result));

    let ix = on_return(program_id, exit_status, used_gas, result);
    invoke(
        &ix,
        accounts
    )?;

    Ok(())
}

fn check_ethereum_authority<'a>(
   sender: &SolidityAccount<'a>,
   recovered_address: &H160,
   trx_nonce: u64,
   chain_id: &U256,
) -> ProgramResult
{
    if sender.get_ether() != *recovered_address {
        return Err!(ProgramError::InvalidArgument; "Invalid sender: actual {}, recovered {}", sender.get_ether(), recovered_address);
    }

    if sender.get_nonce() != trx_nonce {
        return Err!(ProgramError::InvalidArgument; "Invalid Ethereum transaction nonce: acc {}, trx {}", sender.get_nonce(), trx_nonce);
    }

    if SolanaBackend::<ProgramAccountStorage>::chain_id() != *chain_id {
        return Err!(ProgramError::InvalidArgument; "Invalid chain_id: actual {}, expected {}", chain_id, SolanaBackend::<ProgramAccountStorage>::chain_id());
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
            Err!(ProgramError::InvalidInstructionData),
            process_instruction(&program_id, &[], &bad_utf8)
        );
    }
}

