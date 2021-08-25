//! Airdrop implementation: calls to the Ethereum network.

use color_eyre::Result;
use tracing::{error, info};

use secp256k1::SecretKey;
use web3::api::Eth;
use web3::contract::{Contract, Options};
use web3::signing::Key;
use web3::types::U256;
use web3::Transport;

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    wallet: String,
    amount: u64,
}

/// Processes the aridrop: sends needed transactions into Ethereum.
#[allow(unused)]
pub async fn process(airdrop: Airdrop) -> Result<()> {
    info!("Processing {:?}...", airdrop);
    use crate::{config, tokens};

    let http = web3::transports::Http::new(&config::ethereum_endpoint())?;
    let web3 = web3::Web3::new(http);

    let admin_key: SecretKey = config::admin_key().parse()?;
    let recipient = address_from_str(&airdrop.wallet)?;
    let amount = U256::from(airdrop.amount * tokens::multiplication_factor(&airdrop.wallet)?);

    for token in &config::tokens() {
        transfer(
            web3.eth(),
            address_from_str(token)?,
            token,
            &admin_key,
            recipient,
            amount,
        )
        .await
        .map_err(|e| {
            error!("Failed transfer of token {}: {}", token, e);
            e
        })?;
    }

    Ok(())
}

type Address = web3::types::Address;

/// Creates and sends a transfer transaction.
async fn transfer<T: Transport>(
    eth: Eth<T>,
    token: Address,
    token_name: &str,
    admin_key: impl Key + std::fmt::Debug,
    recipient: Address,
    amount: U256,
) -> Result<()> {
    info!(
        "Transfer {} of token {} -> {}",
        amount, token_name, recipient
    );
    let token = Contract::from_json(eth, token, include_bytes!("../abi/UniswapV2ERC20.abi"))
        .map_err(|e| {
            error!("Failed reading UniswapV2ERC20.abi: {}", e);
            e
        })?;

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
        .await
        .map_err(|e| {
            error!("Failed signed_call_with_confirmations: {}", e);
            e
        })?;

    info!("OK");
    Ok(())
}

/// Converts string representation of address to the H160 hash format.
fn address_from_str(s: &str) -> Result<Address> {
    use std::str::FromStr as _;
    let address = if !s.starts_with("0x") {
        Address::from_str(s)?
    } else {
        Address::from_str(&s[2..])?
    };
    Ok(address)
}

#[test]
fn test_address_from_str() {
    let r = address_from_str("ABC");
    assert!(r.is_err());
    assert_eq!(r.err().unwrap().to_string(), "Invalid input length");

    let r = address_from_str("ZYX");
    assert!(r.is_err());
    assert_eq!(
        r.err().unwrap().to_string(),
        "Invalid character 'Z' at position 0"
    );

    let r = address_from_str("0x00000000000000000000000000000000DeadBeef");
    assert!(r.is_ok());
}
