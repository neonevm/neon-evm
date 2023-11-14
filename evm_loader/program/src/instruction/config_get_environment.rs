use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::error::Result;

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Config Get Environment");

    let environment: &str = if cfg!(feature = "mainnet") {
        "mainnet"
    } else if cfg!(feature = "testnet") {
        "testnet"
    } else if cfg!(feature = "devnet") {
        "devnet"
    } else if cfg!(feature = "govertest") {
        "govertest"
    } else if cfg!(feature = "ci") {
        "ci"
    } else {
        "unknown"
    };

    solana_program::program::set_return_data(environment.as_bytes());

    Ok(())
}
