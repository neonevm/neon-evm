//! faucet config module.

pub const DEFAULT_CONFIG: &str = "faucet.toml";

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents the main config.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Faucet {
    pub environment: String,
    pub rpc_port: String,
    pub ethereum_endpoint: String,
    pub token_a: String,
    pub token_b: String,
    pub admin: String,
}

/// Implements construction of the config.
impl Faucet {
    pub fn load(filename: &Path) -> Result<Self> {
        let text = read(filename)?;
        let cfg: Faucet =
            toml::from_str(&text).map_err(|e| Error::Parse(e, filename.to_owned()))?;
        Ok(cfg)
    }
}

/// Reads the main config from a file.
fn read(filename: &Path) -> Result<String> {
    std::fs::read_to_string(filename).map_err(|e| Error::Read(e, filename.to_owned()))
}

/// Represents config errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read config '{1}': {0}")]
    Read(#[source] std::io::Error, std::path::PathBuf),
    #[error("Failed to parse config '{1}': {0}")]
    Parse(#[source] toml::de::Error, std::path::PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;
