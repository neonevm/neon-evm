use log::{debug, info};

use ethnum::U256;
use evm_loader::{
    account_storage::AccountStorage,
    config::{EVM_STEPS_MIN, PAYMENT_TO_TREASURE},
    evm::{ExitStatus, Machine},
    executor::{Action, ExecutorState},
    gasometer::LAMPORTS_PER_SIGNATURE,
    types::{Address, Transaction},
};
use serde::Serialize;

use crate::{
    account_storage::{EmulatorAccountStorage, NeonAccount, SolanaAccount},
    errors::NeonError,
    syscall_stubs::Stubs,
    Config, NeonResult,
};
use crate::{context::Context, types::TxParams};
use solana_sdk::pubkey::Pubkey;

#[derive(Serialize)]
pub struct EmulateReturn {
    pub accounts: Vec<NeonAccount>,
    pub solana_accounts: Vec<SolanaAccount>,
    pub token_accounts: Vec<()>,
    pub result: String,
    pub exit_status: String,
    pub steps_executed: u64,
    pub used_gas: u64,
    pub actions: Vec<Action>,
}

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    context: &Context,
    tx_params: TxParams,
    token: Pubkey,
    chain: u64,
    steps: u64,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
) -> NeonResult<EmulateReturn> {
    let syscall_stubs = Stubs::new(context)?;
    solana_sdk::program_stubs::set_syscall_stubs(syscall_stubs);

    let storage = EmulatorAccountStorage::new(config, context, token, chain);
    storage.initialize_cached_accounts(accounts, solana_accounts);

    let trx = Transaction {
        nonce: storage.nonce(&tx_params.from),
        gas_price: U256::ZERO,
        gas_limit: U256::MAX,
        target: tx_params.to,
        value: tx_params.value.unwrap_or_default(),
        call_data: evm_loader::evm::Buffer::new(&tx_params.data.unwrap_or_default()),
        chain_id: Some(chain.into()),
        ..Transaction::default()
    };

    let (exit_status, actions, steps_executed) = {
        let mut backend = ExecutorState::new(&storage);
        let mut evm = Machine::new(trx, tx_params.from, &mut backend)?;

        let (result, steps_executed) = evm.execute(steps, &mut backend)?;
        let actions = backend.into_actions();
        (result, actions, steps_executed)
    };

    debug!("Execute done, result={exit_status:?}");
    debug!("{steps_executed} steps executed");

    if exit_status == ExitStatus::StepLimit {
        return Err(NeonError::TooManySteps);
    }

    let accounts_operations = storage.calc_accounts_operations(&actions);

    let max_iterations = (steps_executed + (EVM_STEPS_MIN - 1)) / EVM_STEPS_MIN;
    let steps_gas = max_iterations * (LAMPORTS_PER_SIGNATURE + PAYMENT_TO_TREASURE);
    let begin_end_gas = 2 * LAMPORTS_PER_SIGNATURE;
    let actions_gas = storage.apply_actions(&actions);
    let accounts_gas = storage.apply_accounts_operations(accounts_operations);
    info!("Gas - steps: {steps_gas}, actions: {actions_gas}, accounts: {accounts_gas}");

    let (result, status) = match exit_status {
        ExitStatus::Return(v) => (v, "succeed"),
        ExitStatus::Revert(v) => (v, "revert"),
        ExitStatus::Stop | ExitStatus::Suicide => (vec![], "succeed"),
        ExitStatus::StepLimit => unreachable!(),
    };

    let accounts: Vec<NeonAccount> = storage.accounts.borrow().values().cloned().collect();

    let solana_accounts: Vec<SolanaAccount> =
        storage.solana_accounts.borrow().values().cloned().collect();

    let result = EmulateReturn {
        accounts,
        solana_accounts,
        token_accounts: vec![],
        result: hex::encode(result),
        exit_status: status.to_string(),
        steps_executed,
        used_gas: steps_gas + begin_end_gas + actions_gas + accounts_gas,
        actions,
    };

    Ok(result)
}
