use crate::api_server::handlers::process_error;
use crate::{
    api_context, context::Context, types::request_models::GetStorageAtRequest, NeonApiState,
};
use actix_request_identifier::RequestId;
use actix_web::{get, http::StatusCode, web::Query, Responder};
use std::convert::Into;

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[get("/get-storage-at")]
pub async fn get_storage_at(
    state: NeonApiState,
    request_id: RequestId,
    Query(req_params): Query<GetStorageAtRequest>,
) -> impl Responder {
    let rpc_client = match api_context::build_rpc_client(&state, req_params.slot).await {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = Context::new(&*rpc_client, &state.config);

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
