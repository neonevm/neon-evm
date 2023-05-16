use tide::{Request, Result};

use crate::{
    api_server::{request_models::TxParamsRequest, state::State},
    context,
};

use super::{parse_tx, parse_tx_params, process_result};
use crate::commands::emulate as EmulateCommand;

#[allow(clippy::unused_async)]
pub async fn trace(mut req: Request<State>) -> Result<serde_json::Value> {
    let tx_params_request: TxParamsRequest = req.body_json().await.map_err(|e| {
        tide::Error::from_str(
            400,
            format!(
                "Error on parsing transaction parameters request: {:?}",
                e.to_string()
            ),
        )
    })?;

    let state = req.state();

    let tx: crate::types::TxParams = parse_tx(&tx_params_request);

    let signer = context::build_singer(&state.config).map_err(|e| {
        tide::Error::from_str(
            400,
            format!("Error on creating singer: {:?}", e.to_string()),
        )
    })?;

    let rpc_client =
        context::build_rpc_client(&state.config, tx_params_request.slot).map_err(|e| {
            tide::Error::from_str(
                400,
                format!("Error on creating rpc client: {:?}", e.to_string()),
            )
        })?;

    let context = context::create(rpc_client, signer);

    let (token, chain, steps, accounts, solana_accounts) =
        parse_tx_params(&state.config, &context, &tx_params_request);

    process_result(&EmulateCommand::execute(
        &state.config,
        &context,
        tx,
        token,
        chain,
        steps,
        &accounts,
        &solana_accounts,
    ))
}
