use axum::{http::StatusCode, Json};
use std::convert::Into;

use crate::{
    commands::emulate as EmulateCommand, context, types::request_models::EmulateHashRequestModel,
    NeonApiState,
};

use super::{parse_emulation_params, process_error, process_result};

pub async fn emulate_hash(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(emulate_hash_request): Json<EmulateHashRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rpc_client =
        match context::build_hash_rpc_client(&state.config, &emulate_hash_request.hash).await {
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
        &emulate_hash_request.emulation_params,
    )
    .await;

    process_result(
        &EmulateCommand::execute(
            context.rpc_client.as_ref(),
            state.config.evm_loader,
            tx,
            token,
            chain,
            steps,
            state.config.commitment,
            &accounts,
            &solana_accounts,
            &None,
            None,
        )
        .await
        .map_err(Into::into),
    )
}
