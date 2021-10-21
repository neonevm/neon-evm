//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

use std::{
    alloc::Layout,
    convert::{TryFrom, TryInto},
    mem::size_of, 
    ptr::null_mut, 
    usize
};

use evm::{
    ExitError, ExitFatal, ExitReason, ExitSucceed,
    H160, U256,
};

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::{ProgramResult, HEAP_START_ADDRESS},
    program_error::{ProgramError}, pubkey::Pubkey,
    program::{invoke},
    rent::Rent,
    sysvar::Sysvar,
    msg,
};

use crate::{
    //    bump_allocator::BumpAllocator,
    config::{ chain_id, token_mint },
    account_data::{Account, AccountData, Contract, ACCOUNT_SEED_VERSION, ACCOUNT_MAX_SIZE},
    account_storage::{ProgramAccountStorage, /* Sender */ },
    solana_backend::{AccountStorage},
    transaction::{UnsignedTransaction, verify_tx_signature, check_secp256k1_instruction},
    executor_state::{ ExecutorState, ExecutorSubstate, ApplyState },
    storage_account::{ StorageAccount },
    error::EvmLoaderError,
    executor::Machine,
    instruction::{EvmInstruction, on_event, on_return},
    payment,
    token,
    token::{create_associated_token_account, get_token_account_owner},
    operator::authorized_operator_check,
    system::create_pda_account,
    utils::is_zero_initialized
};
use crate::solana_program::program_pack::Pack;

