use std::rc::Rc;

use serde_json::Value;
use solana_sdk::pubkey::Pubkey;

use crate::tracing::tracers::new_tracer;
use crate::types::EmulateRequest;
use crate::{errors::NeonError, rpc::Rpc};

pub async fn trace_transaction(
    rpc_client: &dyn Rpc,
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
    let r = super::emulate::execute(rpc_client, program_id, config, emulation_tracer).await?;

    let mut traces = Rc::try_unwrap(tracer)
        .expect("There is must be only one reference")
        .into_inner()
        .into_traces();
    traces["gas"] = r.used_gas.into();

    Ok(traces)
}
