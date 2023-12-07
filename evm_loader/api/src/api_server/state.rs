use crate::Config;
use neon_lib::rpc::{CallDbClient, CloneRpcClient, RpcEnum};
use neon_lib::types::TracerDb;
use neon_lib::NeonError;

pub struct State {
    pub tracer_db: TracerDb,
    pub rpc_client: CloneRpcClient,
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            tracer_db: TracerDb::new(config.db_config.as_ref().expect("db-config not found")),
            rpc_client: config.build_clone_solana_rpc_client(),
            config,
        }
    }

    pub async fn build_rpc(
        &self,
        slot: Option<u64>,
        tx_index_in_block: Option<u64>,
    ) -> Result<RpcEnum, NeonError> {
        Ok(if let Some(slot) = slot {
            RpcEnum::CallDbClient(
                CallDbClient::new(self.tracer_db.clone(), slot, tx_index_in_block).await?,
            )
        } else {
            RpcEnum::CloneRpcClient(self.rpc_client.clone())
        })
    }
}
