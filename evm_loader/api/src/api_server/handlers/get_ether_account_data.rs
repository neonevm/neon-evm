use crate::api_server::handlers::process_error;
use crate::commands::get_ether_account_data as GetEtherAccountDataCommand;
use crate::{api_context, context::Context, types::request_models::GetEtherRequest, NeonApiState};
use actix_request_identifier::RequestId;
use actix_web::{get, http::StatusCode, web::Query, Responder};
use std::convert::Into;

use super::process_result;

#[tracing::instrument(skip(state, request_id), fields(id = request_id.as_str()))]
#[get("/get-ether-account-data")]
pub async fn get_ether_account_data(
    state: NeonApiState,
    request_id: RequestId,
    Query(req_params): Query<GetEtherRequest>,
) -> impl Responder {
    let rpc_client = match api_context::build_rpc_client(&state, req_params.slot, None).await {
        Ok(rpc_client) => rpc_client,
        Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
    };

    let context = Context::new(&*rpc_client, &state.config);

    process_result(
        &GetEtherAccountDataCommand::execute(
            context.rpc_client,
            &state.config.evm_loader,
            &req_params.ether,
        )
        .await
        .map_err(Into::into),
    )
}
