use actix_web::{get, http::StatusCode, web, Responder};

use crate::commands::get_ether_account_data as GetEtherAccountDataCommand;
use crate::NeonApiState;
use crate::{context, types::request_models::GetEtherRequest};
use std::convert::Into;

use super::{process_error, process_result};

#[get("/get-ether-account-data")]
pub async fn get_ether_account_data(
    web::Query(req_params): web::Query<GetEtherRequest>,
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
        &GetEtherAccountDataCommand::execute(&state.config, &context, &req_params.ether)
            .await
            .map_err(Into::into),
    )
}
