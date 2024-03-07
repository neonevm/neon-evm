use crate::account::{AccountsDB, AllocateResult, StateAccount};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::{EVM_STEPS_LAST_ITERATION_MAX, EVM_STEPS_MIN};
use crate::debug::log_data;
use crate::error::{Error, Result};
use crate::evm::tracing::NoopEventListener;
use crate::evm::{ExitStatus, Machine};
use crate::executor::{Action, ExecutorState, ExecutorStateData};
use crate::gasometer::Gasometer;
use crate::types::boxx::boxx;
use crate::types::Vector;

pub type EvmBackend<'a, 'r> = ExecutorState<'r, ProgramAccountStorage<'a>>;
pub type Evm<'a, 'r> = Machine<EvmBackend<'a, 'r>, NoopEventListener>;

pub fn do_begin<'a>(
    accounts: AccountsDB<'a>,
    mut storage: StateAccount<'a>,
    gasometer: Gasometer,
) -> Result<()> {
    debug_print!("do_begin");

    let account_storage = ProgramAccountStorage::new(accounts)?;

    let origin = storage.trx_origin();

    storage.trx().validate(origin, &account_storage)?;

    // Increment origin nonce in the first iteration
    // This allows us to run multiple iterative transactions from the same sender in parallel
    // These transactions are guaranteed to start in a correct sequence
    // BUT they finalize in an undefined order
    let mut origin_account = account_storage.origin(origin, storage.trx())?;
    origin_account.increment_nonce()?;

    // Burn `gas_limit` tokens from the origin account
    // Later we will mint them to the operator
    // Remaining tokens are returned to the origin in the last iteration
    let gas_limit_in_tokens = storage.trx().gas_limit_in_tokens()?;
    origin_account.burn(gas_limit_in_tokens)?;

    allocate_or_reinit_state(&account_storage, &mut storage, true)?;

    finalize(0, storage, account_storage, None, gasometer)
}

pub fn do_continue<'a>(
    step_count: u64,
    accounts: AccountsDB<'a>,
    mut storage: StateAccount<'a>,
    gasometer: Gasometer,
    reset: bool,
) -> Result<()> {
    debug_print!("do_continue");

    if (step_count < EVM_STEPS_MIN) && (storage.trx().gas_price() > 0) {
        return Err(Error::Custom(format!(
            "Step limit {step_count} below minimum {EVM_STEPS_MIN}"
        )));
    }

    let account_storage = ProgramAccountStorage::new(accounts)?;
    let (steps_executed, results) = {
        allocate_or_reinit_state(&account_storage, &mut storage, reset)?;

        let mut state_data = storage.read_executor_state();
        let mut evm = storage.read_evm();
        //let (mut state, mut evm): (RefMut<'_, ExecutorStateData>, RefMut<'_, Evm>) = (storage.state(), storage.evm());
        let mut backend = ExecutorState::new(&account_storage, &mut state_data);

        let (result, steps_executed, _) = match backend.exit_status() {
            Some(status) => (status.clone(), 0_u64, None::<NoopEventListener>),
            None => evm.execute(step_count, &mut backend)?,
        };

        if (result != ExitStatus::StepLimit) && (steps_executed > 0) {
            backend.set_exit_status(result.clone());
        }

        let results = match result {
            ExitStatus::StepLimit => None,
            _ if steps_executed > EVM_STEPS_LAST_ITERATION_MAX => None,
            result => Some((result, backend.into_actions())),
        };
        (steps_executed, results)
    };

    finalize(steps_executed, storage, account_storage, results, gasometer)
}

fn allocate_or_reinit_state(
    account_storage: &ProgramAccountStorage<'_>,
    storage: &mut StateAccount<'_>,
    allocate: bool,
) -> Result<()> {
    if allocate {
        let mut state_data = boxx(ExecutorStateData::new(account_storage));
        let mut evm_backend = ExecutorState::new(account_storage, &mut state_data);
        let evm = boxx(Evm::new(
            storage.trx(),
            storage.trx_origin(),
            &mut evm_backend,
            None,
        )?);
        //storage.alloc(state_data, evm)?;
        storage.alloc_evm(evm)?;
        storage.alloc_executor_state(state_data)?;
    } else {
        let mut state_data = storage.read_executor_state();
        let mut evm = storage.read_evm();

        let evm_backend = ExecutorState::new(account_storage, &mut state_data);
        evm.reinit(&evm_backend);
    };
    Ok(())
}

fn finalize<'a>(
    steps_executed: u64,
    mut storage: StateAccount<'a>,
    mut accounts: ProgramAccountStorage<'a>,
    results: Option<(ExitStatus, Vector<Action>)>,
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
    log_data(&[
        b"GAS",
        &used_gas.to_le_bytes(),
        &total_used_gas.to_le_bytes(),
    ]);

    storage.consume_gas(used_gas, accounts.operator_balance())?;

    if let Some(status) = status {
        log_return_value(&status);

        let mut origin = accounts.origin(storage.trx_origin(), storage.trx())?;
        storage.refund_unused_gas(&mut origin)?;

        storage.finalize(accounts.program_id())?;
    }

    Ok(())
}

pub fn log_return_value(status: &ExitStatus) {
    let code: u8 = match status {
        ExitStatus::Stop => 0x11,
        ExitStatus::Return(_) => 0x12,
        ExitStatus::Suicide => 0x13,
        ExitStatus::Revert(_) => 0xd0,
        ExitStatus::StepLimit => unreachable!(),
    };

    log_msg!("exit_status={:#04X}", code); // Tests compatibility
    if let ExitStatus::Revert(msg) = status {
        crate::error::print_revert_message(msg);
    }

    log_data(&[b"RETURN", &[code]]);
}
