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
use solana_client::rpc_client::RpcClient;

/// # Errors
pub fn create_from_config_and_options(
    options: &ArgMatches,
    config: &Config,
) -> Result<Context, NeonError> {
    let (cmd, params) = options.subcommand();

    let slot = options.value_of("slot");

    let rpc_client: Box<dyn rpc::Rpc> = match (cmd, params) {
        ("emulate_hash" | "trace_hash", Some(params)) => {
            let hash = params.value_of("hash").expect("hash not found");
            let hash = <[u8; 32]>::from_hex(truncate(hash)).expect("hash cast error");

            Box::new(TrxDbClient::new(
                config.db_config.as_ref().expect("db-config not found"),
                hash,
            ))
        }
        _ => {
            if let Some(slot) = slot {
                let slot = slot.parse().expect("incorrect slot");
                Box::new(CallDbClient::new(
                    config.db_config.as_ref().expect("db-config not found"),
                    slot,
                ))
            } else {
                Box::new(RpcClient::new_with_commitment(
                    config.json_rpc_url.clone(),
                    config.commitment,
                ))
            }
        }
    };

    let mut wallet_manager = None;

    let signer = signer_from_path(
        options,
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonError::KeypairNotSpecified)?;

    Ok(Context { rpc_client, signer })
}
