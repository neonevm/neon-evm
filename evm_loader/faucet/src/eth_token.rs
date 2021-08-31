//! Faucet ETH token module.

use color_eyre::{eyre::eyre, Result};
use tracing::info;

use solana_client::rpc_client::RpcClient;

use evm_loader::token::token_mint;

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

    let address = solana::create_program_address(&params.wallet)?;
    let token_address =
        spl_associated_token_account::get_associated_token_address(&address, &token_mint::id());

    let client = RpcClient::new(config::solana_url());

    let r = client.get_token_account_balance(&token_address)?;

    //let r = client.send_and_confirm_transaction()?;

    dbg!(r);

    Ok(())
}
