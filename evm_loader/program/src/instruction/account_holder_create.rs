use crate::account::{Holder, Operator};
use crate::error::Result;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Create Holder Account");

    let holder = &accounts[0];
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    Holder::init(
        program_id,
        holder,
        crate::account::holder::Data {
            owner: *operator.key,
            transaction_hash: [0_u8; 32],
            transaction_len: 0,
        },
    )?;

    Ok(())
}
