use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::normalize_to_url_if_moniker,
    keypair::{signer_from_path, keypair_from_path},
};
use crate::{rpc, rpc::{db_call_client::CallDbClient, db_trx_client::TrxDbClient}, NeonCliError, program_options::truncate};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer,},
};
use solana_client::rpc_client::RpcClient;
use std::{fmt, fmt::Debug, process::exit, str::FromStr,};
use clap::ArgMatches;
use log::error;
use hex::FromHex;

pub struct Config {
    pub rpc_client: Box<dyn rpc::Rpc>,
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

    let commitment = CommitmentConfig::from_str(options.value_of("commitment").expect("commitment parse error")).expect("CommitmentConfig ctor error");

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


    let db_config = options.value_of("db_config")
        .map(|path|{ solana_cli_config::load_config_file(path).expect("load db-config error")});
    let slot = options.value_of("slot");

    let (cmd, params) = options.subcommand();
    let rpc_client: Box<dyn rpc::Rpc> = match (cmd, params) {
        ("emulate_hash" | "trace_hash", Some(params)) => {
            let hash = params.value_of("hash").expect("hash not found");
            let hash = <[u8; 32]>::from_hex(truncate(hash)).expect("hash cast error");

            Box::new(TrxDbClient::new(&db_config.expect("db-config not found"), hash))
        }
        _ => {
            if let Some(slot) = slot {
                let slot: u64 = slot.parse().expect("slot parse error");
                Box::new(CallDbClient::new(&db_config.expect("db-config not found"), slot))
            } else{
                Box::new(RpcClient::new_with_commitment(json_rpc_url, commitment))
            }
        }
    };

    Config {
        rpc_client,
        evm_loader,
        signer,
        fee_payer,
        commitment,
    }
}



