use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::error::Result;

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Config Get Chain Info");

    let bytes = instruction.try_into()?;
    let index = usize::from_le_bytes(bytes);
    let info = &crate::config::CHAIN_ID_LIST[index];

    let return_data = bincode::serialize(info)?;
    solana_program::program::set_return_data(&return_data);

    Ok(())
}
