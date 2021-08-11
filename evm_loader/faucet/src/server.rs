//! faucet server implementation.

#![allow(unreachable_code)]

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

use color_eyre::Report;
use ethers::prelude::*;
use rouille::{input, router, try_or_400, Request, Response};
use serde::Deserialize;
use tracing::{error, info};

use crate::{config, contract};

type Amount = ethers::types::U256;
type Address = ethers::types::Address;
pub type Account = SignerMiddleware<Provider<Http>, LocalWallet>;
pub type UniswapV2ERC20 = contract::UniswapV2ERC20<Account>;

/// Starts the server in listening mode.
#[allow(clippy::manual_strip)]
pub async fn run(cfg: config::Faucet) {
    let url = format!("localhost:{}", cfg.rpc_port);
    info!("Listening port {}...", cfg.rpc_port);

    rouille::start_server(url, move |request| {
        router!(request,
            (POST) (/request_airdrop) => {
                handle(request, cfg.clone())
            },

            _ => Response::empty_404()
        )
    });
}

#[derive(Debug, Deserialize)]
struct Airdrop {
    wallet: String,
    amount: u64,
}

/// Handles a request for airdrop.
fn handle(request: &Request, cfg: config::Faucet) -> Response {
    info!("Handling request...");

    let input: Airdrop = try_or_400!(input::json_input(request));
    info!("Requesting {:?}...", &input);
    if let Err(err) = process_airdrop(input, cfg) {
        error!("{}", err);
        return Response::text(format!("Error: {}", err));
    }

    info!("OK");
    Response::text("OK")
}

/// Processes the aridrop: sends needed transactions into Ethereum.
fn process_airdrop(input: Airdrop, cfg: config::Faucet) -> Result<(), Report> {
    info!("Processing airdrop...");

    let provider = Provider::<Http>::try_from(cfg.ethereum_endpoint.clone())?;
    let admin = Arc::new(import_account(provider, &cfg.admin)?);

    let token_a = address_from_str(&cfg.token_a)?;
    let token_a = UniswapV2ERC20::new(token_a, admin.clone());
    airdrop(&token_a, &input.wallet, Amount::from(input.amount))?;

    let token_b = address_from_str(&cfg.token_b)?;
    let token_b = UniswapV2ERC20::new(token_b, admin);
    airdrop(&token_b, &input.wallet, Amount::from(input.amount))?;

    Ok(())
}

/// Sends transaction to perform one airdrop operation.
fn airdrop(token: &UniswapV2ERC20, recipient: &str, amount: Amount) -> Result<(), Report> {
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
