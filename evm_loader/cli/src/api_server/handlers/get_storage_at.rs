use crate::{api_server::state::State, context, types::request_models::GetStorageAtRequest};
use tide::{Request, Result};

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::process_result;

#[allow(clippy::unused_async)]
pub async fn get_storage_at(req: Request<State>) -> Result<serde_json::Value> {
    let state = req.state();

    let req_params: GetStorageAtRequest = req.query().unwrap_or_default();

    let signer = context::build_singer(&state.config).map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating singer: {:?}", e.to_string()),
        )
    })?;

    let rpc_client = context::build_rpc_client(&state.config, req_params.slot).map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating rpc client: {:?}", e.to_string()),
        )
    })?;

    let context = context::create(rpc_client, signer);

    process_result(&GetStorageAtCommand::execute(
        &state.config,
        &context,
        req_params.contract_id,
        &req_params.index,
    ))
}
