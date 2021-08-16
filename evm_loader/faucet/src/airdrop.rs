//! Airdrop implementation: calls to the Ethereum network.

use color_eyre::Report;
use tracing::info;

use web3::contract::{Contract, Options};
use web3::types::U256;

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

    let admin_key = config::admin_key().parse()?;
    let recipient = airdrop.wallet;
    let amount = U256::from(airdrop.amount);

    info!("Transfer {} -> token A", airdrop.amount);
    let token_a = address_from_str(&config::token_a())?;
    let token_a = Contract::from_json(
        web3.eth(),
        token_a,
        include_bytes!("../abi/UniswapV2ERC20.abi"),
    )?;

    info!("Sending transaction for transfer of token A...");
    token_a
        .signed_call_with_confirmations(
            "transfer",
            (recipient.clone(), amount),
            Options::default(),
            0,
            &admin_key,
        )
        .await?;

    info!("OK");

    info!("Transfer {} -> token B", airdrop.amount);
    let token_b = address_from_str(&config::token_b())?;
    let token_b = Contract::from_json(
        web3.eth(),
        token_b,
        include_bytes!("../abi/UniswapV2ERC20.abi"),
    )?;

    info!("Sending transaction for transfer of token B...");
    token_b
        .signed_call_with_confirmations(
            "transfer",
            (recipient.clone(), amount),
            Options::default(),
            0,
            &admin_key,
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
