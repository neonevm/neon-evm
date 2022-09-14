use crate::account::{Operator, program, EthereumAccount, Treasury, State, Holder, FinalizedState};
use crate::transaction::{Transaction, recover_caller_address};
use crate::account_storage::ProgramAccountStorage;
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::instruction::transaction::{Accounts, do_begin, do_continue};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Instruction");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from(u32::from_le_bytes(*array_ref![instruction, 4, 4]));
    // skip let unique_index = u32::from_le_bytes(*array_ref![instruction, 8, 4]);
    let message = &instruction[4 + 4 + 4..];

    let storage_info = &accounts[0];

    let accounts = Accounts {
        operator: Operator::from_account(&accounts[1])?,
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[2])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[3])?,
        system_program: program::System::from_account(&accounts[4])?,
        neon_program: program::Neon::from_account(program_id, &accounts[5])?,
        remaining_accounts: &accounts[6..]
    };

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        &accounts.operator,
        Some(&accounts.system_program),
        accounts.remaining_accounts,
    )?;


    match crate::account::tag(program_id, storage_info)? {
        Holder::TAG | FinalizedState::TAG => {
            let trx = Transaction::from_rlp(message)?;
            let caller = recover_caller_address(&trx)?;

            solana_program::log::sol_log_data(&[b"HASH", &trx.hash]);

            let storage = State::new(program_id, storage_info, &accounts, caller, &trx)?;

            do_begin(step_count, accounts, storage, &mut account_storage, trx, caller, 0)
        },
        State::TAG => {
            let storage = State::restore(program_id, storage_info, &accounts.operator, accounts.remaining_accounts)?;
            solana_program::log::sol_log_data(&[b"HASH", &storage.transaction_hash]);

            do_continue(step_count, accounts, storage, &mut account_storage)
        },
        _ => Err!(ProgramError::InvalidAccountData; "Account {} - expected Holder or State", storage_info.key)
    }
}
