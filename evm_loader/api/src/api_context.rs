use crate::NeonApiState;
use hex::FromHex;
use neon_lib::rpc::{CallDbClient, TrxDbClient};
use neon_lib::{context, rpc, NeonError};
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

pub async fn build_hash_rpc_client(
    state: &NeonApiState,
    hash: &str,
) -> Result<Arc<dyn rpc::Rpc>, NeonError> {
    let hash = <[u8; 32]>::from_hex(context::truncate_0x(hash))?;
    Ok(Arc::new(
        TrxDbClient::new(state.tracer_db.clone(), state.indexer_db.clone(), hash).await,
    ))
}
