use actix_web::{get, http::StatusCode, web, Responder};

use crate::{context, types::request_models::GetStorageAtRequest, NeonApiState};
use std::convert::Into;

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::{process_error, process_result};

#[get("/get-storage-at")]
pub async fn get_storage_at(
    web::Query(req_params): web::Query<GetStorageAtRequest>,
    state: web::Data<NeonApiState>,
) -> impl Responder {
    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let (rpc_client, blocking_rpc_client) =
        match context::build_rpc_client(&state.config, req_params.slot) {
            Ok(rpc_client) => rpc_client,
            Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
        };

    let context = context::create(rpc_client, signer, blocking_rpc_client);

    process_result(
        &GetStorageAtCommand::execute(
            &state.config,
            &context,
            req_params.contract_id,
            &req_params.index,
        )
        .await
        .map_err(Into::into),
    )
}
