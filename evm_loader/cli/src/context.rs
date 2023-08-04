use std::sync::Arc;

use clap::ArgMatches;
use hex::FromHex;
use neon_lib::context::truncate_0x;
pub use neon_lib::context::*;
use neon_lib::rpc;
use neon_lib::rpc::CallDbClient;
use neon_lib::rpc::TrxDbClient;
use neon_lib::Config;
use neon_lib::NeonError;
use solana_client::nonblocking::rpc_client::RpcClient;

/// # Errors
pub async fn create_from_config_and_options<'a>(
    options: &'a ArgMatches<'a>,
    config: Arc<Config>,
    slot: &'a Option<u64>,
) -> Result<Context, NeonError> {
    let (cmd, params) = options.subcommand();

    let rpc_client: Arc<dyn rpc::Rpc> = match (cmd, params) {
        ("emulate-hash" | "trace-hash" | "emulate_hash" | "trace_hash", Some(params)) => {
            let hash = params.value_of("hash").expect("hash not found");
            let hash = <[u8; 32]>::from_hex(truncate_0x(hash)).expect("hash cast error");

            Arc::new(
                TrxDbClient::new(
                    config.db_config.as_ref().expect("db-config not found"),
                    hash,
                )
                .await,
            )
        }
        _ => {
            if let Some(slot) = slot {
                Arc::new(CallDbClient::new(
                    config.db_config.as_ref().expect("db-config not found"),
                    *slot,
                ))
            } else {
                Arc::new(RpcClient::new_with_commitment(
                    config.json_rpc_url.clone(),
                    config.commitment,
                ))
            }
        }
    };

    Ok(neon_lib::context::create(rpc_client, config.clone()))
}
