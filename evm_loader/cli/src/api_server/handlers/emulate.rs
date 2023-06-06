use axum::{http::StatusCode, Json};

use crate::{
    commands::emulate as EmulateCommand, context, types::request_models::EmulateRequestModel,
    NeonApiState,
};

use super::{parse_emulation_params, process_error, process_result};

#[allow(clippy::unused_async)]
pub async fn emulate(
    axum::extract::State(state): axum::extract::State<NeonApiState>,
    Json(emulate_request): Json<EmulateRequestModel>,
) -> (StatusCode, Json<serde_json::Value>) {
    let tx = emulate_request.tx_params.into();

    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let rpc_client = match context::build_rpc_client(&state.config, emulate_request.slot) {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = context::create(rpc_client, signer);

    let (token, chain, steps, accounts, solana_accounts) =
        parse_emulation_params(&state.config, &context, &emulate_request.emulation_params);

    process_result(&EmulateCommand::execute(
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
