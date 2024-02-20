use crate::account::{Holder, Operator};
use crate::error::Result;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Delete Holder Account");

    let holder_info = accounts[0].clone();
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    crate::account::legacy::update_holder_account(&holder_info)?;

    let holder = Holder::from_account(program_id, holder_info)?;
    holder.validate_owner(&operator)?;
    unsafe {
        holder.suicide(&operator);
    }

    Ok(())
}
