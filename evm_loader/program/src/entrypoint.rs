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
    account_info::{AccountInfo, next_account_info},
    entrypoint, entrypoint::{HEAP_START_ADDRESS, ProgramResult},
    keccak::Hasher, msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use crate::{
    //    bump_allocator::BumpAllocator,
    account_data::{Account, ACCOUNT_MAX_SIZE, ACCOUNT_SEED_VERSION, AccountData, Contract},
    account_storage::{ProgramAccountStorage, /* Sender */ },
    config::{chain_id, token_mint},
    error::EvmLoaderError,
    executor::Machine,
    executor_state::{ApplyState, ExecutorState, ExecutorSubstate},
    instruction::{EvmInstruction, on_event, on_return},
    operator::authorized_operator_check,
    payment,
    solana_backend::AccountStorage,
    storage_account::StorageAccount,
    system::create_pda_account,
    token,
    token::{
        check_token_mint,
        create_associated_token_account,
        get_token_account_balance,
        get_token_account_delegated_amount
    },
    transaction::{check_secp256k1_instruction, UnsignedTransaction, verify_tx_signature},
    utils::is_zero_initialized
};
use crate::solana_program::program_pack::Pack;

type UsedGas = u64;
type EvmResults = (ExitReason, Vec<u8>, Option<ApplyState>);
type CallResult = Result<(Option<EvmResults>,UsedGas), ProgramError>;

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
        EvmInstruction::ExecuteTrxFromAccountDataIterativeV02 {collateral_pool_index, step_count} => {
            debug_print!("Execute iterative transaction from account data");
            let holder_info = next_account_info(account_info_iter)?;
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[7..];

            let holder_data = holder_info.data.borrow();
            let (unsigned_msg, signature) = get_transaction_from_data(&holder_data)?;

            let caller = verify_tx_signature(signature, unsigned_msg).map_err(|e| E!(ProgramError::MissingRequiredSignature; "Error={:?}", e))?;
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            do_begin(
                collateral_pool_index, step_count, caller, trx,
                program_id, trx_accounts, accounts, storage_info,
                operator_sol_info, collateral_pool_sol_info,
                operator_eth_info, user_eth_info,
                system_info,
                signature
            )?;

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
            let trx_gas_price = u64::try_from(trx.gas_price).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
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

            let (evm_results, used_gas) = do_call(&mut account_storage, trx.call_data, trx.value, trx_gas_limit)?;

            token::user_pays_operator(
                trx_gas_price,
                used_gas,
                user_eth_info,
                operator_eth_info,
                accounts,
                &account_storage
            )?;

            applies_and_invokes(
                program_id,
                &mut account_storage,
                accounts,
                operator_sol_info,
                evm_results.unwrap(),
                used_gas)?;

            Ok(())
        },
        EvmInstruction::OnReturn {status: _, bytes: _} => {
            Ok(())
        },
        EvmInstruction::OnEvent {address: _, topics: _, data: _} => {
            Ok(())
        },
        EvmInstruction::PartialCallFromRawEthereumTXv02 {collateral_pool_index, step_count, from_addr, sign, unsigned_msg} => {
            debug_print!("Execute from raw ethereum transaction iterative");
            let storage_info = next_account_info(account_info_iter)?;

            let sysvar_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[7..];

            check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 13_u16)?;

            let caller = H160::from_slice(from_addr);
            let trx: UnsignedTransaction = rlp::decode(unsigned_msg)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

            do_begin(
                collateral_pool_index, step_count, caller, trx,
                program_id, trx_accounts, accounts, storage_info,
                operator_sol_info, collateral_pool_sol_info,
                operator_eth_info, user_eth_info,
                system_info,
                sign
            )?;

            Ok(())
        },
        EvmInstruction::ContinueV02 { collateral_pool_index, step_count } => {
            debug_print!("Continue");
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let collateral_pool_sol_info = next_account_info(account_info_iter)?;
            let operator_eth_info = next_account_info(account_info_iter)?;
            let user_eth_info = next_account_info(account_info_iter)?;
            let system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[6..];

            let storage = StorageAccount::restore(storage_info, operator_sol_info)?;
            do_continue_top_level(
                storage, step_count, program_id,
                accounts, trx_accounts, storage_info,
                operator_sol_info, operator_eth_info, user_eth_info,
                collateral_pool_index, collateral_pool_sol_info, system_info,
            )?;

            Ok(())
        },
        EvmInstruction::CancelWithNonce { nonce } => {
            debug_print!("CancelWithNonce");
            let storage_info = next_account_info(account_info_iter)?;

            let operator_sol_info = next_account_info(account_info_iter)?;
            let _operator_eth_info = next_account_info(account_info_iter)?;
            let _user_eth_info = next_account_info(account_info_iter)?;
            let incinerator_info = next_account_info(account_info_iter)?;
            // let _system_info = next_account_info(account_info_iter)?;

            authorized_operator_check(operator_sol_info)?;

            let trx_accounts = &accounts[6..];

            if !operator_sol_info.is_signer {
                return Err!(ProgramError::InvalidAccountData);
            }

            let storage = StorageAccount::restore(storage_info, operator_sol_info)?;

            let custom_error: ProgramError = EvmLoaderError::ExclusiveAccessUnvailable.into();
            if let Err(err) = storage.check_accounts(program_id, trx_accounts, false){
                if err == custom_error {return Ok(())}
                return Err(err)
            }

            let account_storage = ProgramAccountStorage::new(program_id, trx_accounts)?;

            let caller_account_info = account_storage.get_caller_account_info();
            let mut caller_account_data = AccountData::unpack(&caller_account_info.try_borrow_data()?)?;
            let mut caller_account = caller_account_data.get_mut_account()?;

            let (caller, trx_nonce) = storage.caller_and_nonce()?;
            if trx_nonce != nonce {
                return Err!(ProgramError::InvalidInstructionData; "trx_nonce<{:?}> != nonce<{:?}>", trx_nonce, nonce);
            }
            if caller_account.ether != caller {
                return Err!(ProgramError::InvalidAccountData; "acc.ether<{:?}> != caller<{:?}>", caller_account.ether, caller);
            }
            caller_account.trx_count += 1;

            caller_account_data.pack(&mut caller_account_info.try_borrow_mut_data()?)?;

            payment::burn_operators_deposit(
                storage_info,
                incinerator_info,
            )?;

            storage.unblock_accounts_and_finalize(program_id, trx_accounts)?;

            Ok(())
        },
        EvmInstruction::PartialCallOrContinueFromRawEthereumTX {collateral_pool_index, step_count, from_addr, sign, unsigned_msg} => {
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
                Err(err) => {
                    let caller = H160::from_slice(from_addr);

                    let finalized_is_outdated = if err == EvmLoaderError::StorageAccountFinalized.into(){
                        StorageAccount::finalized_is_outdated(storage_info, sign, &caller)?
                    }
                    else{
                        false
                    };

                    if err == EvmLoaderError::StorageAccountUninitialized.into() || finalized_is_outdated {
                        check_secp256k1_instruction(sysvar_info, unsigned_msg.len(), 13_u16)?;

                        let trx: UnsignedTransaction = rlp::decode(unsigned_msg)
                            .map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

                        do_begin(
                            collateral_pool_index, 0, caller, trx,
                            program_id, trx_accounts, accounts, storage_info,
                            operator_sol_info, collateral_pool_sol_info,
                            operator_eth_info, user_eth_info,
                            system_info,
                            sign
                        )?;
                    }
                    else{
                        return Err(err)
                    }
                },
                Ok(storage) => {
                    do_continue_top_level(
                        storage, step_count, program_id,
                        accounts, trx_accounts, storage_info,
                        operator_sol_info, operator_eth_info, user_eth_info,
                        collateral_pool_index, collateral_pool_sol_info, system_info,
                    )?;
                }
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
                Err(err) => {
                    let holder_data = holder_info.data.borrow();
                    let (unsigned_msg, signature) = get_transaction_from_data(&holder_data)?;
                    let caller = verify_tx_signature(signature, unsigned_msg).map_err(|e| E!(ProgramError::MissingRequiredSignature; "Error={:?}", e))?;
                    let trx: UnsignedTransaction = rlp::decode(unsigned_msg).map_err(|e| E!(ProgramError::InvalidInstructionData; "DecoderError={:?}", e))?;

                    let finalized_is_outdated = if err == EvmLoaderError::StorageAccountFinalized.into(){
                        StorageAccount::finalized_is_outdated(storage_info, signature, &caller)?
                    }
                    else{
                        false
                    };

                    if err == EvmLoaderError::StorageAccountUninitialized.into() || finalized_is_outdated {
                        do_begin(
                            collateral_pool_index, 0, caller, trx,
                            program_id, trx_accounts, accounts, storage_info,
                            operator_sol_info, collateral_pool_sol_info,
                            operator_eth_info, user_eth_info,
                            system_info,
                            signature
                        )?;
                    }
                    else{
                        return Err(err)
                    }
                },
                Ok(storage) => {
                    do_continue_top_level(
                        storage, step_count, program_id,
                        accounts, trx_accounts, storage_info,
                        operator_sol_info, operator_eth_info, user_eth_info,
                        collateral_pool_index, collateral_pool_sol_info, system_info,
                    )?;
                },
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
                AccountData::FinalizedStorage(_) | AccountData::Empty => {},
                _ => { return Err!(ProgramError::InvalidAccountData; "Can only delete finalized or empty accounts.") },
            };

            **creator_acc_info.lamports.borrow_mut() = creator_acc_info.lamports().checked_add(deleted_acc_info.lamports()).unwrap();
            **deleted_acc_info.lamports.borrow_mut() = 0;

            Ok(())
        },
        EvmInstruction::ResizeStorageAccount {seed} => {
            debug_print!("Execute ResizeStorageAccount");

            let account_info = next_account_info(account_info_iter)?;
            let code_account_info = next_account_info(account_info_iter)?;
            let code_account_new_info = next_account_info(account_info_iter)?;
            let operator_sol_info = next_account_info(account_info_iter)?;

            if !operator_sol_info.is_signer {
                return Err!(ProgramError::InvalidAccountData);
            }

            if code_account_new_info.data_len() <= code_account_info.data_len(){
                return Err!(ProgramError::InvalidAccountData; "new code account size is less than or equal to current code account size");
            }

            let mut account_data = AccountData::unpack(&account_info.try_borrow_data()?)?;
            let account = account_data.get_mut_account()?;
            if account.rw_blocked_acc.is_some() || account.ro_blocked_cnt > 0 {
                return Err!(ProgramError::InvalidInstructionData; "Cannot resize account data. Account is blocked {:?}", *account_info.key);
            }


            if account.code_account != *code_account_info.key {
                return Err!(ProgramError::InvalidArgument; "account.code_account<{:?}> != *code_account_info.key<{:?}>", account.code_account, code_account_info.key);
            }
            if (account.code_account == Pubkey::new_from_array([0; 32])) && (account.trx_count != 0) {
                return Err!(ProgramError::InvalidArgument; "Cannot change user account to contract account");
            }


            let seed = std::str::from_utf8(seed).map_err(|e| E!(ProgramError::InvalidInstructionData; "Seed decode error={:?}", e))?;
            let expected_address = Pubkey::create_with_seed(operator_sol_info.key, seed, program_id)?;
            if *code_account_new_info.key != expected_address {
                return Err!(ProgramError::InvalidArgument; "New code_account must be created by transaction signer");
            }

            AccountData::unpack(&code_account_new_info.try_borrow_data()?)?.check_empty()?;

            let rent = Rent::get()?;
            if !rent.is_exempt(code_account_new_info.lamports(), code_account_new_info.data_len()) {
                return Err!(ProgramError::InvalidArgument; "New code account is not rent exempt.");
            }


            account.code_account = *code_account_new_info.key;
            account_data.pack(&mut account_info.try_borrow_mut_data()?)?;


            if *code_account_info.key == Pubkey::new_from_array([0; 32]) {
                let contract_data = AccountData::Contract( Contract {owner: *account_info.key, code_size: 0_u32} );
                contract_data.pack(&mut code_account_new_info.try_borrow_mut_data()?)?;

                return Ok(());
            }


            debug_print!("move code and storage from {:?} to {:?}", *code_account_info.key, *code_account_new_info.key);
            let mut code_account_data = code_account_info.try_borrow_mut_data()?;
            let mut code_account_new_data = code_account_new_info.try_borrow_mut_data()?;

            code_account_new_data[..code_account_data.len()].copy_from_slice(&code_account_data);
            AccountData::pack(&AccountData::Empty, &mut code_account_data)?;

            payment::transfer_from_code_account_to_operator(code_account_info, operator_sol_info, code_account_info.lamports())?;

            Ok(())
        },
        EvmInstruction::WriteHolder { holder_id, offset, bytes} => {
            let holder_info = next_account_info(account_info_iter)?;
            if holder_info.owner != program_id {
                return Err!(ProgramError::InvalidArgument; "holder_account_info.owner<{:?}> != program_id<{:?}>", holder_info.owner, program_id);
            }

            let operator_info = next_account_info(account_info_iter)?;
            if !operator_info.is_signer {
                return Err!(ProgramError::InvalidArgument; "operator is not signer <{:?}>", operator_info.key);
            }

            // proxy_id_bytes = proxy_id.to_bytes((proxy_id.bit_length() + 7) // 8, 'big')
            // seed = keccak_256(b'holder' + proxy_id_bytes).hexdigest()[:32]
            let bytes_count = std::mem::size_of_val(&holder_id);
            let bits_count = bytes_count * 8;
            let holder_id_bit_length = bits_count - holder_id.leading_zeros() as usize;
            let significant_bytes_count = (holder_id_bit_length + 7) / 8;
            let mut hasher = Hasher::default();
            hasher.hash(b"holder");
            hasher.hash(&holder_id.to_be_bytes()[bytes_count-significant_bytes_count..]);
            let output = hasher.result();
            let seed = &hex::encode(output)[..32];

            let expected_holder_key = Pubkey::create_with_seed(operator_info.key, seed, program_id);
            if expected_holder_key.is_err() {
                return Err!(ProgramError::InvalidArgument; "invalid seed <{:?}>", seed);
            }

            if *holder_info.key != expected_holder_key.unwrap() {
                return Err!(ProgramError::InvalidArgument; "wrong holder account <{:?}>", holder_info.key);
            }

            do_write(holder_info, offset, bytes)
        },
        EvmInstruction::UpdateValidsTable => {
            let code_info = next_account_info(account_info_iter)?;
            
            let mut data = code_info.try_borrow_mut_data()?;
            let account_data = AccountData::unpack(&data)?;
            let contract = account_data.get_contract()?;

            let code_size = contract.code_size as usize;
            if code_size == 0 {
                return Err!(ProgramError::InvalidAccountData; "empty code account");
            }

            let valids = {
                let code = &data[account_data.size()..account_data.size() + code_size];
                evm::Valids::compute(code)
            };

            let expected_valids_size = (code_size / 8) + 1;
            if valids.len() != expected_valids_size {
                return Err!(ProgramError::InvalidInstructionData; "valids.len()<{}> != expected_valids_size<{}>", valids.len(), expected_valids_size);
            }

            let valids_begin = account_data.size() + code_size;
            let valids_end = account_data.size() + code_size + valids.len();
            (&mut data[valids_begin..valids_end]).copy_from_slice(&valids);

            Ok(())
        },
        EvmInstruction::Deposit => {
            let source_info = next_account_info(account_info_iter)?;
            let target_info = next_account_info(account_info_iter)?;
            let _ether_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;
            let evm_loader_info = next_account_info(account_info_iter)?;
            let token_program_info = next_account_info(account_info_iter)?;

            let amount = get_token_account_delegated_amount(source_info, authority_info)?;
            debug_print!("Deposit delegated amount {}", amount);

            transfer_deposit(
                source_info.clone(),
                target_info.clone(),
                authority_info.clone(),
                evm_loader_info,
                token_program_info.clone(),
                amount)?;
            debug_print!("Deposit transfer completed");

            Ok(())
        },
        _ => Err!(ProgramError::InvalidInstructionData; "Invalid instruction"),
    };

    solana_program::msg!("Total memory occupied: {}", &BumpAllocator::occupied());
    result
}

