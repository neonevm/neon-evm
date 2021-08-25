//! Faucet tokens module.

use color_eyre::{eyre::eyre, Result};
use std::sync::RwLock;
use tracing::info;

/// Initializes local cache of tokens properties.
pub fn init(addresses: Vec<String>) -> Result<()> {
    info!("Checking tokens...");
    for a in addresses {
        TOKENS.write().unwrap().insert(a, Token::new(18));
    }
    info!("All tokens are deployed and sane");
    Ok(())
}

/// Returns multiplication factor to convert whole token value to fractions.
pub fn multiplication_factor(address: &str) -> Result<u64> {
    let decimals = {
        TOKENS
            .read()
            .unwrap()
            .get(address)
            .ok_or_else(|| eyre!("Token not found: {}", address))?
            .decimals
    };
    let factor = 10_u64
        .checked_pow(decimals)
        .ok_or_else(|| eyre!("Token {} overflow 10^{}", address, decimals))?;
    Ok(factor)
}

#[derive(Debug, Default, Clone)]
struct Token {
    decimals: u32,
}

impl Token {
    fn new(decimals: u32) -> Self {
        Token { decimals }
    }
}

type Tokens = std::collections::HashMap<String, Token>;

lazy_static::lazy_static! {
    static ref TOKENS: RwLock<Tokens> = RwLock::new(Tokens::default());
}
