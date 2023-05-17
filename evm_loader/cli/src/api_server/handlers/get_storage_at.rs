use crate::{
    api_server::{request_models::GetStorageAtRequest, state::State},
    context,
};
use evm_loader::types::Address;
use tide::{Request, Result};

use crate::commands::get_storage_at as GetStorageAtCommand;

use super::{process_result, u256_of};

#[allow(clippy::unused_async)]
pub async fn get_storage_at(req: Request<State>) -> Result<serde_json::Value> {
    let state = req.state();

    let req_params: GetStorageAtRequest = req.query().unwrap_or_default();

    let address = Address::from_hex(req_params.contract_id.as_str())
        .map_err(|_| tide::Error::from_str(400, "address is incorrect"))?;

    let index = match u256_of(req_params.index.unwrap_or_default().as_str()) {
        Some(index) => index,
        None => return Err(tide::Error::from_str(400, "Index parse error")),
    };

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
        address,
        &index,
    ))
}
