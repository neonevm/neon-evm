use actix_web::{http::StatusCode, routes, web::Json, Responder};
use std::convert::Into;

use crate::{
    commands::emulate as EmulateCommand, context, types::request_models::EmulateHashRequestModel,
    NeonApiState,
};

use super::{parse_emulation_params, process_error, process_result};

#[routes]
#[post("/emulate_hash")] // Obsolete
#[post("/emulate-hash")]
pub async fn emulate_hash(
    state: NeonApiState,
    Json(emulate_hash_request): Json<EmulateHashRequestModel>,
) -> impl Responder {
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

    let context = context::create(&*rpc_client, &state.config);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &emulate_hash_request.emulation_params,
    )
    .await;

    process_result(
        &EmulateCommand::execute(
            context.rpc_client,
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
