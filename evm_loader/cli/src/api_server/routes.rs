use axum::{
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;

// use evm_loader::types::Address;
use crate::{
    api_server::handlers::{
        emulate::emulate, emulate_hash::emulate_hash,
        get_ether_account_data::get_ether_account_data, get_storage_at::get_storage_at,
        trace::trace, trace_hash::trace_hash,
    },
    NeonApiState,
};

pub fn register(s: NeonApiState) -> Router<NeonApiState> {
    ServiceBuilder::new().service::<Router<NeonApiState>>(
        Router::new()
            .route("/emulate", post(emulate))
            .route("/emulate_hash", post(emulate_hash))
            .route("/get-storage-at", get(get_storage_at))
            .route("/get-ether-account-data", get(get_ether_account_data))
            .route("/trace", post(trace))
            .route("/trace_hash", post(trace_hash))
            .with_state(s),
    )
}