type SuccessExitResults = (ExitReason, u64, Vec<u8>, Option<ApplyState>);
type CallResult = Result<Option<SuccessExitResults>, ProgramError>;

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
    debug_print!("Instruction parsed: {:?}", instruction);

    #[allow(clippy::match_same_arms)]
    let result = match instruction {
        EvmInstruction::CreateAccount {lamports: _, space: _, ether, nonce} => {
            let rent = Rent::get()?;
            
            let funding_info = next_account_info(account_info_iter)?;
            let account_info = next_account_info(account_info_iter)?;
            let token_account_info = next_account_info(account_info_iter)?;

            authorized_operator_check(funding_info)?;
            
            debug_print!("Ether: {} {}", &(hex::encode(ether)), &hex::encode([nonce]));

            if !funding_info.is_signer {
                return Err!(ProgramError::InvalidArgument; "!funding_info.is_signer");
            }
            
            let mut program_seeds: Vec<&[u8]> = vec![&[ACCOUNT_SEED_VERSION], ether.as_bytes()];
            let (expected_address, expected_nonce) = Pubkey::find_program_address(&program_seeds, program_id);
            if expected_address != *account_info.key {
                return Err!(ProgramError::InvalidArgument; "expected_address<{:?}> != *account_info.key<{:?}>", expected_address, *account_info.key);
            };
            if expected_nonce != nonce {
                return Err!(ProgramError::InvalidArgument; "expected_nonce<{:?}> != nonce<{:?}>", expected_nonce, nonce);
            };

            let nonce_bytes = &[nonce];
            program_seeds.push(nonce_bytes);

            let code_account_key = {
                let program_code = next_account_info(account_info_iter)?;
                if program_code.owner == program_id {
                    let code_address_seed: &[u8] = &[ &[ACCOUNT_SEED_VERSION], ether.as_bytes() ].concat();
                    let code_address_seed = bs58::encode(code_address_seed).into_string();
                    debug_print!("Code account seed: {}", code_address_seed);
                    let expected_code_address = Pubkey::create_with_seed(funding_info.key, &code_address_seed, program_id)?;
                    if *program_code.key != expected_code_address {
                        return Err!(ProgramError::InvalidArgument; "Unexpected code account. Actual<{:?}> != Expected<{:?}>", program_code.key, expected_code_address);
                    }

                    if !is_zero_initialized(&program_code.try_borrow_data()?) {
                        return Err!(ProgramError::InvalidArgument; "Code account is not empty");
                    }

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

            create_pda_account(
                program_id,
                accounts,
                account_info,
                &program_seeds,
                funding_info.key,
                ACCOUNT_MAX_SIZE
            )?;
            debug_print!("create_account done");

            invoke(
                &create_associated_token_account(funding_info.key, account_info.key, token_account_info.key, &token_mint::id()),
                accounts,
            )?;
            debug_print!("create_associated_token_account done");

            AccountData::Account(Account {
                ether,
                nonce,
                trx_count: 0_u64,
                code_account: code_account_key,
                rw_blocked_acc: None,
                eth_token_account: *token_account_info.key,
                ro_blocked_cnt: 0_u8,
            }).pack(&mut account_info.data.borrow_mut())?;

            Ok(())
        },
        EvmInstruction::ERC20CreateTokenAccount => {
            let payer = next_account_info(account_info_iter)?;
            let account = next_account_info(account_info_iter)?;
            let wallet = next_account_info(account_info_iter)?;
            let contract = next_account_info(account_info_iter)?;
            let token_mint = next_account_info(account_info_iter)?;
            let system_program = next_account_info(account_info_iter)?;
            let token_program = next_account_info(account_info_iter)?;
            let rent = next_account_info(account_info_iter)?;

            authorized_operator_check(payer)?;

            if !payer.is_signer {
                return Err!(ProgramError::InvalidArgument; "!payer.is_signer");
            }

            let wallet_data = AccountData::unpack(&wallet.try_borrow_data()?)?;
            let wallet_data = wallet_data.get_account()?;

            let contract_data = AccountData::unpack(&contract.try_borrow_data()?)?;
            let contract_data = contract_data.get_account()?;
            if contract_data.code_account == Pubkey::new_from_array([0_u8; 32]) {
                return Err!(ProgramError::InvalidArgument; "contract_data.code_account == Pubkey::new_from_array([0_u8; 32])");
            }

            if *token_mint.owner != spl_token::id() {
                return Err!(ProgramError::InvalidArgument; "*token_mint.owner<{:?}> != spl_token::id()", token_mint.owner);
            }
            spl_token::state::Mint::unpack(&token_mint.try_borrow_data()?)?;

            if *system_program.key != solana_program::system_program::id() {
                return Err!(ProgramError::InvalidArgument; "*system_program.key<{:?}> != solana_program::system_program::id()", system_program.key);
            }

            if *token_program.key != spl_token::id() {
                return Err!(ProgramError::InvalidArgument; "*token_program.key<{:?}> != spl_token::id()", token_program.key);
            }

            if *rent.key != solana_program::sysvar::rent::id() {
                return Err!(ProgramError::InvalidArgument; "*rent.key<{:?}> != solana_program::sysvar::rent::id()", rent.key);
            }

            let token_mint_key_bytes = token_mint.key.to_bytes();
            let mut seeds: Vec<&[u8]> = vec![
                &[ACCOUNT_SEED_VERSION],
                b"ERC20Balance",
                &token_mint_key_bytes,
                contract_data.ether.as_bytes(),
                wallet_data.ether.as_bytes()
            ];
            let (expected_address, nonce) = Pubkey::find_program_address(&seeds, program_id);

            if *account.key != expected_address {
                return Err!(ProgramError::InvalidArgument; "*account.key<{:?}> != expected_address", account.key);
            }

            let nonce_bytes = &[nonce];
            seeds.push(nonce_bytes);

            debug_print!("Create program derived account");
            create_pda_account(
                &spl_token::id(),
                accounts,
                account,
                &seeds,
                payer.key,
                spl_token::state::Account::LEN
            )?;

            debug_print!("Initialize token");
            let instruction = spl_token::instruction::initialize_account(
                &spl_token::id(),
                account.key,
                token_mint.key,
                wallet.key
            )?;
            invoke(&instruction, accounts)
        },
        // TODO: EvmInstruction::Call
        // https://github.com/neonlabsorg/neon-evm/issues/188
        // Does not fit in current vision.
        // It is needed to update behavior for all system in whole.
        // EvmInstruction::Call {collateral_pool_index, bytes} => {
        //     let operator_sol_info = next_account_info(account_info_iter)?;
        //     let collateral_pool_sol_info = next_account_info(account_info_iter)?;
        //     let system_info = next_account_info(account_info_iter)?;

        //     let trx_accounts = &accounts[3..];

        //     let mut account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;
        //     if let Sender::Solana(_addr) = account_storage.get_sender() {
        //         // Success execution
        //     } else {
        //         return Err!(ProgramError::InvalidArgument; "This method should used with Solana sender");
        //     }

        //     payment::transfer_from_operator_to_collateral_pool(
        //         program_id,
        //         collateral_pool_index,
        //         operator_sol_info,
        //         collateral_pool_sol_info,
        //         system_info)?;

        //     let call_return = do_call(&mut account_storage, trx_accounts, bytes.to_vec(), U256::zero(), u64::MAX)?;

        //     if let Some(call_results) = call_return {
        //         applies_and_invokes(
        //             program_id,
        //             &mut account_storage,
        //             trx_accounts,
        //             None,
        //             call_results)?;
        //     }

        //     Ok(())
        // },
        EvmInstruction::ExecuteTrxFromAccountDataIterative{collateral_pool_index, step_count} => {
            debug_print!("Execute iterative transaction from account data");
            let holder_info = next_account_info(account_info_iter)?;
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let holder_data = holder_info.data.borrow();
            let (unsigned_msg, signature) = get_transaction_from_data(&holder_data)?;

            let trx_accounts = &accounts[5..];

            let from_addr = verify_tx_signature(signature, unsigned_msg).map_err(|e| E!(ProgramError::MissingRequiredSignature; "Error={:?}", e))?;
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            do_begin(collateral_pool_index, step_count, from_addr, trx,
                program_id, trx_accounts, storage_info,
                operator_sol_info, collateral_pool_sol_info,
                system_info)?;

            Ok(())
        },
        EvmInstruction::CallFromRawEthereumTX {collateral_pool_index, from_addr, sign: _, unsigned_msg} => {
            debug_print!("Execute from raw ethereum transaction");
            // Get six accounts needed for payments (note slice accounts[6..] later)
            let sysvar_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[6..];

            if !operator_sol_info.is_signer {
                return Err!(ProgramError::InvalidAccountData);
            }

            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;
            let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
            // let trx_gas_price = u64::try_from(trx.gas_price).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
            // if trx_gas_price < 1_000_000_000_u64 {
            //     return Err!(ProgramError::InvalidArgument; "trx_gas_price < 1_000_000_000_u64: {} ", trx_gas_price);
            // }
            StorageAccount::check_for_blocked_accounts(program_id, trx_accounts, true)?;
            let mut account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 5_u16)?;
            check_ethereum_transaction(&account_storage, &H160::from_slice(from_addr), &trx)?;

            payment::transfer_from_operator_to_collateral_pool(
                program_id,
                collateral_pool_index,
                operator_sol_info,
                collateral_pool_sol_info,
                system_info)?;

            let call_return = do_call(&mut account_storage, trx.call_data, trx.value, trx_gas_limit)?;

            if let Some(call_results) = call_return {
                if get_token_account_owner(operator_eth_info)? != *operator_sol_info.key {
                    debug_print!("operator ownership");
                    debug_print!("operator token owner {}", operator_eth_info.owner);
                    debug_print!("operator key {}", operator_sol_info.key);
                    return Err!(ProgramError::InvalidInstructionData; "Wrong operator token ownership")
                }
                let used_gas = call_results.1;
                let fee = U256::from(used_gas)
                    .checked_mul(trx.gas_price).ok_or_else(||E!(ProgramError::InvalidArgument))?;
                token::transfer_token(
                    accounts,
                    user_eth_info,
                    operator_eth_info,
                    account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                    account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                    &fee)?;

                applies_and_invokes(
                    program_id,
                    &mut account_storage,
                    accounts,
                    Some(operator_sol_info),
                    call_results)?;
            }
            Ok(())
        },
        EvmInstruction::OnReturn {status: _, bytes: _} => {
            Ok(())
        },
        EvmInstruction::OnEvent {address: _, topics: _, data: _} => {
            Ok(())
        },
        EvmInstruction::PartialCallFromRawEthereumTX {collateral_pool_index, step_count, from_addr, sign: _, unsigned_msg} => {
            debug_print!("Execute from raw ethereum transaction iterative");
            let storage_info = next_account_info(account_info_iter)?;

            let sysvar_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[5..];

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 13_u16)?;

            let caller = H160::from_slice(from_addr);
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            do_begin(collateral_pool_index, step_count, caller, trx,
                     program_id, trx_accounts, storage_info,
                     operator_sol_info, collateral_pool_sol_info,
                     system_info)?;

            Ok(())
        },
        EvmInstruction::Continue { step_count } => {
            debug_print!("Continue");
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[5..];

            let storage = StorageAccount::restore(storage_info, operator_sol_info).map_err(|err| {
                if err == ProgramError::InvalidAccountData {EvmLoaderError::StorageAccountUninitialized.into()}
                else {err}
            })?;

            do_continue_top_level(storage, step_count, program_id,
                accounts, trx_accounts, storage_info,
                operator_sol_info, operator_eth_info, user_eth_info,
            )?;

            Ok(())
        },
        EvmInstruction::Cancel => {
            debug_print!("Cancel");
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let incinerator_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[6..];

            if !operator_sol_info.is_signer {
                return Err!(ProgramError::InvalidAccountData);
            }

            let storage = StorageAccount::restore(storage_info, operator_sol_info).map_err(|err| {
                if err == ProgramError::InvalidAccountData {EvmLoaderError::StorageAccountUninitialized.into()}
                else {err}
            })?;

            let custom_error: ProgramError = EvmLoaderError::ExclusiveAccessUnvailable.into();
            if let Err(err) = storage.check_accounts(program_id, trx_accounts, false){
                if err == custom_error {return Ok(())}
                return Err(err)
            }

            let account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;

            let caller_account_info = account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?;
            let mut caller_account_data = AccountData::unpack(&caller_account_info.try_borrow_data()?)?;
            let mut caller_account = caller_account_data.get_mut_account()?;

            let (caller, _nonce) = storage.caller_and_nonce()?;
            if caller_account.ether != caller {
                return Err!(ProgramError::InvalidAccountData; "acc.ether<{:?}> != caller<{:?}>", caller_account.ether, caller);
            }
            caller_account.trx_count += 1;

            caller_account_data.pack(&mut caller_account_info.try_borrow_mut_data()?)?;

            let executor = Machine::restore(&storage, &account_storage);
            debug_print!("Executor restored");

            let executor_state = executor.into_state();
            let used_gas = executor_state.substate().metadata().gasometer().used_gas();

            let (gas_limit, gas_price) = storage.get_gas_params()?;
            if used_gas > gas_limit {
                return Err!(ProgramError::InvalidArgument);
            }
            let gas_price_wei = U256::from(gas_price);
            let fee = U256::from(used_gas)
                .checked_mul(gas_price_wei).ok_or_else(||E!(ProgramError::InvalidArgument))?;

            let caller_info= account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?;

            token::transfer_token(
                accounts,
                user_eth_info,
                operator_eth_info,
                caller_info,
                account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
                &fee)?;

            payment::burn_operators_deposit(
                storage_info,
                incinerator_info,
            )?;

            storage.unblock_accounts_and_destroy(program_id, trx_accounts)?;

            Ok(())
        },
        EvmInstruction::PartialCallOrContinueFromRawEthereumTX {collateral_pool_index, step_count, from_addr, sign: _, unsigned_msg} => {
            debug_print!("Execute from raw ethereum transaction iterative or continue");
            let storage_info = next_account_info(account_info_iter)?;
            let sysvar_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[7..];

            match StorageAccount::restore(storage_info, operator_sol_info) {
                Err(ProgramError::InvalidAccountData) => { // EXCLUDE Err!
                    check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 13_u16)?;

                    let caller = H160::from_slice(from_addr);
                    let trx: UnsignedTransaction = rlp::decode(unsigned_msg)
                        .map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

                    do_begin(collateral_pool_index, step_count, caller, trx,
                             program_id, trx_accounts, storage_info,
                             operator_sol_info, collateral_pool_sol_info,
                             system_info)?;
                },
                Ok(storage) => {
                    do_continue_top_level(storage, step_count, program_id,
                        accounts, trx_accounts, storage_info,
                        operator_sol_info, operator_eth_info, user_eth_info,
                    )?;
                },
                Err(err) => return Err(err),
            }
            Ok(())
        },
        EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinue{collateral_pool_index, step_count} => {
            debug_print!("Execute iterative transaction from account data or continue");
            let holder_info = next_account_info(account_info_iter)?;
            let storage_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;
            
            let trx_accounts = &accounts[7..];

            match StorageAccount::restore(storage_info, operator_sol_info) {
                Err(ProgramError::InvalidAccountData) => { // EXCLUDE Err!
                    let holder_data = holder_info.data.borrow();
                    let (unsigned_msg, signature) = get_transaction_from_data(&holder_data)?;
                    let caller = verify_tx_signature(signature, unsigned_msg).map_err(|e| E!(ProgramError::MissingRequiredSignature; "Error={:?}", e))?;
                    let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

                    do_begin(collateral_pool_index, step_count, caller, trx,
                             program_id, trx_accounts, storage_info,
                             operator_sol_info, collateral_pool_sol_info,
                             system_info)?;
                },
                Ok(storage) => {
                    do_continue_top_level(storage, step_count, program_id,
                                          accounts, trx_accounts, storage_info,
                                          operator_sol_info, operator_eth_info, user_eth_info,
                    )?;
                },
                Err(err) => return Err(err),
            }
            Ok(())
        },
        EvmInstruction::DeleteAccount { seed } => {
            let deleted_acc_info = next_account_info(account_info_iter)?;
            let creator_acc_info = next_account_info(account_info_iter)?;

            if !creator_acc_info.is_signer {
                return Err!(ProgramError::InvalidAccountData; "Creator acc must be signer. Acc {:?}", *creator_acc_info.key);
            }

            let address = Pubkey::create_with_seed(
                creator_acc_info.key, 
                std::str::from_utf8(seed).map_err(|e| E!(ProgramError::InvalidInstructionData; "Seed decode error={:?}", e))?, 
                program_id)?;

            if *deleted_acc_info.key != address {
                return Err!(ProgramError::InvalidAccountData; "Deleted account info doesn't equal to generated. *deleted_acc_info.key<{:?}> != address<{:?}>", *deleted_acc_info.key, address);
            }

            let data = deleted_acc_info.data.borrow_mut();
            let account_data = AccountData::unpack(&data)?;
            match account_data {
                AccountData::Empty => { },
                _ => { return Err!(ProgramError::InvalidAccountData; "Can only delete empty accounts.") },
            };

            **creator_acc_info.lamports.borrow_mut() = creator_acc_info.lamports().checked_add(deleted_acc_info.lamports()).unwrap();
            **deleted_acc_info.lamports.borrow_mut() = 0;

            Ok(())
        },
        EvmInstruction::ResizeStorageAccount {seed} => {
            debug_print!("Execute ResizeStorageAccount");
            let account_info = next_account_info(account_info_iter)?;
            let code_account = next_account_info(account_info_iter)?;
            let code_account_new = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;

            let mut info_data = account_info.try_borrow_mut_data()?;
            if let AccountData::Account(mut data) = AccountData::unpack(&info_data)? {
                if data.rw_blocked_acc.is_some() || data.ro_blocked_cnt >0 {
                    debug_print!("Cannot resize account data. Account is blocked {:?}", *account_info.key);
                    return Ok(())
                }
                data.code_account = *code_account_new.key;
                AccountData::pack(&AccountData::Account(data), &mut info_data)?;
            } else {
                return Err!(ProgramError::InvalidAccountData)
            }

            if code_account_new.owner == program_id {
                let rent = Rent::get()?;
                if !rent.is_exempt(code_account_new.lamports(), code_account_new.data_len()) {
                    return Err!(ProgramError::InvalidArgument; "Code account is not rent exempt. lamports={:?}, data_len={:?}",
                        code_account_new.lamports(), code_account_new.data_len());
                }
            }
            else {
                return Err!(ProgramError::InvalidArgument; "code_account_new.owner<{:?}> != program_id<{:?}>", code_account_new.owner, program_id);
            }

            let mut code_account_new_data = code_account_new.try_borrow_mut_data()?;

            match AccountData::unpack(&code_account_new_data) {
                Ok(AccountData::Empty) => {},
                _ =>  return Err!(ProgramError::InvalidAccountData)
            }

            if code_account.owner != program_id {
                return Err!(ProgramError::InvalidArgument; "code_account.owner<{:?}> != program_id<{:?}>", code_account.owner, program_id);
            }

            let expected_address = Pubkey::create_with_seed(
                operator_sol_info.key,
                std::str::from_utf8(seed).map_err(|e| E!(ProgramError::InvalidInstructionData; "Seed decode error={:?}", e))?,
                program_id)?;

            if *code_account_new.key != expected_address {
                return Err!(ProgramError::InvalidArgument; "new code_account must be created by operator, new code_account {:?}, operator {:?}, expected address {:?}",
                    *code_account_new.key, *operator_sol_info.key, expected_address);
            }

            let mut code_account_data = code_account.try_borrow_mut_data()?;
            if code_account_data.len() >= code_account_new_data.len(){
                return Err!(ProgramError::InvalidArgument; "current account size >= new account size, account={:?}, size={:?}, new_size={:?}",
                        *account_info.key, code_account_data.len(), code_account_new_data.len());
            }
            if let AccountData::Contract(data) = AccountData::unpack(&code_account_data)? {
                if data.owner != *account_info.key {
                    return Err!(ProgramError::InvalidAccountData;
                            "code_account.data.owner!=contract.key,  code_account.data.owner={:?}, contract.key={:?}", data.owner, *account_info.key)
                }
                debug_print!("move code and storage from {:?} to {:?}", *code_account.key, *code_account_new.key);
                AccountData::pack(&AccountData::Contract(data.clone()), &mut code_account_new_data)?;
                let begin = AccountData::Contract(data).size();
                let end = code_account_data.len();
                code_account_new_data[begin..end].copy_from_slice(&code_account_data[begin..]);

                AccountData::pack(&AccountData::Empty, &mut code_account_data)?;

            } else {
                return Err!(ProgramError::InvalidAccountData)
            };

            Ok(())
        },
        EvmInstruction::WriteHolder {seed, offset, bytes} => {
            let holder_info = next_account_info(account_info_iter)?;
            if holder_info.owner != program_id {
                return Err!(ProgramError::InvalidArgument; "holder_account_info.owner<{:?}> != program_id<{:?}>", holder_info.owner, program_id);
            }

            let operator_info = next_account_info(account_info_iter)?;
            if !operator_info.is_signer {
                return Err!(ProgramError::InvalidArgument; "operator is not signer <{:?}>", operator_info.key);
            }

            let seed = core::str::from_utf8(&seed);
            if seed.is_err() {
                return Err!(ProgramError::InvalidArgument; "invalid seed <{:?}>", seed);
            }

            let must_holder = Pubkey::create_with_seed(operator_info.key, seed.unwrap(), program_id);
            if must_holder.is_err() {
                return Err!(ProgramError::InvalidArgument; "invalid seed <{:?}>", seed.unwrap());
            }

            if *holder_info.key != must_holder.unwrap() {
                return Err!(ProgramError::InvalidArgument; "wrong holder account <{:?}>", holder_info.key);
            }

            do_write(holder_info, offset, bytes)
        },

        EvmInstruction::Write |
        EvmInstruction::Finalise |
        EvmInstruction::CreateAccountWithSeed => Err!(ProgramError::InvalidInstructionData; "Deprecated instruction"),
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
        AccountData::Account(_) | AccountData::Storage(_) | AccountData::ERC20Allowance(_) => {
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

fn do_call(
    account_storage: &mut ProgramAccountStorage<'_>,
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> CallResult
{
    debug_print!("do_call");

    debug_print!("   caller: {}", account_storage.origin());
    debug_print!(" contract: {}", account_storage.contract());

    let call_results = {
        let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, account_storage));
        let executor_state = ExecutorState::new(executor_substate, account_storage);
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
            let apply = executor_state.deconstruct();
            (exit_reason, used_gas, result, Some(apply))
        } else {
            (exit_reason, used_gas, result, None)
        }
    };

    Ok(Some(call_results))
}

