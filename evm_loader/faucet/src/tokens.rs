//! Faucet tokens module.

use crate::ethereum;

use color_eyre::{eyre::eyre, Result};
use derive_new::new;
use tracing::{error, info};

use std::sync::RwLock;
use web3::api::Eth;
use web3::contract::{Contract, Options};
use web3::Transport;

/// Initializes local cache of tokens properties.
pub async fn init(addresses: Vec<String>) -> Result<()> {
    info!("Checking tokens...");
    use crate::config;

    let http = web3::transports::Http::new(&config::web3_rpc_url())?;
    let web3 = web3::Web3::new(http);

    for token_address in addresses {
        let a = ethereum::address_from_str(&token_address)?;
        TOKENS.write().unwrap().insert(
            token_address,
            Token::new(get_decimals(web3.eth(), a).await?),
        );
    }

    info!("All tokens are deployed and sane");
    Ok(())
}

async fn get_decimals<T: Transport>(
    eth: Eth<T>,
    token_address: ethereum::Address,
) -> web3::contract::Result<u32> {
    let token = Contract::from_json(
        eth,
        token_address,
        include_bytes!("../abi/UniswapV2ERC20.abi"),
    )
    .map_err(|e| {
        error!("Failed reading UniswapV2ERC20.abi: {}", e);
        e
    })?;

    let decimals = token
        .query("decimals", (), None, Options::default(), None)
        .await?;
    info!("ERC20 token {} has decimals {}", token_address, decimals);

    Ok(decimals)
}

/// Returns multiplication factor to convert whole token value to fractions.
pub fn multiplication_factor(token_address: &str) -> Result<u64> {
    let decimals = {
        TOKENS
            .read()
            .unwrap()
            .get(token_address)
            .ok_or_else(|| eyre!("Token not found: {}", token_address))?
            .decimals
    };
    let factor = 10_u64
        .checked_pow(decimals)
        .ok_or_else(|| eyre!("Token {} overflow 10^{}", token_address, decimals))?;
    Ok(factor)
}

#[derive(new, Debug, Default, Clone)]
struct Token {
    decimals: u32,
}

type Tokens = std::collections::HashMap<String, Token>;

lazy_static::lazy_static! {
    static ref TOKENS: RwLock<Tokens> = RwLock::new(Tokens::default());
}
