//! Faucet config module.

use std::collections::HashMap;
use std::convert::TryFrom as _;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use tracing::{error, warn};

use solana_client::rpc_client::RpcClient;
use solana_sdk::account_utils::StateMut;
use solana_sdk::bpf_loader;
use solana_sdk::bpf_loader_deprecated;
use solana_sdk::bpf_loader_upgradeable::{self, UpgradeableLoaderState};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;

use crate::{ethereum, id};

lazy_static::lazy_static! {
    static ref CONFIG: RwLock<Faucet> = RwLock::new(Faucet::default());
}

pub const DEFAULT_CONFIG: &str = "faucet.conf";
pub const AUTO: &str = "auto";

/// Represents the config errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read file '{1}': {0}")]
    Read(#[source] std::io::Error, PathBuf),

    #[error("Failed to parse config '{1}': {0}")]
    Parse(#[source] toml::de::Error, PathBuf),

    #[error("Failed to parse boolean literal from config")]
    ParseBool(#[from] std::str::ParseBoolError),

    #[error("Failed to parse integer number from config")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Failed to parse string literal '{0}' from config")]
    ParseString(String),

    #[error("Invalid keypair '{0}' from file '{1}'")]
    InvalidKeypair(String, PathBuf),

    #[error("Failed to parse keypair")]
    ParseKeypair(#[from] ed25519_dalek::SignatureError),

    #[error("Invalid pubkey '{0}'")]
    InvalidPubkey(String),

    #[error("Account not found '{0}'")]
    AccountNotFound(Pubkey),

    #[error("Associated PDA '{0}' not found for program '{1}'")]
    AssociatedPdaNotFound(Pubkey, Pubkey),

    #[error("Associated PDA '{0}' is not valid for program '{1}'")]
    InvalidAssociatedPda(Pubkey, Pubkey),

    #[error("Account is not upgradeable '{0}'")]
    AccountIsNotUpgradeable(Pubkey),

    #[error("Account is not BPF '{0}'")]
    AccountIsNotBpf(Pubkey),
}

/// Represents the config result type.
pub type Result<T> = std::result::Result<T, Error>;

const FAUCET_RPC_BIND: &str = "FAUCET_RPC_BIND";
const FAUCET_RPC_PORT: &str = "FAUCET_RPC_PORT";
const FAUCET_RPC_ALLOWED_ORIGINS: &str = "FAUCET_RPC_ALLOWED_ORIGINS";
const FAUCET_WEB3_ENABLE: &str = "FAUCET_WEB3_ENABLE";
const WEB3_RPC_URL: &str = "WEB3_RPC_URL";
const WEB3_PRIVATE_KEY: &str = "WEB3_PRIVATE_KEY";
const NEON_ERC20_TOKENS: &str = "NEON_ERC20_TOKENS";
const NEON_ERC20_MAX_AMOUNT: &str = "NEON_ERC20_MAX_AMOUNT";
const FAUCET_SOLANA_ENABLE: &str = "FAUCET_SOLANA_ENABLE";
const SOLANA_URL: &str = "SOLANA_URL";
const SOLANA_COMMITMENT: &str = "SOLANA_COMMITMENT";
const EVM_LOADER: &str = "EVM_LOADER";
const NEON_SEED_VERSION: &str = "NEON_SEED_VERSION";
const NEON_TOKEN_MINT: &str = "NEON_TOKEN_MINT";
const NEON_TOKEN_MINT_DECIMALS: &str = "NEON_TOKEN_MINT_DECIMALS";
const NEON_OPERATOR_KEYFILE: &str = "NEON_OPERATOR_KEYFILE";
const NEON_ETH_MAX_AMOUNT: &str = "NEON_ETH_MAX_AMOUNT";
const NEON_LOG: &str = "NEON_LOG";
const RUST_LOG: &str = "RUST_LOG";

static ENV: &[&str] = &[
    FAUCET_RPC_BIND,
    FAUCET_RPC_PORT,
    FAUCET_RPC_ALLOWED_ORIGINS,
    FAUCET_WEB3_ENABLE,
    WEB3_RPC_URL,
    WEB3_PRIVATE_KEY,
    NEON_ERC20_TOKENS,
    NEON_ERC20_MAX_AMOUNT,
    FAUCET_SOLANA_ENABLE,
    SOLANA_URL,
    SOLANA_COMMITMENT,
    EVM_LOADER,
    NEON_OPERATOR_KEYFILE,
    NEON_ETH_MAX_AMOUNT,
    NEON_LOG,
    RUST_LOG,
];

/// Reports if no file exists (it's normal, will be another source of config).
pub fn check_file_exists(file: &Path) {
    if !file.exists() {
        warn!(
            "{} File {:?} is missing; environment variables will be used",
            id::default(),
            file
        );
    }
}

/// Shows the environment variables and their values.
pub fn show_env() {
    for e in ENV {
        let val = env::var(e).unwrap_or_else(|_| " <undefined>".into());
        println!("{}={}", e, val);
    }
}

/// Loads the config from a file and applies defined environment variables.
pub fn load(file: &Path) -> Result<()> {
    if file.exists() {
        CONFIG.write().unwrap().load(file)?;
    }

    for e in ENV {
        if let Ok(val) = env::var(e) {
            match *e {
                FAUCET_RPC_BIND => CONFIG.write().unwrap().rpc.bind = val,
                FAUCET_RPC_PORT => CONFIG.write().unwrap().rpc.port = val.parse::<u16>()?,
                FAUCET_RPC_ALLOWED_ORIGINS => {
                    CONFIG.write().unwrap().rpc.allowed_origins = parse_list_of_strings(&val)?
                }
                FAUCET_WEB3_ENABLE => CONFIG.write().unwrap().web3.enable = val.parse::<bool>()?,
                WEB3_RPC_URL => CONFIG.write().unwrap().web3.rpc_url = val,
                WEB3_PRIVATE_KEY => CONFIG.write().unwrap().web3.private_key = val,
                NEON_ERC20_TOKENS => {
                    CONFIG.write().unwrap().web3.tokens = parse_list_of_strings(&val)?
                }
                NEON_ERC20_MAX_AMOUNT => {
                    CONFIG.write().unwrap().web3.max_amount = val.parse::<u64>()?
                }
                FAUCET_SOLANA_ENABLE => {
                    CONFIG.write().unwrap().solana.enable = val.parse::<bool>()?
                }
                SOLANA_URL => CONFIG.write().unwrap().solana.url = val,
                SOLANA_COMMITMENT => CONFIG.write().unwrap().solana.commitment = val,
                EVM_LOADER => CONFIG.write().unwrap().solana.evm_loader = val,
                NEON_OPERATOR_KEYFILE => {
                    CONFIG.write().unwrap().solana.operator_keyfile = val.into()
                }
                NEON_ETH_MAX_AMOUNT => {
                    CONFIG.write().unwrap().solana.max_amount = val.parse::<u64>()?
                }
                NEON_LOG => {}
                RUST_LOG => {}
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

/// Gets the `rpc.bind` value.
pub fn rpc_bind() -> String {
    let bind = CONFIG.read().unwrap().rpc.bind.clone();
    if bind.is_empty() {
        "0.0.0.0".into()
    } else {
        bind
    }
}

/// Gets the `rpc.port` value.
pub fn rpc_port() -> u16 {
    CONFIG.read().unwrap().rpc.port
}

/// Gets the CORS `rpc.allowed_origins` urls.
pub fn allowed_origins() -> Vec<String> {
    CONFIG.read().unwrap().rpc.allowed_origins.clone()
}

/// Gets the `web3.enable` value.
pub fn web3_enabled() -> bool {
    CONFIG.read().unwrap().web3.enable
}

/// Gets the `web3.rpc_url` value.
pub fn web3_rpc_url() -> String {
    CONFIG.read().unwrap().web3.rpc_url.clone()
}

/// Gets the `web3.private_key` value. Removes prefix 0x if any.
pub fn web3_private_key() -> String {
    let key = &CONFIG.read().unwrap().web3.private_key;
    ethereum::strip_0x_prefix(key).to_owned()
}

/// Gets the `web3.tokens` addresses.
pub fn tokens() -> Vec<String> {
    CONFIG.read().unwrap().web3.tokens.clone()
}

/// Gets the `web3.max_amount` value.
pub fn web3_max_amount() -> u64 {
    CONFIG.read().unwrap().web3.max_amount
}

/// Gets the `solana.enable` value.
pub fn solana_enabled() -> bool {
    CONFIG.read().unwrap().solana.enable
}

/// Gets the `solana.url` value.
pub fn solana_url() -> String {
    CONFIG.read().unwrap().solana.url.clone()
}

/// Gets the `solana.commitment` value.
pub fn solana_commitment() -> CommitmentConfig {
    let commitment = &CONFIG.read().unwrap().solana.commitment;
    match commitment.as_ref() {
        "processed" => CommitmentConfig::processed(),
        "confirmed" => CommitmentConfig::confirmed(),
        "finalized" => CommitmentConfig::finalized(),
        _ => {
            error!("Unexpected commitment level '{}'", commitment);
            error!("Allowed levels: 'processed', 'confirmed' or 'finalized'");
            warn!("The default level 'finalized' will be used");
            CommitmentConfig::default()
        }
    }
}

/// Gets the `solana.evm_loader` address value.
pub fn solana_evm_loader() -> String {
    CONFIG.read().unwrap().solana.evm_loader.clone()
}

/// Gets the `solana.account_seed_version` value.
pub fn solana_account_seed_version() -> u8 {
    CONFIG.read().unwrap().solana.account_seed_version
}

/// Gets the `solana.token_mint` address value.
pub fn solana_token_mint_id() -> String {
    CONFIG.read().unwrap().solana.token_mint.clone()
}

/// Gets the `solana.token_mint_decimals` value.
pub fn solana_token_mint_decimals() -> u8 {
    CONFIG.read().unwrap().solana.token_mint_decimals
}

/// Gets the `solana.operator` keypair value.
pub fn solana_operator_keypair() -> Result<Keypair> {
    let keyfile = CONFIG.read().unwrap().solana.operator_keyfile.clone();
    let key = std::fs::read_to_string(&keyfile).map_err(|e| Error::Read(e, keyfile.clone()))?;
    let key = key.trim();
    if !(key.starts_with('[') && key.ends_with(']')) {
        return Err(Error::InvalidKeypair(key.into(), keyfile));
    }
    let ss = split_comma_separated_list(trim_first_and_last_chars(key));
    let mut bytes = Vec::with_capacity(ss.len());
    for s in ss {
        bytes.push(s.parse::<u8>()?);
    }
    Ok(Keypair::from_bytes(&bytes)?)
}

/// Gets the `solana.max_amount` value
pub fn solana_max_amount() -> u64 {
    CONFIG.read().unwrap().solana.max_amount
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct Rpc {
    bind: String,
    port: u16,
    allowed_origins: Vec<String>,
}

impl std::fmt::Display for Rpc {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "rpc.bind = \"{}\"", self.bind)?;
        if env::var(FAUCET_RPC_BIND).is_ok() {
            writeln!(f, " (overridden by {})", FAUCET_RPC_BIND)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "rpc.port = {}", self.port)?;
        if env::var(FAUCET_RPC_PORT).is_ok() {
            writeln!(f, " (overridden by {})", FAUCET_RPC_PORT)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "rpc.allowed_origins = {:?}", self.allowed_origins)?;
        if env::var(FAUCET_RPC_ALLOWED_ORIGINS).is_ok() {
            write!(f, " (overridden by {})", FAUCET_RPC_ALLOWED_ORIGINS)
        } else {
            write!(f, "")
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct Web3 {
    enable: bool,
    rpc_url: String,
    private_key: String,
    tokens: Vec<String>,
    max_amount: u64,
}

impl std::fmt::Display for Web3 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "web3.enable = {}", self.enable)?;
        if env::var(FAUCET_WEB3_ENABLE).is_ok() {
            write!(f, " (overridden by {})", FAUCET_WEB3_ENABLE)?;
        } else {
            write!(f, "")?;
        }
        if !self.enable {
            return Ok(());
        }
        writeln!(f)?;
        write!(f, "web3.rpc_url = \"{}\"", self.rpc_url)?;
        if env::var(WEB3_RPC_URL).is_ok() {
            writeln!(f, " (overridden by {})", WEB3_RPC_URL)?;
        } else {
            writeln!(f)?;
        }
        write!(
            f,
            "web3.private_key = \"{}\"",
            obfuscate_string(&self.private_key)
        )?;
        if env::var(WEB3_PRIVATE_KEY).is_ok() {
            writeln!(f, " (overridden by {})", WEB3_PRIVATE_KEY)?;
        } else {
            writeln!(f)?;
        }
        write!(
            f,
            "web3.tokens = {:?}",
            obfuscate_list_of_strings(&self.tokens)
        )?;
        if env::var(NEON_ERC20_TOKENS).is_ok() {
            writeln!(f, " (overridden by {})", NEON_ERC20_TOKENS)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "web3.max_amount = {}", self.max_amount)?;
        if env::var(NEON_ERC20_MAX_AMOUNT).is_ok() {
            write!(f, " (overridden by {})", NEON_ERC20_MAX_AMOUNT)
        } else {
            write!(f, "")
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct Solana {
    enable: bool,
    url: String,
    commitment: String,
    evm_loader: String,
    account_seed_version: u8, // from neon params
    token_mint: String,       // from neon params
    token_mint_decimals: u8,  // from neon params
    operator_keyfile: PathBuf,
    max_amount: u64,
}

impl std::fmt::Display for Solana {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "solana.enable = {}", self.enable)?;
        if env::var(FAUCET_SOLANA_ENABLE).is_ok() {
            write!(f, " (overridden by {})", FAUCET_SOLANA_ENABLE)?;
        } else {
            write!(f, "")?;
        }
        if !self.enable {
            return Ok(());
        }
        writeln!(f)?;
        write!(f, "solana.url = \"{}\"", self.url)?;
        if env::var(SOLANA_URL).is_ok() {
            writeln!(f, " (overridden by {})", SOLANA_URL)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "solana.commitment = \"{}\"", self.commitment)?;
        if env::var(SOLANA_COMMITMENT).is_ok() {
            writeln!(f, " (overridden by {})", SOLANA_COMMITMENT)?;
        } else {
            writeln!(f)?;
        }
        write!(
            f,
            "solana.evm_loader = {:?}",
            obfuscate_string(&self.evm_loader)
        )?;
        if env::var(EVM_LOADER).is_ok() {
            writeln!(f, " (overridden by {})", EVM_LOADER)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "solana.operator_keyfile = {:?}", self.operator_keyfile)?;
        if env::var(NEON_OPERATOR_KEYFILE).is_ok() {
            writeln!(f, " (overridden by {})", NEON_OPERATOR_KEYFILE)?;
        } else {
            writeln!(f)?;
        }
        write!(f, "solana.max_amount = {}", self.max_amount)?;
        if env::var(NEON_ETH_MAX_AMOUNT).is_ok() {
            write!(f, " (overridden by {})", NEON_ETH_MAX_AMOUNT)
        } else {
            write!(f, "")
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct Faucet {
    rpc: Rpc,
    web3: Web3,
    solana: Solana,
}

impl Faucet {
    /// Constructs config from a file.
    fn load(&mut self, file: &Path) -> Result<()> {
        let text = std::fs::read_to_string(file).map_err(|e| Error::Read(e, file.to_owned()))?;
        *self = toml::from_str(&text).map_err(|e| Error::Parse(e, file.to_owned()))?;
        Ok(())
    }
}

impl std::fmt::Display for Faucet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{}", self.rpc)?;
        writeln!(f, "{}", self.web3)?;
        write!(f, "{}", self.solana)
    }
}

fn obfuscate_list_of_strings(keys: &[String]) -> Vec<String> {
    keys.iter().map(|s| obfuscate_string(s)).collect()
}

/// Cuts middle part of a key like `0x1234ABC`.
fn obfuscate_string(key: &str) -> String {
    let len = key.len();
    let prefix_len = if key.starts_with("0x") { 6 } else { 4 };
    let suffix_len = 4;
    if len <= prefix_len + suffix_len {
        key.into()
    } else {
        format!("{}•••{}", &key[..prefix_len], &key[len - suffix_len..])
    }
}

/// Cuts middle part of a key like `[1,2,3...N]`.
#[allow(unused)]
fn obfuscate_solana_private_key(key: &str) -> String {
    let ss = split_comma_separated_list(key);
    let len = ss.len();
    if len <= 8 {
        key.into()
    } else {
        format!(
            "{},{},{},{}•••{},{},{},{}",
            ss[0],
            ss[1],
            ss[2],
            ss[3],
            ss[len - 4],
            ss[len - 3],
            ss[len - 2],
            ss[len - 1]
        )
    }
}

#[test]
fn test_obfuscate() {
    let s = obfuscate_string("123");
    assert_eq!(s, "123");
    let s = obfuscate_string("123456789");
    assert_eq!(s, "1234•••6789");
    let s = obfuscate_string("0x123456789");
    assert_eq!(s, "0x1234•••6789");

    let s = obfuscate_list_of_strings(&vec!["AAA".to_string(), "BBB".to_string()]);
    assert_eq!(s, vec!["AAA", "BBB"]);
    let s = obfuscate_list_of_strings(&vec!["CCCCCCCCC".to_string(), "DDDDDDDDD".to_string()]);
    assert_eq!(s, vec!["CCCC•••CCCC", "DDDD•••DDDD"]);

    let s = obfuscate_solana_private_key("123");
    assert_eq!(s, "123");
    let s = obfuscate_solana_private_key("1,2,3");
    assert_eq!(s, "1,2,3");
    let s = obfuscate_solana_private_key("1,2,3,4,5,6,7,8");
    assert_eq!(s, "1,2,3,4,5,6,7,8");
    let s = obfuscate_solana_private_key("1,2,3,4,5,6,7,8,9");
    assert_eq!(s, "1,2,3,4•••6,7,8,9");
}

/// Parses `s` as string representing bracketed list of quoted strings.
/// Example of input: ["AAA", "BBB", "CCC"]
fn parse_list_of_strings(s: &str) -> Result<Vec<String>> {
    let s = unquote('[', ']', s)?;
    s.split(',').map(|s| unquote('"', '"', s)).collect()
}

#[test]
fn test_parse_list_of_strings() {
    let vs = parse_list_of_strings("");
    assert!(vs.is_err());
    assert_eq!(format!("{:?}", vs.err().unwrap()), "ParseString(\"\")");

    let vs = parse_list_of_strings("[]");
    assert!(vs.is_err());
    assert_eq!(format!("{:?}", vs.err().unwrap()), "ParseString(\"\")");

    let vs = parse_list_of_strings("[A]");
    assert!(vs.is_err());
    assert_eq!(format!("{:?}", vs.err().unwrap()), "ParseString(\"A\")");

    let vs = parse_list_of_strings("[A,B,C]");
    assert!(vs.is_err());
    assert_eq!(format!("{:?}", vs.err().unwrap()), "ParseString(\"A\")");

    let vs = parse_list_of_strings("[\"A\",B,C]");
    assert!(vs.is_err());
    assert_eq!(format!("{:?}", vs.err().unwrap()), "ParseString(\"B\")");

    let vs = parse_list_of_strings("[\"A\"]");
    assert!(vs.is_ok());
    assert_eq!(vs.unwrap(), vec!["A"]);

    let vs = parse_list_of_strings("[\"A\",\"B\"]");
    assert!(vs.is_ok());
    assert_eq!(vs.unwrap(), vec!["A", "B"]);

    let vs = parse_list_of_strings("[\"A\",\"B\",\"C\"]");
    assert!(vs.is_ok());
    assert_eq!(vs.unwrap(), vec!["A", "B", "C"]);
}

/// Unquotes `s`.
fn unquote(left_quote: char, right_quote: char, s: &str) -> Result<String> {
    let s = s.trim();
    if !(s.starts_with(left_quote) && s.ends_with(right_quote)) {
        return Err(Error::ParseString(s.into()));
    }
    Ok(trim_first_and_last_chars(s).into())
}

#[test]
fn test_unquote() {
    const L: char = '[';
    const R: char = ']';

    let s = unquote(L, R, "");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"\")");

    let s = unquote(L, R, "A");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"A\")");

    let s = unquote(L, R, "AB");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"AB\")");

    let s = unquote(L, R, "ABC");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"ABC\")");

    let s = unquote(L, R, "[ABC");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"[ABC\")");

    let s = unquote(L, R, "ABC]");
    assert!(s.is_err());
    assert_eq!(format!("{:?}", s.err().unwrap()), "ParseString(\"ABC]\")");

    let s = unquote(L, R, "[]");
    assert!(s.is_ok());
    assert_eq!(s.unwrap(), "");

    let s = unquote(L, R, "[ABC]");
    assert!(s.is_ok());
    assert_eq!(s.unwrap(), "ABC");

    let s = unquote(L, R, "  [ABC]  ");
    assert!(s.is_ok());
    assert_eq!(s.unwrap(), "ABC");
}

/// Splits string as comma-separated list and trims whitespace.
/// String `"A ,B, C    "` will produce vector `["A","B","C"]`.
fn split_comma_separated_list(s: &str) -> Vec<String> {
    s.split(',').map(|s| s.trim().to_owned()).collect()
}

#[test]
fn test_split_comma_separated_list() {
    let ss = split_comma_separated_list("".into());
    assert_eq!(ss, vec!(""));
    let ss = split_comma_separated_list("ABC".into());
    assert_eq!(ss, vec!("ABC"));
    let ss = split_comma_separated_list("ABC,DEF".into());
    assert_eq!(ss, vec!("ABC", "DEF"));
    let ss = split_comma_separated_list("ABC,DEF,GHI".into());
    assert_eq!(ss, vec!("ABC", "DEF", "GHI"));
    let ss = split_comma_separated_list("ABC,".into());
    assert_eq!(ss, vec!("ABC", ""));
    let ss = split_comma_separated_list("ABC,,".into());
    assert_eq!(ss, vec!("ABC", "", ""));
    let ss = split_comma_separated_list(",ABC".into());
    assert_eq!(ss, vec!("", "ABC"));
    let ss = split_comma_separated_list("  ,  ,  ABC".into());
    assert_eq!(ss, vec!("", "", "ABC"));
    let ss = split_comma_separated_list("   ABC   ,   DEF   ,   GHI   ".into());
    assert_eq!(ss, vec!("ABC", "DEF", "GHI"));
}

/// Returns string without it's first and last characters.
/// Works with multi-byte characters and empty strings.
fn trim_first_and_last_chars(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next();
    chars.next_back();
    chars.as_str()
}

#[test]
fn test_trim_first_and_last_chars() {
    let s = trim_first_and_last_chars("");
    assert!(s.is_empty());
    let s = trim_first_and_last_chars("A");
    assert!(s.is_empty());
    let s = trim_first_and_last_chars("AB");
    assert!(s.is_empty());
    let s = trim_first_and_last_chars("ABC");
    assert_eq!(s, "B");
    let s = trim_first_and_last_chars("语言处理");
    assert_eq!(s, "言处");
}

/// Reads NEON parameters from the EVM Loader account.
pub async fn load_neon_params(client: Arc<RpcClient>) -> Result<()> {
    let params = tokio::task::spawn_blocking(move || -> Result<HashMap<String, String>> {
        read_neon_parameters_from_account(client)
    })
    .await
    .expect("Solana does not respond")?;

    for (param_name, val) in &params {
        match param_name.as_ref() {
            NEON_SEED_VERSION => {
                CONFIG.write().unwrap().solana.account_seed_version = val.parse::<u8>()?
            }
            NEON_TOKEN_MINT => CONFIG.write().unwrap().solana.token_mint = val.into(),
            NEON_TOKEN_MINT_DECIMALS => {
                CONFIG.write().unwrap().solana.token_mint_decimals = val.parse::<u8>()?
            }
            _ => {}
        }
    }

    Ok(())
}

#[allow(unused)]
fn read_neon_parameters_from_account(client: Arc<RpcClient>) -> Result<HashMap<String, String>> {
    let evm_loader_id = Pubkey::from_str(&solana_evm_loader())
        .map_err(|_| Error::InvalidPubkey(solana_evm_loader()))?;

    let account = client
        .get_account(&evm_loader_id)
        .map_err(|_| Error::AccountNotFound(evm_loader_id))?;

    if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
        Ok(read_elf_parameters(&account.data))
    } else if account.owner == bpf_loader_upgradeable::id() {
        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = account.state()
        {
            let programdata_account = client
                .get_account(&programdata_address)
                .map_err(|_| Error::AssociatedPdaNotFound(programdata_address, evm_loader_id))?;

            if let Ok(UpgradeableLoaderState::ProgramData { .. }) = programdata_account.state() {
                let offset = UpgradeableLoaderState::programdata_data_offset().unwrap_or(0);
                let program_data = &programdata_account.data[offset..];
                Ok(read_elf_parameters(program_data))
            } else {
                Err(Error::InvalidAssociatedPda(
                    programdata_address,
                    evm_loader_id,
                ))
            }
        } else if let Ok(UpgradeableLoaderState::Buffer { .. }) = account.state() {
            let offset = UpgradeableLoaderState::buffer_data_offset().unwrap_or(0);
            let program_data = &account.data[offset..];
            Ok(read_elf_parameters(program_data))
        } else {
            Err(Error::AccountIsNotUpgradeable(evm_loader_id))
        }
    } else {
        Err(Error::AccountIsNotBpf(evm_loader_id))
    }
}

fn read_elf_parameters(account_data: &[u8]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let elf = goblin::elf::Elf::parse(account_data).expect("Unable to parse ELF file");

    elf.dynsyms.iter().for_each(|sym| {
        let name = String::from(&elf.dynstrtab[sym.st_name]);
        if name.starts_with("NEON") {
            let end = account_data.len();
            let from: usize = usize::try_from(sym.st_value)
                .unwrap_or_else(|_| panic!("Unable to cast usize from u64:{:?}", sym.st_value));
            let to: usize = usize::try_from(sym.st_value + sym.st_size).unwrap_or_else(|err| {
                panic!(
                    "Unable to cast usize from u64:{:?}. Error: {}",
                    sym.st_value + sym.st_size,
                    err
                )
            });
            if to < end && from < end {
                let buf = &account_data[from..to];
                let value = std::str::from_utf8(buf).unwrap();
                result.insert(name, String::from(value));
            } else {
                panic!("{} is out of bounds", name);
            }
        }
    });

    result
}
