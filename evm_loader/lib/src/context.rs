use std::sync::Arc;

use crate::{
    rpc::CallDbClient,
    rpc::{self, TrxDbClient},
    Config, NeonError,
};
use hex::FromHex;
use solana_clap_utils::keypair::signer_from_path;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_client::RpcClient as BlockingRpcClient;
use solana_sdk::signature::Signer;

type FullRpcClient = (
    Arc<dyn rpc::Rpc + Send + Sync>,
    Option<Arc<BlockingRpcClient>>,
);

/// # Errors
pub async fn build_hash_rpc_client(
    config: &Config,
    hash: &str,
) -> Result<FullRpcClient, NeonError> {
    let hash = <[u8; 32]>::from_hex(truncate(hash))?;

    Ok((
        Arc::new(
            TrxDbClient::new(
                config.db_config.as_ref().expect("db-config not found"),
                hash,
            )
            .await,
        ),
        None,
    ))
}

pub fn truncate(in_str: &str) -> &str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {
        in_str
    }
}

pub struct Context {
    pub rpc_client: Arc<dyn rpc::Rpc + Send + Sync>,
    pub signer: Arc<dyn Signer>,
    pub blocking_rpc_client: Option<Arc<BlockingRpcClient>>,
}

#[must_use]
pub fn create(
    rpc_client: Arc<dyn rpc::Rpc + Send + Sync>,
    signer: Arc<dyn Signer>,
    blocking_rpc_client: Option<Arc<BlockingRpcClient>>,
) -> Context {
    Context {
        rpc_client,
        signer,
        blocking_rpc_client,
    }
}

/// # Errors
pub fn build_signer(config: &Config) -> Result<Arc<dyn Signer>, NeonError> {
    let mut wallet_manager = None;

    let signer = signer_from_path(
        &Default::default(),
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonError::KeypairNotSpecified)?;

    Ok(Arc::from(signer))
}

/// # Errors
pub fn build_rpc_client(config: &Config, slot: Option<u64>) -> Result<FullRpcClient, NeonError> {
    if let Some(slot) = slot {
        let config = config
            .db_config
            .clone()
            .ok_or(NeonError::InvalidChDbConfig)?;
        return Ok((Arc::new(CallDbClient::new(&config, slot)), None));
    }

    Ok((
        Arc::new(RpcClient::new_with_commitment(
            config.json_rpc_url.clone(),
            config.commitment,
        )),
        Some(Arc::new(BlockingRpcClient::new_with_commitment(
            config.json_rpc_url.clone(),
            config.commitment,
        ))),
    ))
}
