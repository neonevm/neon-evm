use crate::commands::get_ether_account_data as GetEtherAccountDataCommand;
use crate::{context, types::request_models::GetEtherRequest, NeonApiState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use std::convert::Into;

use super::{process_error, process_result};

#[axum::debug_handler]
pub async fn get_ether_account_data(
    Query(req_params): Query<GetEtherRequest>,
    State(state): State<NeonApiState>,
) -> (StatusCode, Json<serde_json::Value>) {
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
        &GetEtherAccountDataCommand::execute(&state.config, &context, &req_params.ether)
            .await
            .map_err(Into::into),
    )
}
