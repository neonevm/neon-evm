use actix_request_identifier::RequestId;
use actix_web::{http::StatusCode, post, web::Json, Responder};
use std::convert::Into;

use crate::api_server::handlers::process_error;
use crate::{
    api_context, commands::emulate as EmulateCommand, context::Context,
    types::request_models::EmulateRequestModel, NeonApiState,
};

use super::{parse_emulation_params, process_result};

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[post("/emulate")]
pub async fn emulate(
    state: NeonApiState,
    request_id: RequestId,
    Json(emulate_request): Json<EmulateRequestModel>,
) -> impl Responder {
    let tx = emulate_request.tx_params.into();

    let rpc_client = match api_context::build_rpc_client(
        &state,
        emulate_request.slot,
        emulate_request.tx_index_in_block,
    )
    .await
    {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = Context::new(&*rpc_client, &state.config);

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
