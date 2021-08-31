//! Faucet ETH token module.

use color_eyre::{eyre::eyre, Result};
use tracing::info;

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    wallet: String,
    amount: u64,
}

/// Processes the airdrop: sends needed transactions into Solana.
pub async fn airdrop(params: Airdrop) -> Result<()> {
    info!("Processing ETH {:?}...", params);
    use crate::config;

    if params.amount > config::solana_max_amount() {
        return Err(eyre!(
            "Requested value {} exceeds the limit {}",
            params.amount,
            config::solana_max_amount()
        ));
    }

    Ok(())
}
