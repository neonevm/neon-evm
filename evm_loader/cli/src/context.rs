use std::sync::Arc;

use clap::ArgMatches;
use hex::FromHex;
use neon_lib::context::truncate;
pub use neon_lib::context::*;
use neon_lib::rpc;
use neon_lib::rpc::CallDbClient;
use neon_lib::rpc::TrxDbClient;
use neon_lib::Config;
use neon_lib::NeonError;
use solana_clap_utils::keypair::signer_from_path;
use solana_client::nonblocking::rpc_client::RpcClient;

/// # Errors
pub async fn create_from_config_and_options<'a>(
    options: &'a ArgMatches<'a>,
    config: &'a Config,
) -> Result<Context, NeonError> {
    let (cmd, params) = options.subcommand();

    let slot = options.value_of("slot");

    let (rpc_client, blocking_rpc_client): (Arc<dyn rpc::Rpc + Send + Sync>, _) =
        match (cmd, params) {
            ("emulate_hash" | "trace_hash", Some(params)) => {
                let hash = params.value_of("hash").expect("hash not found");
                let hash = <[u8; 32]>::from_hex(truncate(hash)).expect("hash cast error");

                (
                    Arc::new(
                        TrxDbClient::new(
                            config.db_config.as_ref().expect("db-config not found"),
                            hash,
                        )
                        .await,
                    ),
                    None,
                )
            }
            _ => {
                if let Some(slot) = slot {
                    let slot = slot.parse().expect("incorrect slot");
                    (
                        Arc::new(CallDbClient::new(
                            config.db_config.as_ref().expect("db-config not found"),
                            slot,
                        )),
                        None,
                    )
                } else {
                    (
                        Arc::new(RpcClient::new_with_commitment(
                            config.json_rpc_url.clone(),
                            config.commitment,
                        )),
                        Some(Arc::new(
                            solana_client::rpc_client::RpcClient::new_with_commitment(
                                config.json_rpc_url.clone(),
                                config.commitment,
                            ),
                        )),
                    )
                }
            }
        };

    let mut wallet_manager = None;

    let signer = Arc::from(
        signer_from_path(
            options,
            &config.keypair_path,
            "keypair",
            &mut wallet_manager,
        )
        .map_err(|_| NeonError::KeypairNotSpecified)?,
    );

    Ok(Context {
        rpc_client,
        signer,
        blocking_rpc_client,
    })
}
