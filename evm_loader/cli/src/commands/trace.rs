use crate::{
    commands::emulate,
    context::Context,
    event_listener::tracer::Tracer,
    types::{trace::TracedCall, TxParams},
    Config, NeonCliResult,
};
use evm_loader::types::Address;
use solana_sdk::pubkey::Pubkey;

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    context: &Context,
    tx: TxParams,
    token: Pubkey,
    chain: u64,
    steps: u64,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::evm::tracing::using(&mut tracer, || {
        emulate::execute(
            config,
            context,
            tx,
            token,
            chain,
            steps,
            accounts,
            solana_accounts,
        )
    })?;

    let (vm_trace, full_trace_data) = tracer.into_traces();

    let trace = TracedCall {
        vm_trace,
        full_trace_data,
        used_gas: 0,
    };

    Ok(serde_json::json!(trace))
}
