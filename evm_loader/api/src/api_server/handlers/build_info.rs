use crate::build_info::get_build_info;
use axum::{http::StatusCode, Json};
use neon_lib::build_info_common::SlimBuildInfo;

#[tracing::instrument(ret)]
pub async fn build_info() -> (StatusCode, Json<SlimBuildInfo>) {
    (StatusCode::OK, Json(get_build_info()))
}
