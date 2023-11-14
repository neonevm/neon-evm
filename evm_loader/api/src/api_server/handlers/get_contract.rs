use crate::api_server::handlers::process_error;
use crate::commands::get_contract as GetContractCommand;
use crate::{api_context, types::GetContractRequest, NeonApiState};
use actix_request_identifier::RequestId;
use actix_web::post;
use actix_web::web::Json;
use actix_web::{http::StatusCode, Responder};
use std::convert::Into;

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[post("/contract")]
pub async fn get_contract(
    state: NeonApiState,
    request_id: RequestId,
    Json(req_params): Json<GetContractRequest>,
) -> impl Responder {
    let rpc_client = match api_context::build_rpc_client(&state, req_params.slot, None).await {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &GetContractCommand::execute(
            rpc_client.as_ref(),
            &state.config.evm_loader,
            &req_params.contract,
        )
        .await
        .map_err(Into::into),
    )
}
