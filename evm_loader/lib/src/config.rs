use std::{env, str::FromStr, sync::Arc};

use crate::{types::ChDbConfig, NeonError};
use serde::{Deserialize, Serialize};
use solana_clap_utils::{
    input_validators::normalize_to_url_if_moniker, keypair::keypair_from_path,
};
use solana_cli_config::Config as SolanaConfig;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

#[derive(Debug)]
pub struct Config {
    pub evm_loader: Pubkey,
    pub fee_payer: Option<Arc<Keypair>>,
    pub commitment: CommitmentConfig,
    pub solana_cli_config: solana_cli_config::Config,
    pub db_config: Option<ChDbConfig>,
    pub json_rpc_url: String,
    pub keypair_path: String,
}

// impl Debug for Config {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "evm_loader={:?}", self.evm_loader)
//     }
// }

/// # Errors
pub fn create_from_api_config(api_config: &APIOptions) -> Result<Config, NeonError> {
    let solana_cli_config: SolanaConfig =
        if let Some(path) = api_config.solana_cli_config_path.clone() {
            solana_cli_config::Config::load(path.as_str()).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };

    let commitment = CommitmentConfig::from_str(&api_config.commitment)
        .unwrap_or_else(|_| CommitmentConfig::confirmed());

    let json_rpc_url = normalize_to_url_if_moniker(api_config.json_rpc_url.clone());

    let evm_loader: Pubkey = if let Ok(val) = Pubkey::from_str(&api_config.evm_loader) {
        val
    } else {
        return Err(NeonError::EvmLoaderNotSpecified);
    };

    let keypair_path: String = api_config.keypair.clone();

    let fee_payer = keypair_from_path(
        &Default::default(),
        &api_config.fee_payer,
        "fee_payer",
        true,
    )
    .ok()
    .map(Arc::new);

    let db_config: Option<ChDbConfig> = Option::from(api_config.db_config.clone());

    Ok(Config {
        evm_loader,
        fee_payer,
        commitment,
        solana_cli_config,
        db_config,
        json_rpc_url,
        keypair_path,
    })
}

#[derive(Debug, Deserialize, Serialize)]
pub struct APIOptions {
    pub solana_cli_config_path: Option<String>,
    pub commitment: String,
    pub json_rpc_url: String,
    pub evm_loader: String,
    pub keypair: String,
    pub fee_payer: String,
    pub db_config: ChDbConfig,
}

/// # Errors
#[must_use]
pub fn load_api_config_from_enviroment() -> APIOptions {
    let solana_cli_config_path: Option<String> =
        env::var("SOLANA_CLI_CONFIG_PATH").map(Some).unwrap_or(None);

    let commitment = env::var("COMMITMENT")
        .map(|v| v.to_lowercase())
        .expect("commitment variable must be set");

    let json_rpc_url = env::var("SOLANA_URL").expect("solana url variable must be set");

    let evm_loader = env::var("EVM_LOADER").expect("evm loader variable must be set");

    let keypair = env::var("KEYPAIR").expect("keypair must variable be set");

    let fee_payer = env::var("FEEPAIR").expect("fee pair variable must be set");

    let db_config = load_db_config_from_enviroment();

    APIOptions {
        solana_cli_config_path,
        commitment,
        json_rpc_url,
        evm_loader,
        keypair,
        fee_payer,
        db_config,
    }
}

/// # Errors
fn load_db_config_from_enviroment() -> ChDbConfig {
    let clickhouse_url = env::var("NEON_DB_CLICKHOUSE_URLS")
        .map(|urls| {
            urls.split(';')
                .map(std::borrow::ToOwned::to_owned)
                .collect::<Vec<String>>()
        })
        .expect("neon clickhouse db urls valiable must be set");

    let clickhouse_user = env::var("NEON_DB_CLICKHOUSE_USER")
        .map(Some)
        .unwrap_or(None);

    let clickhouse_password = env::var("NEON_DB_CLICKHOUSE_PASSWORD")
        .map(Some)
        .unwrap_or(None);

    let indexer_host =
        env::var("NEON_DB_INDEXER_HOST").expect("neon db indexer host valiable must be set");

    let indexer_port =
        env::var("NEON_DB_INDEXER_PORT").expect("neon db indexer port valiable must be set");

    let indexer_database = env::var("NEON_DB_INDEXER_DATABASE")
        .expect("neon db indexer database valiable must be set");

    let indexer_user =
        env::var("NEON_DB_INDEXER_USER").expect("neon db indexer user valiable must be set");

    let indexer_password = env::var("NEON_DB_INDEXER_PASSWORD")
        .expect("neon db indexer password valiable must be set");

    ChDbConfig {
        clickhouse_url,
        clickhouse_user,
        clickhouse_password,
        indexer_host,
        indexer_port,
        indexer_database,
        indexer_user,
        indexer_password,
    }
}
