use crate::account::{Operator, program, EthereumAccount, Treasury, State, Holder, FinalizedState};
use crate::gasometer::Gasometer;
use crate::types::{Transaction};
use crate::account_storage::ProgramAccountStorage;
use crate::error::{Error, Result};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
};
use crate::instruction::transaction::{Accounts, do_begin, do_continue};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> Result<()> {
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
        remaining_accounts: &accounts[6..],
        all_accounts: accounts
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
            let caller = trx.recover_caller_address()?;

            solana_program::log::sol_log_data(&[b"HASH", &trx.hash]);

            let storage = State::new(program_id, storage_info, &accounts, caller, &trx)?;

            let mut gasometer = Gasometer::new(None, &accounts.operator)?;
            gasometer.record_solana_transaction_cost();
            gasometer.record_address_lookup_table(accounts.all_accounts);
            gasometer.record_iterative_overhead();

            do_begin(accounts, storage, &mut account_storage, gasometer, trx, caller)
        },
        State::TAG => {
            let (storage, _blocked_accounts) = State::restore(
                program_id,
                storage_info,
                &accounts.operator,
                accounts.remaining_accounts,
                false,
            )?;
            solana_program::log::sol_log_data(&[b"HASH", &storage.transaction_hash]);

            let mut gasometer = Gasometer::new(Some(storage.gas_used), &accounts.operator)?;
            gasometer.record_solana_transaction_cost();

            do_continue(step_count, accounts, storage, &mut account_storage, gasometer)
        },
        tag => Err(Error::AccountInvalidTag(*storage_info.key, tag, Holder::TAG))
    }
}
