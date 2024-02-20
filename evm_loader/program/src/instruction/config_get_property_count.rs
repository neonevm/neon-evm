use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::error::Result;

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Config Get Property Count");

    let count = crate::config::PARAMETERS.len();

    let return_data = count.to_le_bytes();
    solana_program::program::set_return_data(&return_data);

    Ok(())
}
