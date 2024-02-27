use neon_lib::config::APIOptions;
use neon_lib::rpc::{CallDbClient, CloneRpcClient, RpcEnum};
use neon_lib::types::TracerDb;
use neon_lib::NeonError;

pub struct State {
    pub tracer_db: TracerDb,
    pub rpc_client: CloneRpcClient,
    pub config: APIOptions,
}

impl State {
    pub fn new(config: APIOptions) -> Self {
        Self {
            tracer_db: TracerDb::new(&config.db_config),
            rpc_client: CloneRpcClient::new_from_api_config(&config),
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
