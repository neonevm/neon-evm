//! Faucet ETH token module.

use color_eyre::{eyre::eyre, Result};
use tracing::info;

use crate::{config, solana};

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

    let address = solana::make_program_address(&params.wallet)?;
    info!("Address: {}", &address);
    info!("Token mint id: {}", &evm_loader::token::token_mint::id());
    let token_address = spl_associated_token_account::get_associated_token_address(
        &address,
        &evm_loader::token::token_mint::id(),
    );
    info!("Token address: {}", &token_address);
    let owner = config::solana_eth_token_owner()?;
    solana::transfer_token(owner, token_address, params.amount)
}
