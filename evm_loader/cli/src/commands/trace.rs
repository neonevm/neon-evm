use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::emulate};
use clap::ArgMatches;
use evm_loader::ExitReason;

use crate::{
    types::ec::{trace::{FlatTrace, FullTraceData, VMTrace},},
};

#[derive(serde::Serialize, Debug)]
pub struct TracedCall {
    pub vm_trace: Option<VMTrace>,
    // pub state_diff: Option<StateDiff>,
    pub traces: Vec<FlatTrace>,
    pub full_trace_data: Vec<FullTraceData>,
    pub js_trace: Option<serde_json::Value>,
    pub result: Vec<u8>,
    pub used_gas: u64,
    pub exit_reason: ExitReason,
}


pub fn execute(config: &Config, params: &ArgMatches) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::using( &mut tracer, || {
        emulate::execute(config, params)
    })?;

    let (vm_trace, flat_trace, full_trace_data, result) = tracer.into_traces();

    let trace = TracedCall{
        vm_trace,
        // state_diff: None,  //TODO:
        traces: flat_trace,
        full_trace_data,
        js_trace: None, // TODO:
        result,
        used_gas: 0,
        exit_reason: ExitReason::StepLimitReached,  // TODO add event ?
    };

    println!("{}", serde_json::json!(trace));
    NeonCliResult::Ok(())
}