/// Transfer Tokens
///
/// # Errors
///
/// Could return:
/// `ProgramError::InvalidInstructionData`
#[inline(never)]
fn transfer_deposit<'a>(
    source_info: AccountInfo<'a>,
    target_info: AccountInfo<'a>,
    authority_info: AccountInfo<'a>,
    evm_loader_info: &'a AccountInfo<'a>,
    token_program_info: AccountInfo<'a>,
    value: u64,
) -> Result<(), ProgramError> {
    debug_print!("Deposit transfer_neon_token");

    check_token_mint(&source_info, &token_mint::id())?;
    check_token_mint(&target_info, &token_mint::id())?;

    let source_balance = get_token_account_balance(&source_info)?;
    if source_balance < value {
        return Err!(ProgramError::InvalidInstructionData;
            "Insufficient funds on token account {:?} {:?}",
            source_info, source_balance
        );
    }

    let (authority_key, bump_seed) =
        Pubkey::find_program_address(&[b"Deposit"], evm_loader_info.key);
    if authority_key != *authority_info.key {
        return Err!(ProgramError::InvalidInstructionData;
            "Incorrect evm token authority {:?} {:?}",
            authority_key, authority_info.key
        );
    }

    debug_print!("Transfer NEON tokens from {} to {} value {}", source_info.key, target_info.key, value);

    let transfer = spl_token::instruction::transfer(
        token_program_info.key,
        source_info.key,
        target_info.key,
        &authority_key,
        &[&authority_key],
        value
    )?;

    invoke_signed(&transfer,
                  &[source_info, target_info, authority_info, token_program_info],
                  &[&[&b"Deposit"[..], &[bump_seed]]])?;

    Ok(())
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
        AccountData::Account(_) | AccountData::Storage(_) | AccountData::ERC20Allowance(_) | AccountData::FinalizedStorage(_) => {
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

    let (evm_results,used_gas) = {
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
        let used_gas = executor_state.gasometer().used_gas();
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let apply = executor_state.deconstruct();
            ((exit_reason, result, Some(apply)), used_gas)
        } else {
            ((exit_reason, result, None), used_gas)
        }
    };

    Ok((Some(evm_results),used_gas))
}

