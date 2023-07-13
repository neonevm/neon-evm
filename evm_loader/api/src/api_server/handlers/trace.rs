use axum::{http::StatusCode, Json};
use std::convert::Into;

use crate::{context, types::request_models::TraceRequestModel, NeonApiState};

use super::{parse_emulation_params, process_error, process_result};

#[axum::debug_handler]
pub async fn trace(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(trace_request): Json<TraceRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let tx = trace_request.emulate_request.tx_params.into();

    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let (rpc_client, blocking_rpc_client) =
        match context::build_rpc_client(&state.config, trace_request.emulate_request.slot) {
            Ok(rpc_client) => rpc_client,
            Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
        };

    let context = context::create(rpc_client, signer, blocking_rpc_client);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_request.emulate_request.emulation_params,
    )
    .await;

    process_result(
        &crate::commands::trace::execute(
            &state.config,
            &context,
            tx,
            token,
            chain,
            steps,
            &accounts,
            &solana_accounts,
        )
        .await
        .map_err(Into::into),
    )
}
