use std::sync::Arc;

use crate::{
    rpc::{self},
    Config, NeonError,
};
use solana_clap_utils::keypair::signer_from_path;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Signer;

pub fn truncate_0x(in_str: &str) -> &str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {
        in_str
    }
}

pub struct Context {
    pub rpc_client: Arc<dyn rpc::Rpc>,
    signer_config: Arc<Config>,
}

impl Context {
    pub fn signer(&self) -> Result<Box<dyn Signer>, NeonError> {
        build_signer(&self.signer_config)
    }

    pub async fn new_from_config(
        config: Arc<Config>,
        slot: Option<u64>,
    ) -> Result<Self, NeonError> {
        let rpc_client: Arc<dyn rpc::Rpc> = if let Some(slot) = slot {
            Arc::new(
                rpc::CallDbClient::new(
                    crate::types::TracerDb::new(
                        config.db_config.as_ref().expect("db-config not found"),
                    ),
                    slot,
                )
                .await?,
            )
        } else {
            Arc::new(RpcClient::new_with_commitment(
                config.json_rpc_url.clone(),
                config.commitment,
            ))
        };

        Ok(Self {
            rpc_client,
            signer_config: config.clone(),
        })
    }

    pub fn new(rpc_client: Arc<dyn rpc::Rpc>, signer_config: Arc<Config>) -> Self {
        Self {
            rpc_client,
            signer_config,
        }
    }
}

/// # Errors
pub fn build_signer(config: &Config) -> Result<Box<dyn Signer>, NeonError> {
    let mut wallet_manager = None;

    let signer = signer_from_path(
        &Default::default(),
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonError::KeypairNotSpecified)?;

    Ok(signer)
}
