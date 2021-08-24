//! Faucet config module.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::RwLock;

pub const DEFAULT_CONFIG: &str = "faucet.conf";
pub const AUTO: &str = "auto";

/// Represents config errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read config '{1}': {0}")]
    Read(#[source] std::io::Error, std::path::PathBuf),
    #[error("Failed to parse config '{1}': {0}")]
    Parse(#[source] toml::de::Error, std::path::PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Loads the config from a file.
pub fn load(filename: &Path) -> Result<()> {
    CONFIG.write().unwrap().load(filename)?;
    Ok(())
}

/// Gets the `rpc.port` value.
pub fn rpc_port() -> u16 {
    CONFIG.read().unwrap().rpc.port
}

/// Gets the CORS `rpc.allowed_origins` urls.
pub fn allowed_origins() -> Vec<String> {
    CONFIG.read().unwrap().rpc.allowed_origins.clone()
}

/// Gets the `ethereum.endpoint` value.
pub fn ethereum_endpoint() -> String {
    CONFIG.read().unwrap().ethereum.endpoint.clone()
}

/// Gets the `ethereum.admin_key` private key value. Removes prefix 0x if any.
pub fn admin_key() -> String {
    let key = &CONFIG.read().unwrap().ethereum.admin_key;
    if key.len() < 3 || !key.starts_with("0x") {
        key.to_owned()
    } else {
        key[2..].to_owned()
    }
}

/// Gets the `ethereum.tokens` addresses.
pub fn tokens() -> Vec<String> {
    CONFIG.read().unwrap().ethereum.tokens.clone()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct General {
    environment: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Rpc {
    port: u16,
    allowed_origins: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Ethereum {
    endpoint: String,
    admin_key: String,
    tokens: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Faucet {
    general: General,
    rpc: Rpc,
    ethereum: Ethereum,
}

impl Faucet {
    /// Constructs config from a file.
    fn load(&mut self, filename: &Path) -> Result<()> {
        let text =
            std::fs::read_to_string(filename).map_err(|e| Error::Read(e, filename.to_owned()))?;
        *self = toml::from_str(&text).map_err(|e| Error::Parse(e, filename.to_owned()))?;
        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref CONFIG: RwLock<Faucet> = RwLock::new(Faucet::default());
}
