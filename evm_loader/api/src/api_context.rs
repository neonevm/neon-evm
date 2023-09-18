use crate::NeonApiState;
use neon_lib::rpc::CallDbClient;
use neon_lib::{rpc, NeonError};
use std::sync::Arc;

pub async fn build_rpc_client(
    state: &NeonApiState,
    slot: Option<u64>,
) -> Result<Arc<dyn rpc::Rpc>, NeonError> {
    if let Some(slot) = slot {
        return build_call_db_client(state, slot).await;
    }

    Ok(state.rpc_client.clone())
}

pub async fn build_call_db_client(
    state: &NeonApiState,
    slot: u64,
) -> Result<Arc<dyn rpc::Rpc>, NeonError> {
    Ok(Arc::new(
        CallDbClient::new(state.tracer_db.clone(), slot).await?,
    ))
}
