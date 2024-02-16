use crate::account::{AccountsDB, AllocateResult, StateAccount};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::{EVM_STEPS_LAST_ITERATION_MAX, EVM_STEPS_MIN};
use crate::error::{Error, Result};
use crate::evm::tracing::NoopEventListener;
use crate::evm::{ExitStatus, Machine};
use crate::executor::{Action, ExecutorState};
use crate::gasometer::Gasometer;
use crate::types::{Address, Transaction};

type EvmBackend<'a, 'r> = ExecutorState<'r, ProgramAccountStorage<'a>>;
type Evm<'a, 'r> = Machine<EvmBackend<'a, 'r>, NoopEventListener>;

pub fn do_begin<'a>(
    accounts: AccountsDB<'a>,
    mut storage: StateAccount<'a>,
    gasometer: Gasometer,
    trx: Transaction,
    origin: Address,
) -> Result<()> {
    debug_print!("do_begin");

    let accounts = ProgramAccountStorage::new(accounts)?;

    let mut backend = ExecutorState::new(&accounts);
    let evm = Machine::new(trx, origin, &mut backend, None::<NoopEventListener>)?;

    // Burn `gas_limit` tokens from the origin account
    // Later we will mint them to the operator
    let mut origin_balance = accounts.create_balance_account(origin, storage.trx_chain_id())?;
    origin_balance.burn(storage.gas_limit_in_tokens()?)?;

    serialize_evm_state(&mut storage, &backend, &evm)?;
    finalize(0, storage, accounts, None, gasometer)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: AccountsDB<'a>,
    mut storage: StateAccount<'a>,
    gasometer: Gasometer,
) -> Result<()> {
    debug_print!("do_continue");

    if (step_count < EVM_STEPS_MIN) && (storage.trx_gas_price() > 0) {
        return Err(Error::Custom(format!(
            "Step limit {step_count} below minimum {EVM_STEPS_MIN}"
        )));
    }

    let account_storage = ProgramAccountStorage::new(accounts)?;
    let (mut backend, mut evm) = deserialize_evm_state(&storage, &account_storage)?;

    let (result, steps_executed, _) = {
        match backend.exit_status() {
            Some(status) => (status.clone(), 0_u64, None),
            None => evm.execute(step_count, &mut backend)?,
        }
    };

    if (result != ExitStatus::StepLimit) && (steps_executed > 0) {
        backend.set_exit_status(result.clone());
    }

    if steps_executed > 0 {
        serialize_evm_state(&mut storage, &backend, &evm)?;
    }

    let results = match result {
        ExitStatus::StepLimit => None,
        _ if steps_executed > EVM_STEPS_LAST_ITERATION_MAX => None,
        result => Some((result, backend.into_actions())),
    };

    finalize(steps_executed, storage, account_storage, results, gasometer)
}

fn finalize<'a>(
    steps_executed: u64,
    mut storage: StateAccount<'a>,
    mut accounts: ProgramAccountStorage<'a>,
    results: Option<(ExitStatus, Vec<Action>)>,
    mut gasometer: Gasometer,
) -> Result<()> {
    debug_print!("finalize");

    if steps_executed > 0 {
        accounts.transfer_treasury_payment()?;
    }

    let status = if let Some((status, actions)) = results {
        if accounts.allocate(&actions)? == AllocateResult::Ready {
            accounts.apply_state_change(actions)?;
            Some(status)
        } else {
            None
        }
    } else {
        None
    };

    gasometer.record_operator_expenses(accounts.operator());

    let used_gas = gasometer.used_gas();
    let total_used_gas = gasometer.used_gas_total();
    solana_program::log::sol_log_data(&[
        b"GAS",
        &used_gas.to_le_bytes(),
        &total_used_gas.to_le_bytes(),
    ]);

    storage.consume_gas(used_gas, accounts.operator_balance())?;

    if let Some(status) = status {
        log_return_value(&status);

        let mut origin = accounts.balance_account(storage.trx_origin(), storage.trx_chain_id())?;
        storage.refund_unused_gas(&mut origin)?;

        storage.finalize(accounts.program_id(), accounts.db())?;
    }

    Ok(())
}

pub fn log_return_value(status: &ExitStatus) {
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
        crate::error::print_revert_message(msg);
    }

    sol_log_data(&[b"RETURN", &[code]]);
}

fn serialize_evm_state(
    state: &mut StateAccount,
    backend: &EvmBackend,
    machine: &Evm,
) -> Result<()> {
    let (evm_state_len, evm_machine_len) = {
        let mut buffer = state.buffer_mut();
        let backend_bytes = backend.serialize_into(&mut buffer)?;

        let buffer = &mut buffer[backend_bytes..];
        let evm_bytes = machine.serialize_into(buffer)?;

        (backend_bytes, evm_bytes)
    };

    state.set_buffer_variables(evm_state_len, evm_machine_len);

    Ok(())
}

fn deserialize_evm_state<'a, 'r>(
    state: &StateAccount<'a>,
    account_storage: &'r ProgramAccountStorage<'a>,
) -> Result<(EvmBackend<'a, 'r>, Evm<'a, 'r>)> {
    let (evm_state_len, evm_machine_len) = state.buffer_variables();
    let buffer = state.buffer();

    let executor_state_data = &buffer[..evm_state_len];
    let backend = ExecutorState::deserialize_from(executor_state_data, account_storage)?;

    let evm_data = &buffer[evm_state_len..][..evm_machine_len];
    let evm = Machine::deserialize_from(evm_data, &backend)?;

    Ok((backend, evm))
}
