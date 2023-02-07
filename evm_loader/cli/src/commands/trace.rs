use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::emulate,
            types::{trace::TracedCall, TxParams},
};
use solana_sdk::pubkey::Pubkey;

pub fn execute(config: &Config, tx: TxParams, token: Pubkey, chain: u64, steps: u64) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::evm::tracing::using( &mut tracer, || {
        emulate::execute(config, tx, token, chain, steps)
    })?;

    let (vm_trace, full_trace_data) = tracer.into_traces();

    let trace = TracedCall{
        vm_trace,
        full_trace_data,
        used_gas: 0,
    };

    Ok(serde_json::json!(trace))
}
