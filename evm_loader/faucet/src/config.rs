//! Faucet config module.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::sync::RwLock;

pub const DEFAULT_CONFIG: &str = "faucet.conf";
pub const AUTO: &str = "auto";

/// Represents the config errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read config '{1}': {0}")]
    Read(#[source] std::io::Error, std::path::PathBuf),
    #[error("Failed to parse config '{1}': {0}")]
    Parse(#[source] toml::de::Error, std::path::PathBuf),
}

/// Represents the config result type.
pub type Result<T> = std::result::Result<T, Error>;

const WEB3_RPC_URL: &str = "WEB3_RPC_URL";
const WEB3_PRIVATE_KEY: &str = "WEB3_PRIVATE_KEY";
static ENV: &[&str] = &[WEB3_RPC_URL, WEB3_PRIVATE_KEY];

/// Shows the environment variables and their values.
pub fn show_env() {
    for e in ENV {
        let val = env::var(e).unwrap_or_else(|_| " <undefined>".into());
        println!("{}={}", e, val);
    }
}

/// Loads the config from a file and applies defined environment variables.
pub fn load(filename: &Path) -> Result<()> {
    if filename.exists() {
        CONFIG.write().unwrap().load(filename)?;
    }

    for e in ENV {
        if let Ok(val) = env::var(e) {
            match *e {
                WEB3_RPC_URL => CONFIG.write().unwrap().web3.rpc_url = val,
                WEB3_PRIVATE_KEY => CONFIG.write().unwrap().web3.private_key = val,
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}

/// Shows the current config.
pub fn show() {
    println!("{}", CONFIG.read().unwrap())
}

/// Gets the `rpc.port` value.
pub fn rpc_port() -> u16 {
    CONFIG.read().unwrap().rpc.port
}

/// Gets the CORS `rpc.allowed_origins` urls.
pub fn allowed_origins() -> Vec<String> {
    CONFIG.read().unwrap().rpc.allowed_origins.clone()
}

/// Gets the `web3.rpc_url` value.
pub fn web3_rpc_url() -> String {
    CONFIG.read().unwrap().web3.rpc_url.clone()
}

/// Gets the `web3.private_key` value. Removes prefix 0x if any.
pub fn web3_private_key() -> String {
    let key = &CONFIG.read().unwrap().web3.private_key;
    if key.len() < 3 || !key.starts_with("0x") {
        key.to_owned()
    } else {
        key[2..].to_owned()
    }
}

/// Gets the `ethereum.tokens` addresses.
pub fn tokens() -> Vec<String> {
    CONFIG.read().unwrap().web3.tokens.clone()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Rpc {
    port: u16,
    allowed_origins: Vec<String>,
}

impl std::fmt::Display for Rpc {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "\nrpc.port = {}", self.port)?;
        write!(f, "rpc.allowed_origins = {:?}", self.allowed_origins)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Web3 {
    #[serde(default)]
    rpc_url: String,
    #[serde(default)]
    private_key: String,
    tokens: Vec<String>,
}

impl std::fmt::Display for Web3 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "\nweb3.rpc_url = {}", self.rpc_url)?;
        if env::var(WEB3_RPC_URL).is_ok() {
            writeln!(f, " (overridden with {})", WEB3_RPC_URL)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "web3.private_key = {}", self.private_key)?;
        if env::var(WEB3_PRIVATE_KEY).is_ok() {
            writeln!(f, " (overridden with {})", WEB3_PRIVATE_KEY)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "web3.tokens = {:?}", self.tokens)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Faucet {
    rpc: Rpc,
    web3: Web3,
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

impl std::fmt::Display for Faucet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.rpc)?;
        write!(f, "{}", self.web3)
    }
}

lazy_static::lazy_static! {
    static ref CONFIG: RwLock<Faucet> = RwLock::new(Faucet::default());
}
