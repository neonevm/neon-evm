use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::{emulate, TxParams},
            types::ec::{trace::{FullTraceData, VMTrace},},
};
use solana_sdk::pubkey::Pubkey;

#[derive(serde::Serialize, Debug)]
pub struct TracedCall {
    pub vm_trace: Option<VMTrace>,
    pub full_trace_data: Vec<FullTraceData>,
    pub used_gas: u64,
}

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
