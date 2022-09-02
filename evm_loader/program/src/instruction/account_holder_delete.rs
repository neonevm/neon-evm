use crate::account::{Operator, Holder};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Delete Holder Account");

    let holder = Holder::from_account(program_id, &accounts[0])?;
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    holder.validate_owner(&operator)?;

    unsafe { holder.suicide(&operator) }
}

