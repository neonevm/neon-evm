use crate::account::{Holder, Operator};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use std::convert::TryFrom;

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Write To Holder");

    let holder_id = u64::from_le_bytes(*array_ref![instruction, 0, 8]);
    let offset = u32::from_le_bytes(*array_ref![instruction, 8, 4]);
    let data_len = u64::from_le_bytes(*array_ref![instruction, 8 + 4, 8]);
    let data_len = usize::try_from(data_len).expect("usize is 8 bytes");

    let data_begin: usize = 8 + 4 + 8;
    let data_end: usize = data_begin + data_len;
    let data = &instruction[data_begin..data_end];

    let operator = Operator::from_account(&accounts[1])?;
    let mut holder = Holder::from_account(program_id, holder_id, &accounts[0], &operator)?;

    holder.write(offset, data)
}
