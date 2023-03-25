use crate::account::{FinalizedState, Holder, Operator};
use crate::error::{Error, Result};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Delete Holder Account");

    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    match crate::account::tag(program_id, &accounts[0])? {
        Holder::TAG => {
            let holder = Holder::from_account(program_id, &accounts[0])?;
            holder.validate_owner(&operator)?;

            unsafe {
                holder.suicide(&operator);
            }
        }
        FinalizedState::TAG => {
            let finalized = FinalizedState::from_account(program_id, &accounts[0])?;
            if &finalized.owner != operator.key {
                return Err(Error::HolderInvalidOwner(finalized.owner, *operator.key));
            }

            unsafe {
                finalized.suicide(&operator);
            }
        }
        _ => {
            return Err(Error::AccountInvalidTag(*accounts[0].key, Holder::TAG));
        }
    }

    Ok(())
}
