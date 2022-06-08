use crate::account::{FinalizedState, Operator};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

struct Accounts<'a> {
    deleted_account: &'a AccountInfo<'a>,
    operator: Operator<'a>,
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Delete Holder or Storage Account");

    let parsed_accounts = Accounts {
        deleted_account: &accounts[0],
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[1]) }?,
    };

    let seed = std::str::from_utf8(instruction)
        .map_err(|e| E!(ProgramError::InvalidInstructionData; "Seed decode error={:?}", e))?;


    validate(program_id, &parsed_accounts, seed)?;
    execute(parsed_accounts)
}

fn validate(program_id: &Pubkey, accounts: &Accounts, seed: &str) -> ProgramResult {
    let address = Pubkey::create_with_seed(accounts.operator.key, seed, program_id)?;
    if *accounts.deleted_account.key != address {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected key {}", accounts.deleted_account.key, address);
    }

    let tag = crate::account::tag(program_id, accounts.deleted_account)?;
    if !(tag == FinalizedState::TAG || tag == crate::account::TAG_EMPTY) {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected empty or finalized storage", accounts.deleted_account.key)
    }

    Ok(())
}

fn execute(accounts: Accounts) -> ProgramResult {
    let Accounts { deleted_account, operator } = accounts;
    unsafe { crate::account::delete(deleted_account, &operator) }
}
