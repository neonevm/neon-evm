use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::{
    config::{NEON_PKG_VERSION, NEON_REVISION},
    error::Result,
};

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Config Get Version");

    let version = std::str::from_utf8(&NEON_PKG_VERSION)?;
    let revision = std::str::from_utf8(&NEON_REVISION)?;

    let return_data = bincode::serialize(&(version, revision))?;
    solana_program::program::set_return_data(&return_data);

    Ok(())
}
