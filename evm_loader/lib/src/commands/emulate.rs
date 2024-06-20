use crate::commands::get_config::BuildConfigSimulator;
use crate::rpc::Rpc;
use crate::tracing::tracers::Tracer;
use crate::types::{EmulateRequest, TxParams};
use crate::{
    account_storage::{EmulatorAccountStorage, SyncedAccountStorage},
    errors::NeonError,
    NeonResult,
};
use evm_loader::account_storage::AccountStorage;
use evm_loader::error::build_revert_message;
use evm_loader::{
    config::{EVM_STEPS_MIN, PAYMENT_TO_TREASURE},
    evm::{ExitStatus, Machine},
    executor::SyncedExecutorState,
    gasometer::LAMPORTS_PER_SIGNATURE,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{hex::Hex, serde_as, DisplayFromStr};
use solana_sdk::{account::Account, pubkey::Pubkey};
use web3::types::Log;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaAccount {
    #[serde_as(as = "DisplayFromStr")]
    pub pubkey: Pubkey,
    pub is_writable: bool,
    pub is_legacy: bool,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateResponse {
    pub exit_status: String,
    pub external_solana_call: bool,
    pub reverts_before_solana_calls: bool,
    pub reverts_after_solana_calls: bool,
    #[serde_as(as = "Hex")]
    pub result: Vec<u8>,
    pub steps_executed: u64,
    pub used_gas: u64,
    pub iterations: u64,
    pub solana_accounts: Vec<SolanaAccount>,
    pub logs: Vec<Log>,
}

impl EmulateResponse {
    pub fn revert<E: ToString>(
        e: &E,
        backend: &SyncedExecutorState<EmulatorAccountStorage<impl Rpc>>,
    ) -> Self {
        let revert_message = build_revert_message(&e.to_string());
        let exit_status = ExitStatus::Revert(revert_message);
        Self {
            exit_status: exit_status.to_string(),
            external_solana_call: false,
            reverts_before_solana_calls: false,
            reverts_after_solana_calls: false,
            result: exit_status.into_result().unwrap_or_default(),
            steps_executed: 0,
            used_gas: 0,
            iterations: 0,
            solana_accounts: vec![],
            logs: backend.backend().logs(),
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

    let solana_overrides = emulate_request.solana_overrides.map(|overrides| {
        overrides
            .iter()
            .map(|(pubkey, account)| (*pubkey, account.as_ref().map(Account::from)))
            .collect()
    });

    let mut storage = EmulatorAccountStorage::with_accounts(
        rpc,
        program_id,
        &emulate_request.accounts,
        emulate_request.chains,
        block_overrides,
        state_overrides,
        solana_overrides,
        emulate_request.tx.chain_id,
    )
    .await?;

    let step_limit = emulate_request.step_limit.unwrap_or(100_000);

    let result = emulate_trx(emulate_request.tx.clone(), &mut storage, step_limit, tracer).await?;

    if storage.is_timestamp_used() {
        let mut storage2 =
            EmulatorAccountStorage::new_from_other(&storage, 5, 3, emulate_request.tx.chain_id)
                .await?;
        if let Ok(result2) = emulate_trx(
            emulate_request.tx,
            &mut storage2,
            step_limit,
            Option::<T>::None,
        )
        .await
        {
            let response = &result.0;
            let response2 = &result2.0;

            let mut combined_solana_accounts = response.solana_accounts.clone();
            response2.solana_accounts.iter().for_each(|v| {
                if let Some(w) = combined_solana_accounts
                    .iter_mut()
                    .find(|x| x.pubkey == v.pubkey)
                {
                    w.is_writable |= v.is_writable;
                    w.is_legacy |= v.is_legacy;
                } else {
                    combined_solana_accounts.push(v.clone());
                }
            });

            let emul_response = EmulateResponse {
                // We get the result from the first response (as it is executed on the current time)
                result: response.result.clone(),
                exit_status: response.exit_status.to_string(),
                external_solana_call: response.external_solana_call,
                reverts_before_solana_calls: response.reverts_before_solana_calls,
                reverts_after_solana_calls: response.reverts_after_solana_calls,

                // ...and consumed resources from the both responses (because the real execution can occur in the future)
                steps_executed: response.steps_executed.max(response2.steps_executed),
                used_gas: response.used_gas.max(response2.used_gas),
                iterations: response.iterations.max(response2.iterations),
                solana_accounts: combined_solana_accounts,
                logs: response.logs.clone(),
            };

            return Ok((emul_response, result.1));
        }
    }

    Ok(result)
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
    storage.increment_nonce(origin, chain_id).await?;

    let mut backend = SyncedExecutorState::new(storage);
    let mut evm = match Machine::new(&tx, origin, &mut backend, tracer).await {
        Ok(evm) => evm,
        Err(e) => {
            error!("EVM creation failed {e:?}");
            return Ok((EmulateResponse::revert(&e, &backend), None));
        }
    };

    let (exit_status, steps_executed, tracer) = evm.execute(step_limit, &mut backend).await?;
    if exit_status == ExitStatus::StepLimit {
        error!("Step_limit={step_limit} exceeded");
        return Ok((
            EmulateResponse::revert(&NeonError::TooManySteps, &backend),
            None,
        ));
    }

    let logs = backend.backend().logs();

    debug!("Execute done, result={exit_status:?}");
    debug!("{steps_executed} steps executed");

    let execute_status = storage.execute_status;

    let steps_iterations = (steps_executed + (EVM_STEPS_MIN - 1)) / EVM_STEPS_MIN;
    let treasury_gas = steps_iterations * PAYMENT_TO_TREASURE;
    let cancel_gas = LAMPORTS_PER_SIGNATURE;

    let begin_end_iterations = 2;
    let iterations: u64 = steps_iterations + begin_end_iterations + storage.realloc_iterations;
    let iterations_gas = iterations * LAMPORTS_PER_SIGNATURE;
    let storage_gas = storage.get_changes_in_rent()?;

    let used_gas = storage_gas + iterations_gas + treasury_gas + cancel_gas;

    let solana_accounts = storage
        .used_accounts()
        .iter()
        .map(|v| SolanaAccount {
            pubkey: v.pubkey,
            is_writable: v.is_writable,
            is_legacy: v.is_legacy,
        })
        .collect::<Vec<_>>();

    Ok((
        EmulateResponse {
            exit_status: exit_status.to_string(),
            external_solana_call: execute_status.external_solana_call,
            reverts_before_solana_calls: execute_status.reverts_before_solana_calls,
            reverts_after_solana_calls: execute_status.reverts_after_solana_calls,
            steps_executed,
            used_gas,
            solana_accounts,
            result: exit_status.into_result().unwrap_or_default(),
            iterations,
            logs,
        },
        tracer.map(|tracer| tracer.into_traces(used_gas)),
    ))
}
