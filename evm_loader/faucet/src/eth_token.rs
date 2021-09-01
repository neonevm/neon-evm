//! Faucet ETH token module.

use color_eyre::{eyre::eyre, Result};
use tracing::info;

use solana_sdk::signature::Signer as _;

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

    let eth_acc = params.wallet.clone();
    let account = solana::make_program_address(&eth_acc)?;
    let token_account = spl_associated_token_account::get_associated_token_address(
        &account,
        &evm_loader::token::token_mint::id(),
    );
    let operator = config::solana_operator_keypair()?;
    let token_owner = config::solana_eth_token_owner_keypair()?;
    solana::transfer_token(
        operator,
        token_owner.pubkey(),
        account,
        token_account,
        params.amount,
    )
}
