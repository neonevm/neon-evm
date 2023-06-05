use tide::{Request, Result};

use crate::{api_server::state::State, context, types::request_models::TraceHashRequestModel};

use super::{parse_emulation_params, process_result};

#[allow(clippy::unused_async)]
pub async fn trace_hash(mut req: Request<State>) -> Result<serde_json::Value> {
    let trace_hash_request: TraceHashRequestModel = req.body_json().await.map_err(|e| {
        tide::Error::from_str(
            400,
            format!(
                "Error on parsing transaction parameters request: {:?}",
                e.to_string()
            ),
        )
    })?;

    let state = req.state();

    let signer = context::build_singer(&state.config).map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating singer: {:?}", e.to_string()),
        )
    })?;

    let rpc_client = context::build_hash_rpc_client(
        &state.config,
        &trace_hash_request.emulate_hash_request.hash,
    )
    .map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating hash rpc client: {:?}", e.to_string()),
        )
    })?;

    let tx = rpc_client.get_transaction_data()?;

    let context = context::create(rpc_client, signer);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_hash_request.emulate_hash_request.emulation_params,
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