#[allow(clippy::too_many_arguments)]
fn do_begin<'a>(
    collateral_pool_index: u32,
    step_count: u64,
    caller: H160,
    trx: UnsignedTransaction,
    program_id: &Pubkey,
    trx_accounts: &'a [AccountInfo<'a>],
    accounts: &'a [AccountInfo<'a>],
    storage_info: &'a AccountInfo<'a>,
    operator_sol_info: &'a AccountInfo<'a>,
    collateral_pool_sol_info: &'a AccountInfo<'a>,
    operator_eth_info: &'a AccountInfo<'a>,
    user_eth_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>,
    trx_sign: &[u8]
) -> ProgramResult
{
    if !operator_sol_info.is_signer {
        return Err!(ProgramError::InvalidAccountData);
    }

    let trx_gas_limit = u64::try_from(trx.gas_limit).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;
    let trx_gas_price = u64::try_from(trx.gas_price).map_err(|e| E!(ProgramError::InvalidInstructionData; "e={:?}", e))?;

    token::check_enough_funds(
        trx_gas_limit,
        trx_gas_price,
        user_eth_info,
        None)?;

    let mut storage = StorageAccount::new(storage_info, operator_sol_info, trx_accounts, caller, trx.nonce, trx_gas_limit, trx_gas_price, trx_sign)?;
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

    let (_,used_gas) = if trx.to.is_some() {
        do_partial_call(&mut storage, step_count, &account_storage, trx.call_data, trx.value, trx_gas_limit)?
    }
    else {
        do_partial_create(&mut storage, step_count, &account_storage, trx.call_data, trx.value, trx_gas_limit)?
    };

    token::user_pays_operator_for_iteration(
        trx_gas_price, used_gas,
        user_eth_info,
        operator_eth_info,
        accounts,
        &account_storage,
        &mut storage,
    )?;

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
    collateral_pool_index: u32,
    collateral_pool_sol_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>,
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
    let (trx_gas_limit, trx_gas_price) = storage.get_gas_params()?;

    payment::transfer_from_operator_to_collateral_pool(
        program_id,
        collateral_pool_index,
        operator_sol_info,
        collateral_pool_sol_info,
        system_info)?;

    let (results, used_gas) = {
        if token::check_enough_funds(
            trx_gas_limit,
            trx_gas_price,
            user_eth_info,
            Some(&mut storage)).is_err() {
            let used_gas = storage.get_payments_info()?.0;
            (Some((ExitReason::Error(ExitError::OutOfFund), vec![0; 0], None)), used_gas)
        } else {
            do_continue(&mut storage, step_count, &mut account_storage)?
        }
    };

    token::user_pays_operator_for_iteration(
        trx_gas_price, used_gas,
        user_eth_info,
        operator_eth_info,
        accounts,
        &account_storage,
        &mut storage,
    )?;

    if let Some(evm_results) = results {
        payment::transfer_from_deposit_to_operator(
            storage_info,
            operator_sol_info)?;

        applies_and_invokes(
            program_id,
            &mut account_storage,
            accounts,
            operator_sol_info,
            evm_results,
            used_gas)?;

        storage.unblock_accounts_and_finalize(program_id, trx_accounts)?;
    }

    Ok(())
}

