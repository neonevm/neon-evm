use std::{env, str::FromStr};

use crate::types::ChDbConfig;
use serde::{Deserialize, Serialize};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

#[derive(Debug)]
pub struct Config {
    pub evm_loader: Pubkey,
    pub key_for_config: Pubkey,
    pub fee_payer: Option<Keypair>,
    pub commitment: CommitmentConfig,
    pub solana_cli_config: solana_cli_config::Config,
    pub db_config: Option<ChDbConfig>,
    pub json_rpc_url: String,
    pub keypair_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct APIOptions {
    pub solana_cli_config_path: Option<String>,
    pub commitment: CommitmentConfig,
    pub json_rpc_url: String,
    pub evm_loader: Pubkey,
    pub key_for_config: Pubkey,
    pub db_config: ChDbConfig,
}

/// # Errors
#[must_use]
pub fn load_api_config_from_enviroment() -> APIOptions {
    let solana_cli_config_path: Option<String> =
        env::var("SOLANA_CLI_CONFIG_PATH").map(Some).unwrap_or(None);

    let commitment = env::var("COMMITMENT")
        .map(|v| v.to_lowercase())
        .ok()
        .and_then(|s| CommitmentConfig::from_str(&s).ok())
        .unwrap_or(CommitmentConfig::confirmed());

    let json_rpc_url = env::var("SOLANA_URL").expect("solana url variable must be set");

    let evm_loader = env::var("EVM_LOADER")
        .ok()
        .and_then(|v| Pubkey::from_str(&v).ok())
        .expect("EVM_LOADER variable must be a valid pubkey");

    let key_for_config = env::var("SOLANA_KEY_FOR_CONFIG")
        .ok()
        .and_then(|v| Pubkey::from_str(&v).ok())
        .expect("SOLANA_KEY_FOR_CONFIG variable must be a valid pubkey");

    let db_config = load_db_config_from_enviroment();

    APIOptions {
        solana_cli_config_path,
        commitment,
        json_rpc_url,
        evm_loader,
        key_for_config,
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

    ChDbConfig {
        clickhouse_url,
        clickhouse_user,
        clickhouse_password,
    }
}
