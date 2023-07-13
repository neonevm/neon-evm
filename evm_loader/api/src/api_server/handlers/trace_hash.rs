use std::convert::Into;

use crate::{context, types::request_models::TraceHashRequestModel, NeonApiState};
use axum::{http::StatusCode, Json};

use super::{parse_emulation_params, process_error, process_result};

#[axum::debug_handler]
pub async fn trace_hash(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(trace_hash_request): Json<TraceHashRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let (rpc_client, blocking_rpc_client) = match context::build_hash_rpc_client(
        &state.config,
        &trace_hash_request.emulate_hash_request.hash,
    )
    .await
    {
        Ok((rpc_client, blocking_rpc_client)) => (rpc_client, blocking_rpc_client),
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

    let context = context::create(rpc_client, signer, blocking_rpc_client);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_hash_request.emulate_hash_request.emulation_params,
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
