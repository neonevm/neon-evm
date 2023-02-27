use crate::account::{FinalizedState, Holder, Operator};
use crate::error::{Error, Result};
use arrayref::array_ref;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Write To Holder");

    let transaction_hash = *array_ref![instruction, 0, 32];
    let offset = usize::from_le_bytes(*array_ref![instruction, 32, 8]);
    let data = &instruction[32 + 8..];

    let holder_info = &accounts[0];

    let mut holder = match crate::account::tag(program_id, holder_info)? {
        Holder::TAG => Holder::from_account(program_id, holder_info),
        FinalizedState::TAG => {
            let finalized_state = FinalizedState::from_account(program_id, holder_info)?;
            let holder_data = crate::account::holder::Data {
                owner: finalized_state.owner,
                transaction_hash,
            };
            unsafe { finalized_state.replace(holder_data) }
        }
        tag => {
            return Err(Error::AccountInvalidTag(*holder_info.key, tag, Holder::TAG));
        }
    }?;

    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    holder.validate_owner(&operator)?;

    if holder.transaction_hash != transaction_hash {
        holder.clear()?;
        holder.transaction_hash = transaction_hash;
    }

    solana_program::log::sol_log_data(&[b"HASH", &transaction_hash]);

    holder.write(offset, data)?;

    Ok(())
}
