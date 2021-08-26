//! Airdrop implementation: calls to the Ethereum network.

use crate::ethereum;

use color_eyre::{eyre::eyre, Result};
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

    let admin_key: SecretKey = config::web3_private_key().parse()?;
    let http = web3::transports::Http::new(&config::web3_rpc_url())?;
    let web3 = web3::Web3::new(http);

    let recipient = ethereum::address_from_str(&airdrop.wallet)?;
    let amount = U256::from(airdrop.amount);

    for token in &config::tokens() {
        let factor = U256::from(tokens::multiplication_factor(token)?);
        let internal_amount = amount
            .checked_mul(factor)
            .ok_or_else(|| eyre!("Overflow {} * {}", amount, factor))?;
        transfer(
            web3.eth(),
            ethereum::address_from_str(token)?,
            token,
            &admin_key,
            recipient,
            internal_amount,
        )
        .await
        .map_err(|e| {
            error!("Failed transfer of token {}: {}", token, e);
            e
        })?;
    }

    Ok(())
}

/// Creates and sends a transfer transaction.
async fn transfer<T: Transport>(
    eth: Eth<T>,
    token: ethereum::Address,
    token_name: &str,
    admin_key: impl Key + std::fmt::Debug,
    recipient: ethereum::Address,
    amount: U256,
) -> web3::contract::Result<()> {
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