#[allow(clippy::too_many_arguments)]
fn do_begin<'a>(
    collateral_pool_index: u32,
    step_count: u64,
    caller: H160,
    trx: UnsignedTransaction,
    program_id: &Pubkey,
    trx_accounts: &'a [AccountInfo<'a>],
    storage_info: &'a AccountInfo<'a>,
    operator_sol_info: &'a AccountInfo<'a>,
    collateral_pool_sol_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>
) -> ProgramResult
{
    if !operator_sol_info.is_signer {
        return Err!(ProgramError::InvalidAccountData);
    }

    let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
    let trx_gas_price = u64::try_from(trx.gas_price).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;

    let mut storage = StorageAccount::new(storage_info, operator_sol_info, trx_accounts, caller, trx.nonce, trx_gas_limit, trx_gas_price)?;
    StorageAccount::check_for_blocked_accounts(program_id, trx_accounts, false)?;
    let account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;
    check_ethereum_transaction(&account_storage, &caller, &trx)?;

    payment::transfer_from_operator_to_collateral_pool(
        program_id,
        collateral_pool_index,
        operator_sol_info,
        collateral_pool_sol_info,
        system_info)?;
    payment::transfer_from_operator_to_deposit(
        operator_sol_info,
        storage_info,
        system_info)?;

    if trx.to.is_some() {
        do_partial_call(&mut storage, step_count, &account_storage, trx.call_data, trx.value, trx_gas_limit)?;
    }
    else {
        do_partial_create(&mut storage, step_count, &account_storage, trx.call_data, trx.value, trx_gas_limit)?;
    }

    storage.block_accounts(program_id, trx_accounts)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn do_continue_top_level<'a>(
    mut storage: StorageAccount,
    step_count: u64,
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    trx_accounts: &'a [AccountInfo<'a>],
    storage_info: &'a AccountInfo<'a>,
    operator_sol_info: &'a AccountInfo<'a>,
    operator_eth_info: &'a AccountInfo<'a>,
    user_eth_info: &'a AccountInfo<'a>,
) -> ProgramResult
{
    if !operator_sol_info.is_signer {
        return Err!(ProgramError::InvalidAccountData);
    }

    let custom_error: ProgramError = EvmLoaderError::ExclusiveAccessUnvailable.into();
    if let Err(err) = storage.check_accounts(program_id, trx_accounts, true){
        if err == custom_error {
            return Ok(())}
        return Err(err)
    }

    let mut account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;
    let call_return = do_continue(&mut storage, step_count, &mut account_storage)?;

    if let Some(call_results) = call_return {
        payment::transfer_from_deposit_to_operator(
            storage_info,
            operator_sol_info,
        )?;
        if get_token_account_owner(operator_eth_info)? != *operator_sol_info.key {
            debug_print!("operator token ownership");
            debug_print!("operator token owner {}", operator_eth_info.owner);
            debug_print!("operator key {}", operator_sol_info.key);
            return Err!(ProgramError::InvalidInstructionData; "Wrong operator token ownership")
        }
        let (gas_limit, gas_price) = storage.get_gas_params()?;
        let used_gas = call_results.1;
        if used_gas > gas_limit {
            return Err!(ProgramError::InvalidArgument);
        }
        let gas_price_wei = U256::from(gas_price);
        let fee = U256::from(used_gas)
            .checked_mul(gas_price_wei).ok_or_else(||E!(ProgramError::InvalidArgument))?;
        token::transfer_token(
            accounts,
            user_eth_info,
            operator_eth_info,
            account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?,
            account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?,
            &fee)?;

        applies_and_invokes(
            program_id,
            &mut account_storage,
            accounts,
            Some(operator_sol_info),
            call_results)?;

        storage.unblock_accounts_and_destroy(program_id, trx_accounts)?;
    }

    Ok(())
}

