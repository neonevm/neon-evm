use std::fmt::{Display, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};

use evm_loader::evm::tracing::tracers::new_tracer;
use evm_loader::evm::tracing::{TraceCallConfig, TraceConfig};
use evm_loader::types::Address;

use crate::{
    account_storage::EmulatorAccountStorage,
    commands::emulate::{emulate_transaction, emulate_trx, setup_syscall_stubs},
    errors::NeonError,
    rpc::Rpc,
    types::TxParams,
};

#[allow(clippy::too_many_arguments)]
pub async fn trace_transaction(
    rpc_client: &dyn Rpc,
    evm_loader: Pubkey,
    tx: TxParams,
    token: Pubkey,
    chain_id: u64,
    steps: u64,
    commitment: CommitmentConfig,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
    trace_call_config: TraceCallConfig,
) -> Result<Value, NeonError> {
    let tracer = new_tracer(&trace_call_config.trace_config)?;

    let (emulation_result, _storage) = emulate_transaction(
        rpc_client,
        evm_loader,
        tx,
        token,
        chain_id,
        steps,
        commitment,
        accounts,
        solana_accounts,
        &trace_call_config.block_overrides,
        trace_call_config.state_overrides,
        Some(Arc::clone(&tracer)),
    )
    .await?;

    Ok(Arc::try_unwrap(tracer)
        .expect("There is must be only one reference")
        .into_inner()
        .expect("Poisoned RwLock")
        .into_traces(emulation_result))
}

#[derive(Serialize, Deserialize)]
pub struct TraceBlockReturn(pub Vec<Value>);

impl Display for TraceBlockReturn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ traced call(s): {} }}", self.0.len())
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn trace_block(
    rpc_client: &dyn Rpc,
    evm_loader: Pubkey,
    transactions: Vec<TxParams>,
    token: Pubkey,
    chain_id: u64,
    steps: u64,
    commitment: CommitmentConfig,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
    trace_config: &TraceConfig,
) -> Result<TraceBlockReturn, NeonError> {
    setup_syscall_stubs(rpc_client).await?;

    let storage = EmulatorAccountStorage::with_accounts(
        rpc_client,
        evm_loader,
        token,
        chain_id,
        commitment,
        accounts,
        solana_accounts,
        &None,
        None,
    )
    .await?;

    let mut results = vec![];
    for tx_params in transactions {
        let result = trace_trx(tx_params, &storage, chain_id, steps, trace_config).await?;
        results.push(result);
    }

    Ok(TraceBlockReturn(results))
}

async fn trace_trx<'a>(
    tx_params: TxParams,
    storage: &'a EmulatorAccountStorage<'a>,
    chain_id: u64,
    steps: u64,
    trace_config: &TraceConfig,
) -> Result<Value, NeonError> {
    let tracer = new_tracer(trace_config)?;

    let emulation_result = emulate_trx(
        tx_params,
        storage,
        chain_id,
        steps,
        Some(Arc::clone(&tracer)),
    )
    .await?;

    Ok(Arc::try_unwrap(tracer)
        .expect("There is must be only one reference")
        .into_inner()
        .expect("Poisoned RwLock")
        .into_traces(emulation_result))
}