fn do_partial_call(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &ProgramAccountStorage,
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> CallResult
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

    let executor_state = executor.into_state();
    let used_gas = executor_state.gasometer().total_used_gas()/2;
    debug_print!("first iteration complete; steps executed={:?}; used_gas={:?}", step_count, used_gas);

    Ok((None,used_gas))
}

fn do_partial_create<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &ProgramAccountStorage<'a>,
    instruction_data: Vec<u8>,
    transfer_value: U256,
    gas_limit: u64,
) -> CallResult
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

    let executor_state = executor.into_state();
    let used_gas = executor_state.gasometer().total_used_gas()/2;
    debug_print!("first iteration of deployment complete; steps executed={:?}; used_gas={:?}", step_count, used_gas);

    Ok((None,used_gas))
}

#[allow(clippy::unnecessary_wraps)]
fn do_continue<'a>(
    storage: &mut StorageAccount,
    step_count: u64,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> CallResult
{
    debug_print!("do_continue");

    let (evm_results, used_gas) = {
        let mut executor = Machine::restore(storage, account_storage);
        debug_print!("Executor restored");

        let (result, exit_reason) = match executor.execute_n_steps(step_count) {
            Ok(()) => {
                executor.save_into(storage);
                debug_print!("{} steps executed", step_count);
                let executor_state = executor.into_state();
                let used_gas = executor_state.gasometer().total_used_gas()/2;
                return Ok((None, used_gas));
            }
            Err((result, reason)) => (result, reason)
        };

        debug_print!("Call done");

        let executor_state = executor.into_state();
        let used_gas = executor_state.gasometer().used_gas();
        if exit_reason.is_succeed() {
            debug_print!("Succeed execution");
            let apply = executor_state.deconstruct();
            ((exit_reason, result, Some(apply)), used_gas)
        } else {
            ((exit_reason, result, None), used_gas)
        }
    };

    Ok((Some(evm_results),used_gas))
}

fn applies_and_invokes<'a>(
    program_id: &Pubkey,
    account_storage: &mut ProgramAccountStorage<'a>,
    accounts: &'a [AccountInfo<'a>],
    operator: &AccountInfo<'a>,
    evm_results: EvmResults,
    used_gas: UsedGas
) -> ProgramResult {
    let (exit_reason, result, applies_logs_transfers) = evm_results;
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
                ExitError::StaticModeViolation => {("ExitError: STATICCALL tried to change state", 0xee)}
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
    let sender_account = account_storage.get_caller_account();

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
    let contract_account = account_storage.get_contract_account();

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
