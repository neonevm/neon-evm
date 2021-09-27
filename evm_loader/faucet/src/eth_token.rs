//! Faucet ETH token module.

use color_eyre::eyre::eyre;
use color_eyre::Result;
use tracing::info;

use crate::{config, ethereum, solana};

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    /// Ethereum address of the recipient.
    wallet: String,
    /// Amount of a token to be received.
    amount: u64,
}

/// Processes the airdrop: sends needed transactions into Solana.
pub async fn airdrop(params: Airdrop) -> Result<()> {
    info!("Processing ETH {:?}...", params);

    if params.amount > config::solana_max_amount() {
        return Err(eyre!(
            "Requested value {} exceeds the limit {}",
            params.amount,
            config::solana_max_amount()
        ));
    }

    let operator = config::solana_operator_keypair()
        .map_err(|e| eyre!("config::solana_operator_keypair: {:?}", e))?;
    let ether_address = ethereum::address_from_str(&params.wallet)
        .map_err(|e| eyre!("ethereum::address_from_str({}): {:?}", &params.wallet, e))?;
    solana::transfer_token(operator, ether_address, params.amount)
        .await
        .map_err(|e| {
            eyre!(
                "solana::transfer_token(operator, {}): {:?}",
                ether_address,
                e
            )
        })?;
    Ok(())
}
