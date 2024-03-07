use evm_loader::account::ContractAccount;
use evm_loader::account_storage::AccountStorage;
use evm_loader::error::build_revert_message;
use evm_loader::executor::ExecutorStateData;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use solana_sdk::pubkey::Pubkey;

use crate::commands::get_config::BuildConfigSimulator;
use crate::rpc::Rpc;
use crate::tracing::tracers::Tracer;
use crate::types::{EmulateRequest, TxParams};
use crate::{
    account_storage::{EmulatorAccountStorage, SolanaAccount},
    errors::NeonError,
    NeonResult,
};
use evm_loader::{
    config::{EVM_STEPS_MIN, PAYMENT_TO_TREASURE},
    evm::{ExitStatus, Machine},
    executor::{Action, ExecutorState},
    gasometer::LAMPORTS_PER_SIGNATURE,
};
use serde_with::{hex::Hex, serde_as};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateResponse {
    pub exit_status: String,
    #[serde_as(as = "Hex")]
    pub result: Vec<u8>,
    pub steps_executed: u64,
    pub used_gas: u64,
    pub iterations: u64,
    pub solana_accounts: Vec<SolanaAccount>,
}

impl EmulateResponse {
    pub fn revert<E: ToString>(e: E) -> Self {
        let revert_message = build_revert_message(&e.to_string());
        let exit_status = ExitStatus::Revert(revert_message);
        Self {
            exit_status: exit_status.to_string(),
            result: exit_status.into_result().unwrap_or_default(),
            steps_executed: 0,
            used_gas: 0,
            iterations: 0,
            solana_accounts: vec![],
        }
    }
}

pub async fn execute<T: Tracer>(
    rpc: &(impl Rpc + BuildConfigSimulator),
    program_id: Pubkey,
    emulate_request: EmulateRequest,
    tracer: Option<T>,
) -> NeonResult<(EmulateResponse, Option<Value>)> {
    let block_overrides = emulate_request
        .trace_config
        .as_ref()
        .and_then(|t| t.block_overrides.clone());
    let state_overrides = emulate_request
        .trace_config
        .as_ref()
        .and_then(|t| t.state_overrides.clone());

    let mut storage = EmulatorAccountStorage::with_accounts(
        rpc,
        program_id,
        &emulate_request.accounts,
        emulate_request.chains,
        block_overrides,
        state_overrides,
    )
    .await?;

    let step_limit = emulate_request.step_limit.unwrap_or(100000);

    emulate_trx(emulate_request.tx, &mut storage, step_limit, tracer).await
}

async fn emulate_trx<T: Tracer>(
    tx_params: TxParams,
    storage: &mut EmulatorAccountStorage<'_, impl Rpc>,
    step_limit: u64,
    tracer: Option<T>,
) -> NeonResult<(EmulateResponse, Option<Value>)> {
    info!("tx_params: {:?}", tx_params);

    let (origin, tx) = tx_params.into_transaction(storage).await;

    info!("origin: {:?}", origin);
    info!("tx: {:?}", tx);

    let chain_id = tx.chain_id().unwrap_or_else(|| storage.default_chain_id());
    storage.use_balance_account(origin, chain_id, true).await?;

    // Execute and return results to restrict the lifetime of mutable borrow.
    let (actions, exit_status, steps_executed, tracer) = {
        let mut executor_state_data = ExecutorStateData::new(storage);
        let mut backend = ExecutorState::new(storage, &mut executor_state_data);
        let mut evm = match Machine::new(&tx, origin, &mut backend, tracer).await {
            Ok(evm) => evm,
            Err(e) => return Ok((EmulateResponse::revert(e), None)),
        };

        let (exit_status, steps_executed, tracer) = evm.execute(step_limit, &mut backend).await?;
        if exit_status == ExitStatus::StepLimit {
            return Err(NeonError::TooManySteps);
        }

        (backend.into_actions(), exit_status, steps_executed, tracer)
    };

    storage.apply_actions(actions.clone()).await?;
    storage.mark_legacy_accounts().await?;

    debug!("Execute done, result={exit_status:?}");
    debug!("{steps_executed} steps executed");

    let steps_iterations = (steps_executed + (EVM_STEPS_MIN - 1)) / EVM_STEPS_MIN;
    let treasury_gas = steps_iterations * PAYMENT_TO_TREASURE;
    let cancel_gas = LAMPORTS_PER_SIGNATURE;

    let begin_end_iterations = 2;
    let iterations: u64 = steps_iterations + begin_end_iterations + realloc_iterations(&actions);
    let iterations_gas = iterations * LAMPORTS_PER_SIGNATURE;

    let used_gas = storage.gas + iterations_gas + treasury_gas + cancel_gas;

    let solana_accounts = storage.accounts.borrow().values().cloned().collect();

    Ok((
        EmulateResponse {
            exit_status: exit_status.to_string(),
            steps_executed,
            used_gas,
            solana_accounts,
            result: exit_status.into_result().unwrap_or_default(),
            iterations,
        },
        tracer.map(|tracer| tracer.into_traces()),
    ))
}

fn realloc_iterations(actions: &[Action]) -> u64 {
    let mut result = 0;

    for action in actions {
        if let Action::EvmSetCode { code, .. } = action {
            let size = ContractAccount::required_account_size(code);
            let c = size / MAX_PERMITTED_DATA_INCREASE;
            result = std::cmp::max(result, c);
        }
    }

    result as u64
}
