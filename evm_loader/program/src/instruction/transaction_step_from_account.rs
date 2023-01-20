use crate::account::{Operator, program, EthereumAccount, Treasury, State, Holder, FinalizedState};
use crate::error::{Error, Result};
use crate::gasometer::Gasometer;
use crate::types::{Transaction};
use crate::account_storage::ProgramAccountStorage;
use arrayref::{array_ref};
use ethnum::U256;
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
};
use crate::instruction::transaction::{Accounts, do_begin, do_continue};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> Result<()> {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Account");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from(u32::from_le_bytes(*array_ref![instruction, 4, 4]));

    let holder_or_storage_info = &accounts[0];

    let accounts = Accounts {
        operator: Operator::from_account(&accounts[1])?,
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[2])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[3])?,
        system_program: program::System::from_account(&accounts[4])?,
        neon_program: program::Neon::from_account(program_id, &accounts[5])?,
        remaining_accounts: &accounts[6..],
        all_accounts: accounts,
    };

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        &accounts.operator,
        Some(&accounts.system_program),
        accounts.remaining_accounts,
    )?;

    execute(program_id, holder_or_storage_info, accounts, &mut account_storage, step_count, None)
}

pub fn execute<'a>(
    program_id: &'a Pubkey,
    holder_or_storage_info: &'a AccountInfo<'a>,
    accounts: Accounts<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    step_count: u64,
    gas_multiplier: Option<U256>,
) -> Result<()> {
    match crate::account::tag(program_id, holder_or_storage_info)? {
        Holder::TAG => {
            let trx = {
                let holder = Holder::from_account(program_id, holder_or_storage_info)?;
                holder.validate_owner(&accounts.operator)?;
                
                let message = holder.transaction();
                let trx = Transaction::from_rlp(&message)?;
                
                holder.validate_transaction(&trx)?;

                trx
            };

            solana_program::log::sol_log_data(&[b"HASH", &trx.hash]);

            let caller = trx.recover_caller_address()?;
            let mut storage = State::new(program_id, holder_or_storage_info, &accounts, caller, &trx)?;

            if let Some(gas_multiplier) = gas_multiplier {
                storage.gas_limit = storage.gas_limit.saturating_mul(gas_multiplier);
            }

            let mut gasometer = Gasometer::new(None, &accounts.operator)?;
            gasometer.record_solana_transaction_cost();
            gasometer.record_address_lookup_table(accounts.all_accounts);
            gasometer.record_iterative_overhead();
            gasometer.record_write_to_holder(&trx);

            do_begin(accounts, storage, account_storage, gasometer, trx, caller)
        }
        State::TAG => {
            let (storage, _blocked_accounts) = State::restore(
                program_id,
                holder_or_storage_info,
                &accounts.operator,
                accounts.remaining_accounts,
                false,
            )?;

            solana_program::log::sol_log_data(&[b"HASH", &storage.transaction_hash]);

            let mut gasometer = Gasometer::new(Some(storage.gas_used), &accounts.operator)?;
            gasometer.record_solana_transaction_cost();

            do_continue(step_count, accounts, storage, account_storage, gasometer)
        }
        FinalizedState::TAG => {
            Err(Error::StorageAccountFinalized)
        }
        tag => {
            Err(Error::AccountInvalidTag(*holder_or_storage_info.key, tag, Holder::TAG))
        }
    }
}