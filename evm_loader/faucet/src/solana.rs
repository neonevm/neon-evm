//! Faucet Solana ETH token module.

use color_eyre::Result;
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
    Ok(())
}
