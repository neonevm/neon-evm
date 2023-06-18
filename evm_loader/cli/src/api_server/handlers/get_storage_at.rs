use crate::{context, types::request_models::GetStorageAtRequest, NeonApiState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::{process_error, process_result};

#[allow(clippy::unused_async)]
pub async fn get_storage_at(
    Query(req_params): Query<GetStorageAtRequest>,
    State(state): State<NeonApiState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let signer = match context::build_signer(&state.config) {
        Ok(signer) => signer,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let rpc_client = match context::build_rpc_client(&state.config, req_params.slot) {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = context::create(rpc_client, signer);

    process_result(&GetStorageAtCommand::execute(
        &state.config,
        &context,
        req_params.contract_id,
        &req_params.index,
    ))
}
