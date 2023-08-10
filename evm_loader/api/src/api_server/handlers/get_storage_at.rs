use crate::{context, types::request_models::GetStorageAtRequest, NeonApiState};
use actix_web::{get, http::StatusCode, web::Query, Responder};
use std::convert::Into;

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::{process_error, process_result};

#[get("/get-storage-at")]
pub async fn get_storage_at(
    Query(req_params): Query<GetStorageAtRequest>,
    state: NeonApiState,
) -> impl Responder {
    let rpc_client = match context::build_rpc_client(&state.config, req_params.slot) {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = context::create(&*rpc_client, &state.config);

    process_result(
        &GetStorageAtCommand::execute(
            context.rpc_client,
            &state.config.evm_loader,
            req_params.contract_id,
            &req_params.index,
        )
        .await
        .map_err(Into::into),
    )
}
