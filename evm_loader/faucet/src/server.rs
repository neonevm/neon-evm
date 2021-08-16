//! faucet server implementation.

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

use actix_web::web::{post, Bytes};
use actix_web::{App, HttpResponse, HttpServer, Responder};
use color_eyre::Report;
use serde::Deserialize;
use tracing::{error, info};

use ethers::prelude::{
    transaction::eip2718::TypedTransaction, Http, LocalWallet, Middleware, Provider, Signer,
    SignerMiddleware,
};

use crate::{config, contract};

/// Represents a signer account.
pub type Account = SignerMiddleware<Provider<Http>, LocalWallet>;

/// Starts the server in listening mode.
pub async fn start(rpc_port: u16) -> Result<(), Report> {
    HttpServer::new(|| App::new().route("/request_airdrop", post().to(handle_request_airdrop)))
        .bind(("localhost", rpc_port))?
        .run()
        .await?;
    Ok(())
}

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, Deserialize)]
struct Airdrop {
    wallet: String,
    amount: u64,
}

/// Handles a request for airdrop.
async fn handle_request_airdrop(body: Bytes) -> impl Responder {
    println!();
    info!("Handling Request for Airdrop...");

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    let airdrop = serde_json::from_str::<Airdrop>(&input.unwrap());
    if let Err(err) = airdrop {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = process_airdrop(airdrop.unwrap()).await {
        error!("InternalServerError: {}", err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}

type Amount = ethers::types::U256;
type Address = ethers::types::Address;
type UniswapV2ERC20 = contract::UniswapV2ERC20<Account>;

/// Processes the aridrop: sends needed transactions into Ethereum.
async fn process_airdrop(airdrop: Airdrop) -> Result<(), Report> {
    info!("Processing {:?}...", airdrop);

    let admin = derive_address(&config::admin_key())?;
    let provider = Provider::<Http>::try_from(config::ethereum_endpoint())?.with_sender(admin);
    let admin = Arc::new(import_account(provider.clone(), &config::admin_key())?);

    let token_a = address_from_str(&config::token_a())?;
    let token_a = UniswapV2ERC20::new(token_a, admin.clone());
    let token_b = address_from_str(&config::token_b())?;
    let token_b = UniswapV2ERC20::new(token_b, admin.clone());

    let recipient = &airdrop.wallet;
    let amount = Amount::from(airdrop.amount);

    info!("Depositing {} -> token A...", amount);
    let tx = create_transfer_tx(&token_a, recipient, amount).await?;
    info!("Sending transaction for transfer of token A...");
    let tx = provider.send_transaction(tx, None).await?;
    info!("Waiting transaction for transfer of token A...");
    let _receipt = tx.await?;
    //info!("{:?}", receipt);
    info!("OK");

    info!("Depositing {} -> token B...", amount);
    let tx = create_transfer_tx(&token_b, recipient, amount).await?;
    info!("Sending transaction for transfer of token B...");
    let tx = provider.send_transaction(tx, None).await?;
    info!("Waiting transaction for transfer of token B...");
    let _receipt = tx.await?;
    //info!("{:?}", receipt);
    info!("OK");

    Ok(())
}

/// Creates transaction to perform one airdrop operation.
async fn create_transfer_tx(
    token: &UniswapV2ERC20,
    recipient: &str,
    amount: Amount,
) -> Result<TypedTransaction, Report> {
    let recipient = address_from_str(recipient)?;
    let call = token.transfer(recipient, amount);
    Ok(call.tx)
}

/// Calculates address of given private key.
fn derive_address(priv_key: &str) -> Result<Address, Report> {
    let wallet = priv_key.parse::<LocalWallet>()?;
    Ok(wallet.address())
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
