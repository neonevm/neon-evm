use crate::api_server::handlers::process_error;
use crate::NeonApiState;
use actix_request_identifier::RequestId;
use actix_web::routes;
use actix_web::{http::StatusCode, Responder};
use std::convert::Into;

use crate::commands::get_config as GetConfigCommand;

use super::process_result;

#[tracing::instrument(skip_all, fields(id = request_id.as_str()))]
#[routes]
#[post("/config")]
#[get("/config")]
pub async fn get_config(state: NeonApiState, request_id: RequestId) -> impl Responder {
    let rpc = match state.build_rpc(None, None).await {
        Ok(rpc) => rpc,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    process_result(
        &GetConfigCommand::execute(&rpc, state.config.evm_loader)
            .await
            .map_err(Into::into),
    )
}
