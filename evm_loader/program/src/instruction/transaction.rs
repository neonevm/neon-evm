use ethnum::U256;
use solana_program::account_info::AccountInfo;

use crate::account::{EthereumAccount, Operator, program, State, Treasury};
use crate::account_storage::{AccountsReadiness, ProgramAccountStorage};
use crate::config::{EVM_STEPS_MIN, EVM_STEPS_LAST_ITERATION_MAX, PAYMENT_TO_TREASURE};
use crate::evm::{Machine, ExitStatus};
use crate::executor::{Action, ExecutorState};
use crate::state_account::Deposit;
use crate::types::{Address, Transaction};
use crate::gasometer::Gasometer;
use crate::error::{Result, Error, format_revert_message};


pub struct Accounts<'a> {
    pub operator: Operator<'a>,
    pub treasury: Treasury<'a>,
    pub operator_ether_account: EthereumAccount<'a>,
    pub system_program: program::System<'a>,
    pub neon_program: program::Neon<'a>,
    pub remaining_accounts: &'a [AccountInfo<'a>],
    pub all_accounts: &'a [AccountInfo<'a>],
}


pub fn do_begin<'a>(
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    gasometer: Gasometer,
    trx: Transaction,
    caller: Address,
) -> Result<()> {
    debug_print!("do_begin");

    account_storage.check_for_blocked_accounts()?;
    account_storage.block_accounts(true);

    let mut backend = ExecutorState::new(account_storage);
    let evm = Machine::new(trx, caller, &mut backend)?;

    { // Save EVM State into storage
        let mut buffer: &mut [u8] = &mut storage.evm_state_mut_data();
        bincode::serialize_into(&mut buffer, &backend)?;
        bincode::serialize_into(&mut buffer, &evm)?;
    }

    finalize(0, accounts, storage, account_storage, None, gasometer)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    gasometer: Gasometer,
) -> Result<()> {
    debug_print!("do_continue");

    if (step_count < EVM_STEPS_MIN) && (storage.gas_price > 0) {
        return Err(Error::Custom(format!("Step limit {step_count} below minimum {EVM_STEPS_MIN}")));
    }

    let (mut backend, mut evm) = {
        let mut buffer: &[u8] = &mut storage.evm_state_data();
        let backend: ExecutorState<_> = ExecutorState::deserialize_from(&mut buffer, account_storage)?;
        let evm: Machine<_> = bincode::deserialize_from(&mut buffer)?;
        (backend, evm)
    };

    let (result, steps_executed) = {
        match backend.exit_status() {
            Some(status) => (status.clone(), 0_u64),
            None => evm.execute(step_count, &mut backend)?,
        }
    };

    if (result != ExitStatus::StepLimit) && (steps_executed > 0) {
        backend.set_exit_status(result.clone());
    }

    if steps_executed > 0 {
        let mut buffer: &mut [u8] = &mut storage.evm_state_mut_data();
        bincode::serialize_into(&mut buffer, &backend)?;
        bincode::serialize_into(&mut buffer, &evm)?;
    }

    let results = match result {
        ExitStatus::StepLimit => None,
        _ if steps_executed > EVM_STEPS_LAST_ITERATION_MAX => None,
        result => Some((result, backend.into_actions())),
    };

    finalize(steps_executed, accounts, storage, account_storage, results, gasometer)
}

fn pay_gas_cost<'a>(
    used_gas: U256,
    operator_ether_account: EthereumAccount<'a>,
    storage: &mut State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
) -> Result<()> {
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
    steps_executed: u64,
    accounts: Accounts<'a>,
    mut storage: State<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    results: Option<(ExitStatus, Vec<Action>)>,
    mut gasometer: Gasometer,
) -> Result<()> {
    debug_print!("finalize");

    if steps_executed > 0 {
        accounts.system_program.transfer(&accounts.operator, &accounts.treasury, PAYMENT_TO_TREASURE)?;
    }

    let exit_reason_opt = if let Some((exit_reason, apply_state)) = results {
        if account_storage.apply_state_change(
            &accounts.neon_program,
            &accounts.system_program,
            &accounts.operator,
            apply_state,
        )? == AccountsReadiness::Ready {
            Some(exit_reason)
        } else {
            None
        }
    } else {
        None
    };

    gasometer.record_operator_expenses(&accounts.operator);

    let total_used_gas = gasometer.used_gas_total();
    let gas_limit = storage.gas_limit;
    if total_used_gas > gas_limit {
        return Err(Error::OutOfGas(gas_limit, total_used_gas));
    }

    let used_gas = gasometer.used_gas();
    solana_program::log::sol_log_data(&[b"IX_GAS", &used_gas.as_u64().to_le_bytes()]);

    pay_gas_cost(used_gas, accounts.operator_ether_account, &mut storage, account_storage)?;


    if let Some(exit_reason) = exit_reason_opt {
        log_return_value(&exit_reason, total_used_gas);

        account_storage.block_accounts(false);
        storage.finalize(Deposit::ReturnToOperator(accounts.operator))?;
    }

    Ok(())
}


pub fn log_return_value(status: &ExitStatus, gas_used: U256) {
    use solana_program::log::sol_log_data;

    let code: u8 = match status {
        ExitStatus::Stop => 0x11,
        ExitStatus::Return(_) => 0x12,
        ExitStatus::Suicide => 0x13,
        ExitStatus::Revert(_) => 0xd0,
        ExitStatus::StepLimit => unreachable!(),
    };

    solana_program::msg!("exit_status={:#04X}", code); // Tests compatibility
    if let ExitStatus::Revert(msg) = status {
        solana_program::msg!("Revert: {}", format_revert_message(msg));
    }

    sol_log_data(&[
        b"RETURN",
        &[code],
        &gas_used.as_u64().to_le_bytes(),
    ]);
}
