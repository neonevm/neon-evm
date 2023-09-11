use std::convert::Into;

use crate::{context, types::request_models::TraceHashRequestModel, NeonApiState};
use axum::{http::StatusCode, Json};
use neon_lib::commands::trace::trace_transaction;

use super::{parse_emulation_params, process_error, process_result};

#[tracing::instrument(skip(state))]
pub async fn trace_hash(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(trace_hash_request): Json<TraceHashRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rpc_client = match context::build_hash_rpc_client(
        &state.config,
        &trace_hash_request.emulate_hash_request.hash,
    )
    .await
    {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let tx = match rpc_client.get_transaction_data().await {
        Ok(tx) => tx,
        Err(e) => {
            return process_error(
                StatusCode::BAD_REQUEST,
                &crate::errors::NeonError::SolanaClientError(e),
            )
        }
    };

    let context = context::create(rpc_client, state.config.clone());

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_hash_request.emulate_hash_request.emulation_params,
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
            trace_hash_request.trace_config.unwrap_or_default().into(),
        )
        .await
        .map_err(Into::into),
    )
}
