use crate::account::{Holder, Operator};
use arrayref::array_ref;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> ProgramResult {
    solana_program::msg!("Instruction: Create Holder Account");

    let holder = &accounts[0];
    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?;

    let seed_len = usize::from_le_bytes(*array_ref![instruction, 0, 8]);
    let seed_bytes = instruction[8..8 + seed_len].to_vec();
    let seed = String::from_utf8(seed_bytes)
        .map_err(|_| E!(ProgramError::InvalidArgument; "Seed bytes aren't valid UTF8"))?;

    let expected_holder = Pubkey::create_with_seed(operator.key, &seed, program_id)
        .map_err(|_| E!(ProgramError::InvalidArgument; "Invalid seed bytes"))?;
    if expected_holder != *holder.key {
        return Err!(ProgramError::InvalidArgument; "Holder doesn't match seeds");
    }

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
