use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::{emulate, TxParams},
            types::ec::{trace::{FullTraceData, VMTrace},},
};
use evm_loader::ExitReason;

#[derive(serde::Serialize, Debug)]
pub struct TracedCall {
    pub vm_trace: Option<VMTrace>,
    pub full_trace_data: Vec<FullTraceData>,
    pub used_gas: u64,
    pub exit_reason: ExitReason,
}

pub fn execute(config: &Config, tx: &TxParams) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::using( &mut tracer, || {
        emulate::send(config, tx)
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

