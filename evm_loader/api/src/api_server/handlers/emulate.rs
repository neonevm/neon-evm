use actix_request_identifier::RequestId;
use actix_web::{http::StatusCode, post, web::Json, Responder};
use std::convert::Into;

use crate::api_server::handlers::process_error;
use crate::{commands::emulate as EmulateCommand, types::EmulateApiRequest, NeonApiState};

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[post("/emulate")]
pub async fn emulate(
    state: NeonApiState,
    request_id: RequestId,
    Json(emulate_request): Json<EmulateApiRequest>,
) -> impl Responder {
    let slot = emulate_request.slot;
    let index = emulate_request.tx_index_in_block;

    let rpc = match state.build_rpc(slot, index).await {
        Ok(rpc) => rpc,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &EmulateCommand::execute(&rpc, state.config.evm_loader, emulate_request.body, None)
            .await
            .map_err(Into::into),
    )
}
