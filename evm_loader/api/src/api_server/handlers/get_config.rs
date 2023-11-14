use crate::api_server::handlers::process_error;
use crate::{api_context, NeonApiState};
use actix_request_identifier::RequestId;
use actix_web::routes;
use actix_web::{http::StatusCode, Responder};
use std::convert::Into;

use crate::commands::get_config as GetConfigCommand;

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[routes]
#[post("/config")]
#[get("/config")]
pub async fn get_config(state: NeonApiState, request_id: RequestId) -> impl Responder {
    let rpc_client = match api_context::build_rpc_client(&state, None, None).await {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &GetConfigCommand::execute(rpc_client.as_ref(), state.config.evm_loader)
            .await
            .map_err(Into::into),
    )
}
