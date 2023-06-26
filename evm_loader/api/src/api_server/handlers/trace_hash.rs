use crate::NeonApiState;
use axum::{http::StatusCode, Json};
use std::convert::Into;

use crate::{context, types::request_models::TraceHashRequestModel};

use super::{parse_emulation_params, process_error, process_result};

#[allow(clippy::unused_async)]
pub async fn trace_hash(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(trace_hash_request): Json<TraceHashRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let rpc_client = match context::build_hash_rpc_client(
        &state.config,
        &trace_hash_request.emulate_hash_request.hash,
    ) {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let tx = match rpc_client.get_transaction_data() {
        Ok(tx) => tx,
        Err(e) => {
            return process_error(
                StatusCode::BAD_REQUEST,
                &crate::errors::NeonError::SolanaClientError(e),
            )
        }
    };

    let context = context::create(rpc_client, signer);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_hash_request.emulate_hash_request.emulation_params,
    );

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
        .map_err(Into::into),
    )
}
