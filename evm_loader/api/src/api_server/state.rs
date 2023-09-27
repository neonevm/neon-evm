use crate::Config;
use neon_lib::types::TracerDb;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

pub struct State {
    pub tracer_db: TracerDb,
    pub rpc_client: Arc<RpcClient>,
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        let db_config = config.db_config.as_ref().expect("db-config not found");
        Self {
            tracer_db: TracerDb::new(db_config),
            rpc_client: Arc::new(RpcClient::new_with_commitment(
                config.json_rpc_url.clone(),
                config.commitment,
            )),
            config,
        }
    }
}
