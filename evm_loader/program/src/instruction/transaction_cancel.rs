use crate::account::{Operator, Storage, Incinerator};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::account_storage::ProgramAccountStorage;
use crate::storage_account::Deposit;
use crate::config::token_mint;

struct Accounts<'a> {
    storage: Storage<'a>,
    // operator: Operator<'a>,
    incinerator: Incinerator<'a>,
    remaining_accounts: &'a [AccountInfo<'a>],
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Cancel Transaction");

    let storage_info = &accounts[0];
    let operator = Operator::from_account(&accounts[1])?;
    let incinerator = Incinerator::from_account(&accounts[2])?;
    let remaining_accounts = &accounts[3..];

    let storage = Storage::restore(program_id, storage_info, &operator, remaining_accounts)?;

    let accounts = Accounts { storage, incinerator, remaining_accounts };
    let nonce = u64::from_le_bytes(*array_ref![instruction, 0, 8]);

    validate(&accounts, nonce)?;
    execute(program_id, accounts)
}

fn validate(accounts: &Accounts, nonce: u64) -> ProgramResult {
    let storage = &accounts.storage;

    if storage.nonce != nonce {
        return Err!(ProgramError::InvalidInstructionData; "trx_nonce<{}> != nonce<{}>", storage.nonce, nonce);
    }

    Ok(())
}

fn execute<'a>(program_id: &'a Pubkey, accounts: Accounts<'a>) -> ProgramResult {
    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        accounts.remaining_accounts,
        &token_mint::id())?;
    let caller_account = account_storage.ethereum_account_mut(&accounts.storage.caller)
        .expect("Caller account present in the transaction");

    caller_account.trx_count += 1;

    account_storage.block_accounts(false)?;
    accounts.storage.finalize(Deposit::Burn(accounts.incinerator))?;

    Ok(())
}
