use axum::{
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;

// use evm_loader::types::Address;
use crate::{
    api_server::handlers::{
        build_info::build_info, emulate::emulate, get_ether_account_data::get_ether_account_data,
        get_storage_at::get_storage_at, trace::trace,
    },
    NeonApiState,
};

pub fn register() -> Router<NeonApiState> {
    ServiceBuilder::new().service::<Router<NeonApiState>>(
        Router::new()
            .route("/emulate", post(emulate)) // Obsolete
            .route("/get-storage-at", get(get_storage_at))
            .route("/get-ether-account-data", get(get_ether_account_data))
            .route("/trace", post(trace))
            .route("/build-info", get(build_info)),
    )
}
