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
        trace::trace, trace_hash::trace_hash, trace_next_block::trace_next_block,
    },
    NeonApiState,
};

pub fn register() -> Router<NeonApiState> {
    ServiceBuilder::new().service::<Router<NeonApiState>>(
        Router::new()
            .route("/emulate", post(emulate))
            .route("/emulate-hash", post(emulate_hash))
            .route("/emulate_hash", post(emulate_hash)) // Obsolete
            .route("/get-storage-at", get(get_storage_at))
            .route("/get-ether-account-data", get(get_ether_account_data))
            .route("/trace", post(trace))
            .route("/trace-hash", post(trace_hash))
            .route("/trace_hash", post(trace_hash)) // Obsolete
            .route("/trace-next-block", post(trace_next_block)),
    )
}
