use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ethnum::U256;
use evm_loader::types::Address;
use serde_json::{json, Value};
use solana_sdk::pubkey::Pubkey;

use crate::commands::get_neon_elf::CachedElfParams;
use crate::errors::NeonCliError;
use crate::{Config, Context, NeonCliResult};

use crate::types::request_models::EmulationParamsRequestModel;
use std::str::FromStr;

pub mod emulate;
pub mod emulate_hash;
pub mod get_ether_account_data;
pub mod get_storage_at;
pub mod trace;
pub mod trace_hash;

pub fn u256_of(index: &str) -> Option<U256> {
    if index.is_empty() {
        return Some(U256::ZERO);
    }

    U256::from_str_prefixed(index).ok()
}

pub(crate) fn parse_emulation_params(
    config: &Config,
    context: &Context,
    params: &EmulationParamsRequestModel,
) -> (Pubkey, u64, u64, Vec<Address>, Vec<Pubkey>) {
    // Read ELF params only if token_mint or chain_id is not set.
    let mut token: Option<Pubkey> = params.token_mint.map(Into::into);
    let mut chain = params.chain_id;
    if token.is_none() || chain.is_none() {
        let cached_elf_params = CachedElfParams::new(config, context);
        token = token.or_else(|| {
            Some(
                Pubkey::from_str(
                    cached_elf_params
                        .get("NEON_TOKEN_MINT")
                        .expect("NEON_TOKEN_MINT load error"),
                )
                .expect("NEON_TOKEN_MINT Pubkey ctor error "),
            )
        });
        chain = chain.or_else(|| {
            Some(
                u64::from_str(
                    cached_elf_params
                        .get("NEON_CHAIN_ID")
                        .expect("NEON_CHAIN_ID load error"),
                )
                .expect("NEON_CHAIN_ID u64 ctor error"),
            )
        });
    }
    let token = token.expect("token_mint get error");
    let chain = chain.expect("chain_id get error");
    let max_steps = params.max_steps_to_execute;

    let accounts = params.cached_accounts.clone().unwrap_or_default();

    let solana_accounts = params
        .solana_accounts
        .clone()
        .map(|vec| vec.into_iter().map(Into::into).collect())
        .unwrap_or_default();

    (token, chain, max_steps, accounts, solana_accounts)
}

impl IntoResponse for NeonCliError {
    fn into_response(self) -> Response {
        let (status, error_message) = (StatusCode::INTERNAL_SERVER_ERROR, self.to_string());

        let body = Json(json!({
            "result": "error",
            "error":error_message,
        }));

        (status, body).into_response()
    }
}

fn process_result(result: &NeonCliResult) -> (StatusCode, Json<Value>) {
    match result {
        Ok(value) => (
            StatusCode::OK,
            Json(json!({
                "result": "success",
                "value": value.to_string(),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "result": "error",
                "error": e.to_string(),
            })),
        ),
    }
}

fn process_error(status_code: StatusCode, e: &NeonCliError) -> (StatusCode, Json<Value>) {
    (
        status_code,
        Json(json!({
            "result": "error",
            "error": e.to_string(),
        })),
    )
}
