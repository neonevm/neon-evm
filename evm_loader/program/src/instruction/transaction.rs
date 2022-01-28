use evm::{ExitError, ExitReason, H160, U256};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use crate::account;
use crate::account::{EthereumAccount, Operator, program, Storage, FinalizedStorage, Treasury};
use crate::account_storage::{ProgramAccountStorage};
use crate::executor::Machine;
use crate::executor_state::{ApplyState, ExecutorState, ExecutorSubstate};
use crate::storage_account::Deposit;
use crate::transaction::{check_ethereum_transaction, UnsignedTransaction};
use crate::error::EvmLoaderError;


pub struct Accounts<'a> {
    pub operator: Operator<'a>,
    pub treasury: Treasury<'a>,
    pub operator_ether_account: EthereumAccount<'a>,
    pub system_program: program::System<'a>,
    pub neon_program: program::Neon<'a>,
    pub remaining_accounts: &'a [AccountInfo<'a>],
}

pub fn is_new_transaction<'a>(
    program_id: &'a Pubkey,
    storage_info: &'a AccountInfo<'a>,
    signature: &[u8; 65],
    caller: &H160,
) -> Result<bool, ProgramError> {
    match account::tag(program_id, storage_info)? {
        account::TAG_EMPTY => Ok(true),
        FinalizedStorage::TAG => {
            if FinalizedStorage::from_account(program_id, storage_info)?.is_outdated(signature, caller) {
                Ok(true)
            } else {
                return Err!(EvmLoaderError::StorageAccountFinalized.into(); "Transaction already finalized")
            }
        },
        Storage::TAG => Ok(false),
        _ => return Err!(ProgramError::InvalidAccountData; "Account {} - expected storage or empty", storage_info.key)
    }
}

pub fn do_begin<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: Storage<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    trx: UnsignedTransaction,
    caller: H160,
) -> ProgramResult {
    debug_print!("do_begin");
    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    check_ethereum_transaction(account_storage, &caller, &trx)?;
    account_storage.check_for_blocked_accounts(false)?;
    account_storage.block_accounts(true)?;


    let (results, used_gas) = {
        let executor_substate = Box::new(ExecutorSubstate::new(trx.gas_limit.as_u64(), account_storage));
        let executor_state = ExecutorState::new(executor_substate, account_storage);
        let mut executor = Machine::new(caller, executor_state);

        if let Some(code_address) = trx.to {
            executor.call_begin(caller, code_address, trx.call_data, trx.value, trx.gas_limit.as_u64())?;
        } else {
            executor.create_begin(caller, trx.call_data, trx.value, trx.gas_limit.as_u64())?;
        }

        execute_steps(executor, step_count, &mut storage)
    };

    finalize(accounts, storage, account_storage, results, used_gas)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: Storage<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> ProgramResult {
    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    let (results, used_gas) = {
        let executor = Machine::restore(&storage, account_storage);
        execute_steps(executor, step_count, &mut storage)
    };

    finalize(accounts, storage, account_storage, results, used_gas)
}


type EvmResults = (Vec<u8>, ExitReason, Option<ApplyState>);

fn execute_steps(
    mut executor: Machine<ProgramAccountStorage>,
    step_count: u64,
    storage: &mut Storage
) -> (Option<EvmResults>, U256) {

    match executor.execute_n_steps(step_count) {
        Ok(_) => { // step limit
            let used_gas = executor.gasometer().total_used_gas() / 2;
            executor.save_into(storage);

            (None, U256::from(used_gas))
        },
        Err((result, reason)) => { // transaction complete
            let used_gas = executor.gasometer().used_gas();

            let apply_state = if reason.is_succeed() {
                Some(executor.into_state().deconstruct())
            } else {
                None
            };

            (Some((result, reason, apply_state)), U256::from(used_gas))
        }
    }
}

fn pay_gas_cost<'a>(
    used_gas: U256,
    operator_ether_account: EthereumAccount<'a>,
    storage: &mut Storage<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> ProgramResult {
    debug_print!("pay_gas_cost {}", used_gas);

    let gas_for_iteration = used_gas.saturating_sub(storage.gas_used_and_paid);
    account_storage.transfer_gas_payment(
        storage.caller,
        operator_ether_account,
        gas_for_iteration,
        storage.gas_price,
    )?;

    storage.gas_used_and_paid += gas_for_iteration;
    storage.number_of_payments += 1;

    Ok(())
}

fn finalize<'a>(
    accounts: Accounts<'a>,
    mut storage: Storage<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    results: Option<EvmResults>,
    used_gas: U256,
) -> ProgramResult {
    debug_print!("finalize");

    let results = match pay_gas_cost(used_gas, accounts.operator_ether_account, &mut storage, account_storage) {
        Ok(()) => results,
        Err(ProgramError::InsufficientFunds) => Some((vec![], ExitError::OutOfFund.into(), None)),
        Err(e) => return Err(e)
    };

    if let Some((result, exit_reason, apply_state)) = results {
        if let Some(apply_state) = apply_state {
            account_storage.apply_state_change(&accounts.neon_program, &accounts.system_program, &accounts.operator, apply_state)?;
        }

        accounts.neon_program.on_return(exit_reason, used_gas, &result)?;

        account_storage.block_accounts(false)?;
        storage.finalize(Deposit::ReturnToOperator(accounts.operator))?;
    }

    Ok(())
}