fn do_partial_call<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &ProgramAccountStorage<'a>,
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> ProgramResult
{
    debug_print!("do_partial_call");

    let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, account_storage));
    let executor_state = ExecutorState::new(executor_substate, account_storage);
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
    account_storage: &ProgramAccountStorage<'a>,
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> ProgramResult
{
    debug_print!("do_partial_create gas_limit={}", gas_limit);

    let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, account_storage));
    let executor_state = ExecutorState::new(executor_substate, account_storage);
    let mut executor = Machine::new(executor_state);

    debug_print!("Executor initialized");

    executor.create_begin(account_storage.origin(), instruction_data, transfer_value, gas_limit)?;
    executor.execute_n_steps(step_count).unwrap();

    debug_print!("save");
    executor.save_into(storage);

    debug_print!("partial create complete");
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn do_continue<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> CallResult
{
    debug_print!("do_continue");

    let call_results = {
        let mut executor = Machine::restore(storage, account_storage);
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
            let apply = executor_state.deconstruct();
            (exit_reason, used_gas, result, Some(apply))
        } else {
            (exit_reason, used_gas, result, None)
        }
    };

    Ok(Some(call_results))
}

fn applies_and_invokes<'a>(
    program_id: &Pubkey,
    account_storage: &mut ProgramAccountStorage<'a>,
    accounts: &'a [AccountInfo<'a>],
    operator: Option<&AccountInfo<'a>>,
    call_results: SuccessExitResults
) -> ProgramResult {
    let (exit_reason, used_gas, result, applies_logs_transfers) = call_results;
    if let Some(applies_logs_transfers) = applies_logs_transfers {
        let (
            applies,
            logs,
            transfers,
            spl_transfers,
            spl_approves,
            erc20_approves,
        ) = applies_logs_transfers;

        account_storage.apply_transfers(accounts, transfers)?;
        account_storage.apply_spl_approves(accounts, spl_approves)?;
        account_storage.apply_spl_transfers(accounts, spl_transfers)?;
        account_storage.apply_erc20_approves(accounts, operator, erc20_approves)?;
        account_storage.apply(applies, operator, false)?;

        debug_print!("Applies done");
        for log in logs {
            invoke(&on_event(program_id, log), accounts)?;
        }
    }

    invoke_on_return(program_id, accounts, exit_reason, used_gas, &result)?;

    Ok(())
}

