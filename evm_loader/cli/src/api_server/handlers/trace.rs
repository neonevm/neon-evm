use tide::{Request, Result};

use crate::{api_server::state::State, context, types::request_models::TraceRequestModel};

use super::{parse_emulation_params, process_result};

#[allow(clippy::unused_async)]
pub async fn trace(mut req: Request<State>) -> Result<serde_json::Value> {
    let trace_request: TraceRequestModel = req.body_json().await.map_err(|e| {
        tide::Error::from_str(
            400,
            format!(
                "Error on parsing transaction parameters request: {:?}",
                e.to_string()
            ),
        )
    })?;

    let state = req.state();

    let tx = trace_request.emulate_request.tx_params.into();

    let signer = context::build_singer(&state.config).map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating singer: {:?}", e.to_string()),
        )
    })?;

    let rpc_client = context::build_rpc_client(&state.config, trace_request.emulate_request.slot)
        .map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating rpc client: {:?}", e.to_string()),
        )
    })?;

    let context = context::create(rpc_client, signer);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_request.emulate_request.emulation_params,
    );

    process_result(&crate::commands::trace::execute(
        &state.config,
        &context,
        tx,
        token,
        chain,
        steps,
        &accounts,
        &solana_accounts,
    ))
}
