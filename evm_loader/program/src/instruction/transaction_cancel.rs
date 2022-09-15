use crate::account::{Operator, State, Incinerator};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::account_storage::ProgramAccountStorage;
use crate::state_account::Deposit;

struct Accounts<'a> {
    storage: State<'a>,
    operator: Operator<'a>,
    incinerator: Incinerator<'a>,
    remaining_accounts: &'a [AccountInfo<'a>],
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Cancel Transaction");

    let storage_info = &accounts[0];
    let operator = Operator::from_account(&accounts[1])?;
    let incinerator = Incinerator::from_account(&accounts[2])?;
    let remaining_accounts = &accounts[3..];

    let storage = State::restore(program_id, storage_info, &operator, remaining_accounts)?;

    let accounts = Accounts { storage, operator, incinerator, remaining_accounts };
    let transaction_hash = array_ref![instruction, 0, 32];

    solana_program::log::sol_log_data(&[b"HASH", transaction_hash]);

    validate(&accounts, transaction_hash)?;
    execute(program_id, accounts)
}

fn validate(accounts: &Accounts, transaction_hash: &[u8; 32]) -> ProgramResult {
    let storage = &accounts.storage;

    if &storage.transaction_hash != transaction_hash {
        return Err!(ProgramError::InvalidInstructionData; "Invalid transaction hash");
    }

    Ok(())
}

fn execute<'a>(program_id: &'a Pubkey, accounts: Accounts<'a>) -> ProgramResult {
    let used_gas = accounts.storage.gas_used;
    solana_program::log::sol_log_data(&[b"CL_TX_GAS", used_gas.as_u64().to_le_bytes().as_slice()]);

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        &accounts.operator,
        None,
        accounts.remaining_accounts,
    )?;
    let caller_account = account_storage.ethereum_account_mut(&accounts.storage.caller);
    caller_account.trx_count += 1;

    account_storage.block_accounts(false)?;
    accounts.storage.finalize(Deposit::Burn(accounts.incinerator))?;

    Ok(())
}
