use crate::api_server::handlers::process_error;
use crate::{types::GetStorageAtRequest, NeonApiState};
use actix_request_identifier::RequestId;
use actix_web::post;
use actix_web::web::Json;
use actix_web::{http::StatusCode, Responder};
use std::convert::Into;

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[post("/storage")]
pub async fn get_storage_at(
    state: NeonApiState,
    request_id: RequestId,
    Json(req_params): Json<GetStorageAtRequest>,
) -> impl Responder {
    let rpc = match state.build_rpc(req_params.slot, None).await {
        Ok(rpc) => rpc,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &GetStorageAtCommand::execute(
            &rpc,
            &state.config.evm_loader,
            req_params.contract,
            req_params.index,
        )
        .await
        .map_err(Into::into),
    )
}
