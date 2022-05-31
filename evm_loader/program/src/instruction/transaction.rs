use evm::{ExitError, ExitReason, H160, U256};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use crate::account;
use crate::account::{EthereumAccount, Operator, program, State, FinalizedState, Treasury};
use crate::account_storage::{ProgramAccountStorage};
use crate::executor::Machine;
use crate::executor_state::{ApplyState};
use crate::state_account::Deposit;
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
        FinalizedState::TAG => {
            if FinalizedState::from_account(program_id, storage_info)?.is_outdated(signature, caller) {
                Ok(true)
            } else {
                return Err!(EvmLoaderError::StorageAccountFinalized.into(); "Transaction already finalized")
            }
        },
        State::TAG => Ok(false),
        _ => return Err!(ProgramError::InvalidAccountData; "Account {} - expected storage or empty", storage_info.key)
    }
}

pub fn do_begin<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
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
        let mut executor = Machine::new(caller, account_storage)?;
        executor.gasometer_mut().record_iterative_overhead();
        executor.gasometer_mut().record_transaction_size(&trx);

        let begin_result = if let Some(code_address) = trx.to {
            executor.call_begin(caller, code_address, trx.call_data, trx.value, trx.gas_limit, trx.gas_price)
        } else {
            executor.create_begin(caller, trx.call_data, trx.value, trx.gas_limit, trx.gas_price)
        };

        match begin_result {
            Ok(()) => {
                execute_steps(executor, step_count, &mut storage)
            }
            Err(ProgramError::InsufficientFunds) => {
                let result = vec![];
                let exit_reason = ExitError::OutOfFund.into();

                (Some((result, exit_reason, None)), executor.used_gas())
            }
            Err(e) => return Err(e)
        }
    };

    finalize(accounts, storage, account_storage, results, used_gas)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> ProgramResult {
    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    let (results, used_gas) = {
        let executor = Machine::restore(&storage, account_storage)?;
        execute_steps(executor, step_count, &mut storage)
    };

    finalize(accounts, storage, account_storage, results, used_gas)
}


type EvmResults = (Vec<u8>, ExitReason, Option<ApplyState>);

fn execute_steps(
    mut executor: Machine<ProgramAccountStorage>,
    step_count: u64,
    storage: &mut State
) -> (Option<EvmResults>, U256) {

    match executor.execute_n_steps(step_count) {
        Ok(_) => { // step limit
            let used_gas = executor.used_gas();
            executor.save_into(storage);

            (None, used_gas)
        },
        Err((result, reason)) => { // transaction complete
            let used_gas = executor.used_gas();

            let apply_state = if reason.is_succeed() {
                Some(executor.into_state().deconstruct())
            } else {
                None
            };

            (Some((result, reason, apply_state)), used_gas)
        }
    }
}

fn pay_gas_cost<'a>(
    used_gas: U256,
    operator_ether_account: EthereumAccount<'a>,
    storage: &mut State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> ProgramResult {
    debug_print!("pay_gas_cost {}", used_gas);

    // Can overflow in malicious transaction
    let value = used_gas.saturating_mul(storage.gas_price);

    account_storage.transfer_gas_payment(
        storage.caller,
        operator_ether_account,
        value,
    )?;

    storage.gas_used_and_paid = storage.gas_used_and_paid.saturating_add(used_gas);
    storage.number_of_payments = storage.number_of_payments.saturating_add(1);

    Ok(())
}

fn finalize<'a>(
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    results: Option<EvmResults>,
    used_gas: U256,
) -> ProgramResult {
    debug_print!("finalize");

    // The only place where checked math is requiered.
    // Saturating math should be used everywhere else for gas calculation
    let total_used_gas = storage.gas_used_and_paid.checked_add(used_gas);

    // Integer overflow or more than gas_limit. Consume remaining gas and revert transaction with Out of Gas
    if total_used_gas.is_none() || (total_used_gas > Some(storage.gas_limit))  {
        let out_of_gas = Some((vec![], ExitError::OutOfGas.into(), None));
        let remaining_gas = storage.gas_limit.saturating_sub(storage.gas_used_and_paid);

        return finalize(accounts, storage, account_storage, out_of_gas, remaining_gas);
    }

    let results = match pay_gas_cost(used_gas, accounts.operator_ether_account, &mut storage, account_storage) {
        Ok(()) => results,
        Err(ProgramError::InsufficientFunds) => Some((vec![], ExitError::OutOfFund.into(), None)),
        Err(e) => return Err(e)
    };

    if let Some((result, exit_reason, apply_state)) = results {
        if let Some(apply_state) = apply_state {
            account_storage.apply_state_change(&accounts.neon_program, &accounts.system_program, &accounts.operator, apply_state)?;
        } else {
            // Transaction ended with error, no state to apply
            // Increment nonce here. Normally it is incremented inside apply_state_change
            if let Some(caller) = account_storage.ethereum_account_mut(&storage.caller) {
                caller.trx_count = caller.trx_count.checked_add(1)
                    .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - nonce overflow", caller.address))?;
            }
        }

        accounts.neon_program.on_return(exit_reason, storage.gas_used_and_paid, &result)?;

        account_storage.block_accounts(false)?;
        storage.finalize(Deposit::ReturnToOperator(accounts.operator))?;
    }

    Ok(())
}