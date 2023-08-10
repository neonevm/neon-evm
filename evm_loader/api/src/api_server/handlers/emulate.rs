use actix_web::{http::StatusCode, post, web::Json, Responder};
use std::convert::Into;

use crate::{
    commands::emulate as EmulateCommand, context, types::request_models::EmulateRequestModel,
    NeonApiState,
};

use super::{parse_emulation_params, process_error, process_result};

#[post("/emulate")]
pub async fn emulate(
    state: NeonApiState,
    Json(emulate_request): Json<EmulateRequestModel>,
) -> impl Responder {
    let tx = emulate_request.tx_params.into();

    let rpc_client = match context::build_rpc_client(&state.config, emulate_request.slot) {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = context::create(&*rpc_client, &state.config);

    let (token, chain, steps, accounts, solana_accounts) =
        parse_emulation_params(&state.config, &context, &emulate_request.emulation_params).await;

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
