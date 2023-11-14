use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::error::Result;

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Config Get Status");

    if cfg!(feature = "emergency") {
        solana_program::program::set_return_data(&[0]);
    } else {
        solana_program::program::set_return_data(&[1]);
    }

    Ok(())
}
