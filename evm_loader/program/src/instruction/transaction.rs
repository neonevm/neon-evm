use evm::{ExitError, ExitReason, H160, U256};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use crate::account::{EthereumAccount, Operator, program, State, Treasury};
use crate::account_storage::{AccountsReadiness, AccountStorage, ProgramAccountStorage};
use crate::executor::{Action, Gasometer, Machine};
use crate::state_account::Deposit;
use crate::transaction::{check_ethereum_transaction, Transaction};
use crate::executor::LAMPORTS_PER_SIGNATURE;

/// Current cap of transaction accounts
const TX_ACCOUNT_CNT: u64 = 30;

pub struct Accounts<'a> {
    pub operator: Operator<'a>,
    pub treasury: Treasury<'a>,
    pub operator_ether_account: EthereumAccount<'a>,
    pub system_program: program::System<'a>,
    pub neon_program: program::Neon<'a>,
    pub remaining_accounts: &'a [AccountInfo<'a>],
}


pub fn do_begin<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    trx: Transaction,
    caller: H160,
    alt_cost: u64,
) -> ProgramResult {
    debug_print!("do_begin");

    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    check_ethereum_transaction(account_storage, &caller, &trx)?;
    account_storage.check_for_blocked_accounts()?;
    account_storage.block_accounts(true)?;


    let (results, gasometer) = {
        let mut executor = Machine::new(caller, account_storage)?;
        executor.gasometer_mut().record_iterative_overhead();
        executor.gasometer_mut().record_transaction_size(&trx);
        executor.gasometer_mut().record_alt_cost(alt_cost);

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

                (Some((result, exit_reason, None)), executor.take_gasometer())
            }
            Err(e) => return Err(e)
        }
    };

    finalize(accounts, storage, account_storage, results, gasometer.used_gas(), gasometer)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> ProgramResult {
    debug_print!("do_continue");

    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    let (results, gasometer) = {
        let executor = Machine::restore(&storage, account_storage)?;
        execute_steps(executor, step_count, &mut storage)
    };

    finalize(accounts, storage, account_storage, results, gasometer.used_gas(), gasometer)
}


type EvmResults = (Vec<u8>, ExitReason, Option<Vec<Action>>);

fn execute_steps(
    mut executor: Machine<ProgramAccountStorage>,
    step_count: u64,
    storage: &mut State
) -> (Option<EvmResults>, Gasometer) {
    match executor.execute_n_steps(step_count) {
        Ok(_) => { // step limit
            executor.save_into(storage);

            (None, executor.take_gasometer())
        },
        Err((result, reason)) => { // transaction complete
            let (apply_state, gasometer) = if reason.is_succeed() {
                // TODO: Save only when there is needed to repeat transaction.
                executor.save_into(storage);

                let (actions, gasometer) = executor.into_state_actions_and_gasometer();

                (Some(actions), gasometer)
            } else {
                (None, executor.take_gasometer())
            };

            (Some((result, reason, apply_state)), gasometer)
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
    storage.gas_used = storage.gas_used.saturating_add(used_gas);

    account_storage.transfer_gas_payment(
        storage.caller,
        operator_ether_account,
        value,
    )?;

    Ok(())
}

fn finalize<'a>(
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    results: Option<EvmResults>,
    mut used_gas: U256,
    mut gasometer: Gasometer,
) -> ProgramResult {
    debug_print!("finalize");

    let accounts_operations = match results {
        None => vec![],
        Some((_, _, ref actions)) => {
            let accounts_operations = account_storage.calc_accounts_operations(actions);
            gasometer.record_accounts_operations(&accounts_operations);
            used_gas = gasometer.used_gas();

            accounts_operations
        },
    };

    // The only place where checked math is required.
    // Saturating math should be used everywhere else for gas calculation.
    let total_used_gas = storage.gas_used.checked_add(used_gas);

    // Integer overflow or more than gas_limit. Consume remaining gas and revert transaction with Out of Gas
    if total_used_gas.is_none() || (total_used_gas > Some(storage.gas_limit)) {
        let out_of_gas = Some((vec![], ExitError::OutOfGas.into(), None));
        let remaining_gas = storage.gas_limit.saturating_sub(storage.gas_used);

        return finalize(accounts, storage, account_storage, out_of_gas, remaining_gas, gasometer);
    }

    let (results, accounts_operations) = match pay_gas_cost(used_gas, accounts.operator_ether_account, &mut storage, account_storage) {
        Ok(()) => (results, accounts_operations),
        Err(ProgramError::InsufficientFunds) => (Some((vec![], ExitError::OutOfFund.into(), None)), vec![]),
        Err(e) => return Err(e)
    };
    solana_program::log::sol_log_data(&[b"IX_GAS", used_gas.as_u64().to_le_bytes().as_slice()]);

    if let Some((result, exit_reason, apply_state)) = results {
        let apply_state = apply_state.unwrap_or_else(
            || vec![Action::EvmIncrementNonce { address: storage.caller }],
        );
        if account_storage.apply_state_change(
            &accounts.neon_program,
            &accounts.system_program,
            &accounts.operator,
            apply_state,
            accounts_operations,
        )? == AccountsReadiness::Ready {
            accounts.neon_program.on_return(exit_reason, storage.gas_used, &result);

            account_storage.block_accounts(false)?;
            storage.finalize(Deposit::ReturnToOperator(accounts.operator))?;
        }
    }

    Ok(())
}

#[must_use]
pub fn alt_cost(tx_acc_count: u64) -> u64 {
    if tx_acc_count > TX_ACCOUNT_CNT {
        let extend = tx_acc_count /TX_ACCOUNT_CNT+1;
        // create_alt + extend_alt + deactivate_alt + close_alt
        (extend+3) * LAMPORTS_PER_SIGNATURE
    }
    else{
        0
    }
}
