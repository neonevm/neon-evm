use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::{emulate, TxParams},
            types::{trace::TracedCall,},
};
use solana_sdk::pubkey::Pubkey;
use evm_loader::ExitReason;


pub fn execute(config: &Config, tx: &TxParams, token: Pubkey, chain: u64, steps: u64) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::using( &mut tracer, || {
        emulate::send(config, tx, token, chain, steps)
    })?;

    let (vm_trace, full_trace_data) = tracer.into_traces();

    let trace = TracedCall{
        vm_trace,
        full_trace_data,
        used_gas: 0,
        exit_reason: ExitReason::StepLimitReached,  // TODO add event ?
    };

    println!("{}", serde_json::json!(trace));
    NeonCliResult::Ok(())
}
