//! Airdrop implementation: calls to the Ethereum network.

use color_eyre::Report;
use tracing::info;

use secp256k1::SecretKey;
use web3::api::Eth;
use web3::contract::{Contract, Options};
use web3::signing::Key;
use web3::types::U256;
use web3::Transport;

use crate::config;

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    wallet: String,
    amount: u64,
}

type Address = web3::types::Address;

/// Processes the aridrop: sends needed transactions into Ethereum.
pub async fn process(airdrop: Airdrop) -> Result<(), Report> {
    info!("Processing {:?}...", airdrop);

    let http = web3::transports::Http::new(&config::ethereum_endpoint())?;
    let web3 = web3::Web3::new(http);

    let admin_key: SecretKey = config::admin_key().parse()?;
    let recipient = address_from_str(&airdrop.wallet)?;
    let amount = U256::from(airdrop.amount);

    transfer(
        web3.eth(),
        address_from_str(&config::token_a())?,
        "A",
        &admin_key,
        recipient,
        amount,
    )
    .await?;
    transfer(
        web3.eth(),
        address_from_str(&config::token_b())?,
        "B",
        &admin_key,
        recipient,
        amount,
    )
    .await?;

    Ok(())
}

/// Creates and sends a transfer transaction.
async fn transfer<T: Transport>(
    eth: Eth<T>,
    token: Address,
    token_name: &str,
    admin_key: impl Key,
    recipient: Address,
    amount: U256,
) -> Result<(), Report> {
    info!("Transfer {} -> token {}", amount, token_name);
    let token = Contract::from_json(eth, token, include_bytes!("../abi/UniswapV2ERC20.abi"))?;

    info!(
        "Sending transaction for transfer of token {}...",
        token_name
    );
    token
        .signed_call_with_confirmations(
            "transfer",
            (recipient, amount),
            Options::default(),
            0, // confirmations
            admin_key,
        )
        .await?;

    info!("OK");
    Ok(())
}

/// Converts string representation of address to the H160 hash format.
fn address_from_str(s: &str) -> Result<Address, Report> {
    use std::str::FromStr as _;
    let address = if !s.starts_with("0x") {
        Address::from_str(s)?
    } else {
        Address::from_str(&s[2..])?
    };
    Ok(address)
}
