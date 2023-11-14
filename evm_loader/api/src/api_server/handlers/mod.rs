use actix_web::http::StatusCode;
use actix_web::web::Json;
use serde::Serialize;
use serde_json::{json, Value};

use crate::errors::NeonError;
use crate::NeonApiResult;

use std::net::AddrParseError;
use tracing::error;

pub mod build_info;
pub mod emulate;
pub mod get_balance;
pub mod get_config;
pub mod get_contract;
pub mod get_holder;
pub mod get_storage_at;
pub mod trace;

#[derive(Debug)]
pub struct NeonApiError(pub NeonError);

impl NeonApiError {
    pub fn into_inner(self) -> NeonError {
        self.into()
    }
}

impl From<NeonError> for NeonApiError {
    fn from(value: NeonError) -> Self {
        NeonApiError(value)
    }
}

impl From<NeonApiError> for NeonError {
    fn from(value: NeonApiError) -> Self {
        value.0
    }
}

impl From<AddrParseError> for NeonApiError {
    fn from(value: AddrParseError) -> Self {
        NeonApiError(value.into())
    }
}

fn process_result<T: Serialize>(
    result: &NeonApiResult<T>,
) -> (Json<serde_json::Value>, StatusCode) {
    match result {
        Ok(value) => (
            Json(json!({
                "result": "success",
                "value": value,
            })),
            StatusCode::OK,
        ),
        Err(e) => process_error(StatusCode::INTERNAL_SERVER_ERROR, &e.0),
    }
}

fn process_error(status_code: StatusCode, e: &NeonError) -> (Json<Value>, StatusCode) {
    error!("NeonError: {e}");
    (
        Json(json!({
            "result": "error",
            "error": e.to_string(),
        })),
        status_code,
    )
}
