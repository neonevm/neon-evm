use crate::account::{EthereumAccount, Incinerator, Operator, State};
use crate::state_account::{BlockedAccounts, Deposit};
use arrayref::array_ref;
use ethnum::U256;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

struct Accounts<'a> {
    storage: State<'a>,
    incinerator: Incinerator<'a>,
    remaining_accounts: &'a [AccountInfo<'a>],
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> ProgramResult {
    solana_program::msg!("Instruction: Cancel Transaction");

    let storage_info = &accounts[0];
    let operator = Operator::from_account(&accounts[1])?;
    let incinerator = Incinerator::from_account(&accounts[2])?;
    let remaining_accounts = &accounts[3..];

    let (storage, blocked_accounts) = State::restore(
        program_id,
        storage_info,
        &operator,
        remaining_accounts,
        true,
    )?;

    let accounts = Accounts {
        storage,
        incinerator,
        remaining_accounts,
    };
    let transaction_hash = array_ref![instruction, 0, 32];

    solana_program::log::sol_log_data(&[b"HASH", transaction_hash]);

    validate(&accounts, transaction_hash)?;
    execute(program_id, accounts, &blocked_accounts)
}

fn validate(accounts: &Accounts, transaction_hash: &[u8; 32]) -> ProgramResult {
    let storage = &accounts.storage;

    if &storage.transaction_hash != transaction_hash {
        return Err!(ProgramError::InvalidInstructionData; "Invalid transaction hash");
    }

    Ok(())
}

fn execute<'a>(
    program_id: &'a Pubkey,
    accounts: Accounts<'a>,
    blocked_accounts: &BlockedAccounts,
) -> ProgramResult {
    let used_gas = U256::ZERO;
    let total_used_gas = accounts.storage.gas_used;
    solana_program::log::sol_log_data(&[
        b"GAS",
        &used_gas.to_le_bytes(),
        &total_used_gas.to_le_bytes(),
    ]);

    for (info, blocked) in accounts.remaining_accounts.iter().zip(blocked_accounts) {
        if !blocked.exists {
            continue;
        }

        if let Ok(mut ether_account) = EthereumAccount::from_account(program_id, info) {
            ether_account.rw_blocked = false;
            if ether_account.address == accounts.storage.caller {
                ether_account.trx_count += 1;
            }
        }
    }

    accounts
        .storage
        .finalize(Deposit::Burn(accounts.incinerator))?;

    Ok(())
}
