use crate::account::{Holder, Operator};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

pub fn process<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> ProgramResult {
    solana_program::msg!("Instruction: Create Holder Account");

    let holder = &accounts[0];
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    Holder::init(
        holder,
        crate::account::holder::Data {
            owner: *operator.key,
            transaction_hash: [0_u8; 32],
        },
    )?;

    Ok(())
}
