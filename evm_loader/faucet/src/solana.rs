//! Faucet Solana utilities module.

use std::str::FromStr;

use color_eyre::Result;

use solana_sdk::pubkey::Pubkey;

use crate::{config, ethereum};

/// Generates a Solana address by corresponding Ethereum address.
pub fn create_program_address(seed: &str) -> Result<Pubkey> {
    let seed = hex::decode(ethereum::strip_0x_prefix(seed))?;
    let seeds = vec![&seed[..]];
    let evm_loader_id = Pubkey::from_str(&config::solana_evm_loader())?;
    let (address, _nonce) = Pubkey::find_program_address(&seeds, &evm_loader_id);
    Ok(address)
}