fn invoke_on_return<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    exit_reason: ExitReason,
    used_gas: u64,
    result: &[u8],
) -> ProgramResult
{
    let (exit_message, exit_status) = match exit_reason {
        ExitReason::Succeed(success_code) => {
            match success_code {
                ExitSucceed::Stopped => {("ExitSucceed: Machine encountered an explict stop.", 0x11)},
                ExitSucceed::Returned => {("ExitSucceed: Machine encountered an explict return.", 0x12)},
                ExitSucceed::Suicided => {("ExitSucceed: Machine encountered an explict suicide.", 0x13)},
            }
        },
        ExitReason::Error(error_code) => {
            match error_code {
                ExitError::StackUnderflow => {("ExitError: Trying to pop from an empty stack.", 0xe1)},
                ExitError::StackOverflow => {("ExitError: Trying to push into a stack over stack limit.", 0xe2)},
                ExitError::InvalidJump => {("ExitError: Jump destination is invalid.", 0xe3)},
                ExitError::InvalidRange => {("ExitError: An opcode accesses memory region, but the region is invalid.", 0xe4)},
                ExitError::DesignatedInvalid => {("ExitError: Encountered the designated invalid opcode.", 0xe5)},
                ExitError::CallTooDeep => {("ExitError: Call stack is too deep (runtime).", 0xe6)},
                ExitError::CreateCollision => {("ExitError: Create opcode encountered collision (runtime).", 0xe7)},
                ExitError::CreateContractLimit => {("ExitError: Create init code exceeds limit (runtime).", 0xe8)},
                ExitError::OutOfOffset => {("ExitError: An opcode accesses external information, but the request is off offset limit (runtime).", 0xe9)},
                ExitError::OutOfGas => {("ExitError: Execution runs out of gas (runtime).", 0xea)},
                ExitError::OutOfFund => {("ExitError: Not enough fund to start the execution (runtime).", 0xeb)},
                ExitError::PCUnderflow => {("ExitError: PC underflowed (unused).", 0xec)},
                ExitError::CreateEmpty => {("ExitError: Attempt to create an empty account (runtime, unused).", 0xed)},
            }
        },
        ExitReason::Revert(_) => {("Revert", 0xd0)},
        ExitReason::Fatal(fatal_code) => {
            match fatal_code {
                ExitFatal::NotSupported => {("Fatal: The operation is not supported.", 0xf1)},
                ExitFatal::UnhandledInterrupt => {("Fatal: The trap (interrupt) is unhandled.", 0xf2)},
                ExitFatal::CallErrorAsFatal(_) => {("Fatal: The environment explictly set call errors as fatal error.", 0xf3)},
            }
        },
        ExitReason::StepLimitReached => unreachable!(),
    };

    msg!("{} exit_status={:#04X?}", exit_message, exit_status);
    debug_print!("used gas {}", used_gas);
    debug_print!("result {}", &hex::encode(&result));

    let ix = on_return(program_id, exit_status, used_gas, result);
    invoke(
        &ix,
        accounts
    )?;

    Ok(())
}

