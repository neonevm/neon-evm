use crate::Config;
use neon_lib::types::{IndexerDb, TracerDb};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub tracer_db: TracerDb,
    pub indexer_db: IndexerDb,
    pub rpc_client: Arc<RpcClient>,
    pub config: Arc<Config>,
}

impl State {
    pub async fn new(config: Config) -> Self {
        let db_config = config.db_config.as_ref().expect("db-config not found");
        Self {
            tracer_db: TracerDb::new(db_config),
            indexer_db: IndexerDb::new(db_config).await,
            rpc_client: Arc::new(RpcClient::new_with_commitment(
                config.json_rpc_url.clone(),
                config.commitment,
            )),
            config: Arc::new(config),
        }
    }
}
