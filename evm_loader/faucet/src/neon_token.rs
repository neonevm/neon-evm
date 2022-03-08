//! Faucet NEON token module.

use eyre::{eyre, Result};
use tracing::info;

use crate::{config, ethereum, id::ReqId, solana};

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    /// Ethereum address of the recipient.
    wallet: String,
    /// Amount of a token to be received.
    amount: u64,
    /// Specifies amount in whole tokens (false, default) or in 10E-9 fractions (true).
    #[serde(default)]
    pub in_fractions: bool,
}

/// Processes the airdrop: sends needed transactions into Solana.
pub async fn airdrop(id: &ReqId, params: Airdrop) -> Result<()> {
    info!("{} Processing NEON {:?}...", id, params);

    if config::solana_account_seed_version() == 0 {
        // not yet initialized
        if !solana::is_alive().await {
            return Err(eyre!("Solana does not respond"));
        }
        config::load_neon_params(solana::get_client()).await?;
    }

    let limit = if !params.in_fractions {
        config::solana_max_amount()
    } else {
        solana::convert_whole_to_fractions(config::solana_max_amount())?
    };

    if params.amount > limit {
        return Err(eyre!(
            "Requested value {} exceeds the limit {}",
            params.amount,
            limit
        ));
    }

    let operator = config::solana_operator_keypair()
        .map_err(|e| eyre!("config::solana_operator_keypair: {:?}", e))?;
    let ether_address = ethereum::address_from_str(&params.wallet)
        .map_err(|e| eyre!("ethereum::address_from_str({}): {:?}", &params.wallet, e))?;
    solana::deposit_token(
        id,
        operator,
        ether_address,
        params.amount,
        params.in_fractions,
    )
    .await
    .map_err(|e| {
        eyre!(
            "solana::deposit_token(operator, {}): {:?}",
            ether_address,
            e
        )
    })?;
    Ok(())
}
