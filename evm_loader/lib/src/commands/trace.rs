use std::rc::Rc;

use serde_json::Value;
use solana_sdk::pubkey::Pubkey;

use crate::errors::NeonError;
use crate::rpc::RpcEnum;
use crate::tracing::tracers::new_tracer;
use crate::types::EmulateRequest;

pub async fn trace_transaction(
    rpc: &RpcEnum,
    program_id: Pubkey,
    config: EmulateRequest,
) -> Result<Value, NeonError> {
    let trace_config = config
        .trace_config
        .as_ref()
        .map(|c| c.trace_config.clone())
        .unwrap_or_default();

    let tracer = new_tracer(&trace_config)?;

    let emulation_tracer = Some(Rc::clone(&tracer));
    let r = super::emulate::execute(rpc, program_id, config, emulation_tracer).await?;

    let mut traces = Rc::try_unwrap(tracer)
        .expect("There is must be only one reference")
        .into_inner()
        .into_traces();
    traces["gas"] = r.used_gas.into();

    Ok(traces)
}
