use crate::account::{Holder, Operator};
use crate::error::Result;
use arrayref::array_ref;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Create Holder Account");

    let holder = accounts[0].clone();
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    let seed_len = usize::from_le_bytes(*array_ref![instruction, 0, 8]);
    let seed_bytes = instruction[8..8 + seed_len].to_vec();
    let seed = std::str::from_utf8(&seed_bytes)?;

    Holder::create(program_id, holder, seed, &operator)?;

    Ok(())
}
