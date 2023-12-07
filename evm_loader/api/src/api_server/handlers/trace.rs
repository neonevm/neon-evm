use actix_request_identifier::RequestId;
use actix_web::{http::StatusCode, post, web::Json, Responder};
use std::convert::Into;

use crate::api_server::handlers::process_error;
use crate::commands::trace::trace_transaction;
use crate::{types::EmulateApiRequest, NeonApiState};

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[post("/trace")]
pub async fn trace(
    state: NeonApiState,
    request_id: RequestId,
    Json(trace_request): Json<EmulateApiRequest>,
) -> impl Responder {
    let slot = trace_request.slot;
    let index = trace_request.tx_index_in_block;

    let rpc = match state.build_rpc(slot, index).await {
        Ok(rpc) => rpc,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &trace_transaction(&rpc, state.config.evm_loader, trace_request.body)
            .await
            .map_err(Into::into),
    )
}
