use crate::{
    program_options::truncate,
    rpc,
    rpc::{CallDbClient, TrxDbClient},
    Config, NeonCliError,
};
use clap::ArgMatches;
use hex::FromHex;
use solana_clap_utils::keypair::signer_from_path;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Signer;

pub struct Context {
    pub rpc_client: Box<dyn rpc::Rpc>,
    pub signer: Box<dyn Signer>,
}

#[must_use]
pub fn create(rpc_client: Box<dyn rpc::Rpc>, signer: Box<dyn Signer>) -> Context {
    Context { rpc_client, signer }
}

/// # Errors
pub fn create_from_config_and_options(
    options: &ArgMatches,
    config: &Config,
) -> Result<Context, NeonCliError> {
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
    .map_err(|_| NeonCliError::KeypairNotSpecified)?;

    Ok(Context { rpc_client, signer })
}

/// # Errors
pub fn build_signer(config: &Config) -> Result<Box<dyn Signer>, NeonCliError> {
    let mut wallet_manager = None;

    let signer = signer_from_path(
        &ArgMatches::default(),
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonCliError::KeypairNotSpecified)?;

    Ok(signer)
}
/// # Errors
pub fn build_hash_rpc_client(
    config: &Config,
    hash: &str,
) -> Result<Box<dyn rpc::Rpc>, NeonCliError> {
    let hash = <[u8; 32]>::from_hex(truncate(hash))?;

    Ok(Box::new(TrxDbClient::new(
        config.db_config.as_ref().expect("db-config not found"),
        hash,
    )))
}

/// # Errors
pub fn build_rpc_client(
    config: &Config,
    slot: Option<u64>,
) -> Result<Box<dyn rpc::Rpc>, NeonCliError> {
    if let Some(slot) = slot {
        let config = config
            .db_config
            .clone()
            .ok_or(NeonCliError::InvalidChDbConfig)?;
        return Ok(Box::new(CallDbClient::new(&config, slot)));
    }

    Ok(Box::new(RpcClient::new_with_commitment(
        config.json_rpc_url.clone(),
        config.commitment,
    )))
}
