use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::normalize_to_url_if_moniker,
    keypair::{signer_from_path, keypair_from_path},
};
use crate::{rpc, rpc::{db::PostgresClient, NODE_INSTANCE, DB_INSTANCE}, NeonCliError};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer,},
};
use solana_client::rpc_client::RpcClient;
use std::{fmt, fmt::Debug, process::{exit}, str::FromStr, sync::Arc,};
use clap::ArgMatches;
use log::error;


pub struct Config {
    pub rpc_client: rpc::Clients,
    pub evm_loader: Pubkey,
    pub signer: Box<dyn Signer>,
    pub fee_payer: Option<Keypair>,
    pub commitment: CommitmentConfig,
}

impl Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "evm_loader={:?}, signer={:?}", self.evm_loader, self.signer)
    }
}

/// # Panics
pub fn create(options: &ArgMatches) -> Config {

    let cli_config = options.value_of("config_file").map_or_else(
        solana_cli_config::Config::default,
        |config_file| solana_cli_config::Config::load(config_file).unwrap_or_default()
    );

    let commitment = CommitmentConfig::from_str(options.value_of("commitment").unwrap()).unwrap();

    let json_rpc_url = normalize_to_url_if_moniker(
        options
            .value_of("json_rpc_url")
            .unwrap_or(&cli_config.json_rpc_url),
    );

    let evm_loader =
        if let Some(evm_loader) = pubkey_of(options, "evm_loader") {
            evm_loader
        } else {
            let e = NeonCliError::EvmLoaderNotSpecified;
            error!("{}", e);
            exit(e.error_code() as i32);
        };

    let mut wallet_manager = None;

    let signer = signer_from_path(
        options,
        options
            .value_of("keypair")
            .unwrap_or(&cli_config.keypair_path),
        "keypair",
        &mut wallet_manager,
    ).unwrap_or_else(
        |_| {
            let e = NeonCliError::KeypairNotSpecified;
            error!("{}", e);
            exit(e.error_code() as i32);
        }
    );

    let fee_payer = keypair_from_path(
        options,
        options
            .value_of("fee_payer")
            .unwrap_or(&cli_config.keypair_path),
        "fee_payer",
        true,
    ).ok();


    let rpc_client = if let Some(slot) = options.value_of("slot") {
        let slot:u64 = slot.parse().unwrap();

        let db_config = options.value_of("db_config")
            .map(|path|{ solana_cli_config::load_config_file(path).unwrap()})
            .unwrap();

        DB_INSTANCE.set(Arc::new(PostgresClient::new(&db_config, slot))).unwrap();
        rpc::Clients::Postgress
    } else {
        NODE_INSTANCE.set(
            Arc::new(RpcClient::new_with_commitment(json_rpc_url, commitment))
        ).unwrap_or_else(|_|{
            error!("NODE_INSTANCE.set error");
            exit(NeonCliError::UnknownError.error_code() as i32);
        });
        rpc::Clients::Node
    };

    Config {
        rpc_client,
        evm_loader,
        signer,
        fee_payer,
        commitment,
    }
}