fn check_ethereum_transaction(
   account_storage: &ProgramAccountStorage,
   recovered_address: &H160,
   transaction: &UnsignedTransaction
) -> ProgramResult
{
    let sender_account = account_storage.get_caller_account().ok_or_else(||E!(ProgramError::InvalidArgument))?;

    if sender_account.get_ether() != *recovered_address {
        return Err!(ProgramError::InvalidArgument; "Invalid sender: actual {}, recovered {}", sender_account.get_ether(), recovered_address);
    }

    if sender_account.get_nonce() != transaction.nonce {
        return Err!(ProgramError::InvalidArgument; "Invalid Ethereum transaction nonce: acc {}, trx {}", sender_account.get_nonce(), transaction.nonce);
    }

    let contract_address: H160 = transaction.to.map_or_else(
        || {
            let mut stream = rlp::RlpStream::new_list(2);
            stream.append(recovered_address);
            stream.append(&U256::from(transaction.nonce));
            crate::utils::keccak256_h256(&stream.out()).into()
        },
        |to| to
    );
    let contract_account = account_storage.get_contract_account().ok_or_else(||E!(ProgramError::InvalidArgument))?;

    if contract_account.get_ether() != contract_address {
        return Err!(ProgramError::InvalidArgument; "Invalid contract: actual {}, expected {}", contract_account.get_ether(), contract_address);
    }


    if chain_id() != transaction.chain_id {
        return Err!(ProgramError::InvalidArgument; "Invalid chain_id: actual {}, expected {}", transaction.chain_id, chain_id());
    }


    Ok(())
}

// Pull in syscall stubs when building for non-BPF targets
//#[cfg(not(target_arch = "bpf"))]
//solana_sdk::program_stubs!();

#[cfg(test)]
mod tests {
    use solana_sdk::{program_error::ProgramError, pubkey::Pubkey};

    use super::*;

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

