//! faucet server implementation.

#![allow(unreachable_code)]

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

use color_eyre::Report;
use ethers::prelude::*;
use rouille::{input, router, Request, Response};
use serde::Deserialize;
use tracing::{error, info};
use transaction::eip2718::TypedTransaction;

use crate::{config, contract};

pub type Account = SignerMiddleware<Provider<Http>, LocalWallet>;
pub type UniswapV2ERC20 = contract::UniswapV2ERC20<Account>;

/// Starts the server in listening mode.
#[allow(clippy::manual_strip)]
pub fn start(cfg: config::Faucet) {
    info!("Listening port {}...", cfg.rpc_port);
    let url = format!("localhost:{}", cfg.rpc_port);

    rouille::start_server(url, move |request| {
        router!(request,
            (POST) (/request_airdrop) => {
                handle(request, cfg.clone())
            },

            _ => Response::empty_404()
        )
    });
}

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, Deserialize)]
struct Airdrop {
    wallet: String,
    amount: u64,
}

const ALLOW_ORIGIN: &str = "*";
const ALLOW_METHODS: &str = "*";
const ALLOW_HEADERS: &str = "*";

/// Handles a request for airdrop.
fn handle(request: &Request, cfg: config::Faucet) -> Response {
    println!();
    info!("Handling {:?}...", request);

    let body = input::plain_text_body(request);
    if let Err(err) = body {
        error!("{}", err);
        return Response::text(format!("Error: {}", err))
            .with_additional_header("Access-Control-Allow-Origin", ALLOW_ORIGIN)
            .with_additional_header("Access-Control-Allow-Methods", ALLOW_METHODS)
            .with_additional_header("Access-Control-Allow-Headers", ALLOW_HEADERS);
    }

    let input = serde_json::from_str(&body.unwrap());
    if let Err(err) = input {
        error!("{}", err);
        return Response::text(format!("Error: {}", err))
            .with_additional_header("Access-Control-Allow-Origin", ALLOW_ORIGIN)
            .with_additional_header("Access-Control-Allow-Methods", ALLOW_METHODS)
            .with_additional_header("Access-Control-Allow-Headers", ALLOW_HEADERS);
    }

    let input: Airdrop = input.unwrap();
    info!("Requesting {:?}...", &input);

    let rt = tokio::runtime::Runtime::new();
    if let Err(err) = rt {
        error!("{}", err);
        return Response::text(format!("Error: {}", err))
            .with_additional_header("Access-Control-Allow-Origin", ALLOW_ORIGIN)
            .with_additional_header("Access-Control-Allow-Methods", ALLOW_METHODS)
            .with_additional_header("Access-Control-Allow-Headers", ALLOW_HEADERS);
    }

    if let Err(err) = rt.unwrap().block_on(process_airdrop(input, cfg)) {
        error!("{}", err);
        return Response::text(format!("Error: {}", err))
            .with_additional_header("Access-Control-Allow-Origin", ALLOW_ORIGIN)
            .with_additional_header("Access-Control-Allow-Methods", ALLOW_METHODS)
            .with_additional_header("Access-Control-Allow-Headers", ALLOW_HEADERS);
    }

    info!("OK");
    Response::text("OK")
        .with_additional_header("Access-Control-Allow-Origin", ALLOW_ORIGIN)
        .with_additional_header("Access-Control-Allow-Methods", ALLOW_METHODS)
        .with_additional_header("Access-Control-Allow-Headers", ALLOW_HEADERS)
}

type Amount = ethers::types::U256;
type Address = ethers::types::Address;

/// Processes the aridrop: sends needed transactions into Ethereum.
async fn process_airdrop(input: Airdrop, cfg: config::Faucet) -> Result<(), Report> {
    info!("Processing Airdrop...");

    let admin = address_from_str(&cfg.admin)?;
    let provider = Provider::<Http>::try_from(cfg.ethereum_endpoint.clone())?.with_sender(admin);
    let admin = Arc::new(import_account(provider.clone(), &cfg.admin_key)?);

    let token_a = address_from_str(&cfg.token_a)?;
    let token_a = UniswapV2ERC20::new(token_a, admin.clone());
    let token_b = address_from_str(&cfg.token_b)?;
    let token_b = UniswapV2ERC20::new(token_b, admin.clone());

    let amount = Amount::from(input.amount);

    info!("Depositing token A {}...", input.amount);
    let tx = airdrop(&token_a, &input.wallet, amount).await?;
    let tx = provider.send_transaction(tx, None).await?;
    let _receipt = tx.await?;
    //info!("{:?}", receipt);

    info!("Depositing token B {}...", input.amount);
    let tx = airdrop(&token_b, &input.wallet, amount).await?;
    let tx = provider.send_transaction(tx, None).await?;
    let _receipt = tx.await?;
    //info!("{:?}", receipt);

    Ok(())
}

/// Creates transaction to perform one airdrop operation.
async fn airdrop(
    token: &UniswapV2ERC20,
    recipient: &str,
    amount: Amount,
) -> Result<TypedTransaction, Report> {
    let recipient = address_from_str(recipient)?;
    let call = token.transfer(recipient, amount);
    Ok(call.tx)
}

/// Imports account from it's private key.
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
