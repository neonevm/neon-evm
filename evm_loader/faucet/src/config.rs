//! Faucet config module.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::RwLock;

/// Represents config errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read config '{1}': {0}")]
    Read(#[source] std::io::Error, std::path::PathBuf),
    #[error("Failed to parse config '{1}': {0}")]
    Parse(#[source] toml::de::Error, std::path::PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const DEFAULT_CONFIG: &str = "faucet.conf";

/// Loads the config from a file.
pub fn load(filename: &Path) -> Result<()> {
    CONFIG.write().unwrap().load(filename)?;
    Ok(())
}

/// Gets the `rpc_port` value.
pub fn rpc_port() -> u16 {
    CONFIG.read().unwrap().rpc_port
}

/// Gets the `ethereum_endpoint` value.
pub fn ethereum_endpoint() -> String {
    CONFIG.read().unwrap().ethereum_endpoint.clone()
}

/// Gets the `token_a` value.
pub fn token_a() -> String {
    CONFIG.read().unwrap().token_a.clone()
}

/// Gets the `token_b` value.
pub fn token_b() -> String {
    CONFIG.read().unwrap().token_b.clone()
}

/// Gets the `admin` value.
pub fn admin() -> String {
    CONFIG.read().unwrap().admin.clone()
}

/// Represents the main config.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Faucet {
    pub environment: String,
    pub rpc_port: u16,
    pub ethereum_endpoint: String,
    pub token_a: String,
    pub token_b: String,
    pub admin: String,
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
