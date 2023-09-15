use axum::{http::StatusCode, Json};
use std::convert::Into;

use crate::commands::trace::trace_transaction;
use crate::{context, types::request_models::TraceRequestModel, NeonApiState};

use super::{parse_emulation_params, process_error, process_result};

#[tracing::instrument(skip(state))]
pub async fn trace(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(trace_request): Json<TraceRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let tx = trace_request.emulate_request.tx_params.into();

    let rpc_client =
        match context::build_rpc_client(&state.config, trace_request.emulate_request.slot).await {
            Ok(rpc_client) => rpc_client,
            Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
        };

    let context = context::create(rpc_client, state.config.clone());

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_request.emulate_request.emulation_params,
    )
    .await;

    process_result(
        &trace_transaction(
            context.rpc_client.as_ref(),
            state.config.evm_loader,
            tx,
            token,
            chain,
            steps,
            state.config.commitment,
            &accounts,
            &solana_accounts,
            trace_request.trace_call_config.unwrap_or_default(),
        )
        .await
        .map_err(Into::into),
    )
}
