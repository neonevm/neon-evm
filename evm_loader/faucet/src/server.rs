//! faucet server implementation.

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

use color_eyre::Report;
use ethers::prelude::*;

use crate::{config, contract};

type Amount = ethers::types::U256;
type Address = ethers::types::Address;
pub type Account = SignerMiddleware<Provider<Http>, LocalWallet>;
pub type UniswapV2ERC20 = contract::UniswapV2ERC20<Account>;

/// Starts the server in listening mode.
pub async fn run(cfg: &config::Faucet) -> Result<(), Report> {
    let provider = Provider::<Http>::try_from(cfg.ethereum_endpoint.clone())?;
    let admin = Arc::new(import_account(provider, &cfg.admin)?);

    let recipient = "0xAAA";
    let amount = Amount::from(100);

    let token_a = address_from_str(&cfg.token_a)?;
    let token_a = UniswapV2ERC20::new(token_a, admin.clone());
    airdrop(&token_a, recipient, amount).await?;

    let token_b = address_from_str(&cfg.token_b)?;
    let token_b = UniswapV2ERC20::new(token_b, admin);
    airdrop(&token_b, recipient, amount).await?;

    Ok(())
}

/// Sends transaction to perform one airdrop operation.
async fn airdrop(token: &UniswapV2ERC20, recipient: &str, amount: Amount) -> Result<(), Report> {
    let recipient = address_from_str(recipient)?;
    let call = token.transfer(recipient, amount);
    dbg!(&call);
    Ok(())
}

/// Imports account by it's private key.
fn import_account(provider: Provider<Http>, priv_key: &str) -> Result<Account, Report> {
    let wallet = priv_key.parse::<LocalWallet>()?;
    let account = SignerMiddleware::new(provider, wallet);
    Ok(account)
}

/// Converts string representation of address to the H160 hash format.
fn address_from_str(s: &str) -> Result<Address, Report> {
    let address = if !s.starts_with("0x") {
        Address::from_str(s)?
    } else {
        Address::from_str(&s[2..])?
    };
    Ok(address)
}
