use crate::{
    api_server::handlers::process_error,
    commands::trace::trace_block,
    context, errors,
    types::{request_models::TraceNextBlockRequestModel, IndexerDb},
    NeonApiState,
};
use actix_web::{http::StatusCode, post, web::Json, Responder};

use super::{parse_emulation_params, process_result};

#[post("/trace-next-block")]
pub async fn trace_next_block(
    state: NeonApiState,
    Json(trace_next_block_request): Json<TraceNextBlockRequestModel>,
) -> impl Responder {
    let rpc_client =
        match context::build_call_db_client(&state.config, trace_next_block_request.slot) {
            Ok(rpc_client) => rpc_client,
            Err(e) => return process_error(StatusCode::BAD_REQUEST, &e),
        };

    let context = context::create(&*rpc_client, &state.config);

    let (token, chain, steps, accounts, solana_accounts) = parse_emulation_params(
        &state.config,
        &context,
        &trace_next_block_request.emulation_params,
    )
    .await;

    let indexer_db = IndexerDb::new(
        state
            .config
            .db_config
            .as_ref()
            .expect("db-config is required"),
    )
    .await;

    // TODO: Query next block (which parent = slot) instead of getting slot + 1:
    let transactions = match indexer_db
        .get_block_transactions(trace_next_block_request.slot + 1)
        .await
    {
        Ok(transactions) => transactions,
        Err(e) => {
            return process_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &errors::NeonError::PostgreError(e),
            )
        }
    };

    process_result(
        &trace_block(
            context.rpc_client,
            state.config.evm_loader,
            transactions,
            token,
            chain,
            steps,
            state.config.commitment,
            &accounts,
            &solana_accounts,
            &trace_next_block_request.trace_config.unwrap_or_default(),
        )
        .await
        .map_err(Into::into),
    )
}
