use serde_json::Value;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::rc::Rc;

use evm_loader::evm::tracing::tracers::new_tracer;
use evm_loader::evm::tracing::TraceCallConfig;
use evm_loader::types::Address;

use crate::{commands::emulate::emulate_transaction, errors::NeonError, rpc::Rpc, types::TxParams};

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
        Some(Rc::clone(&tracer)),
    )
    .await?;

    Ok(Rc::try_unwrap(tracer)
        .expect("There is must be only one reference")
        .into_inner()
        .into_traces(emulation_result))
}
