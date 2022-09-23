use crate::account::{Operator, program, EthereumAccount, Treasury, State, Holder, FinalizedState};
use crate::error::EvmLoaderError;
use crate::transaction::{ Transaction, recover_caller_address};
use crate::account_storage::ProgramAccountStorage;
use arrayref::{array_ref};
use evm::U256;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use crate::instruction::transaction::{Accounts, do_begin, do_continue, alt_cost};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Account");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from(u32::from_le_bytes(*array_ref![instruction, 4, 4]));
    let alt_gas_used = alt_cost(accounts.len() as u64);

    let holder_or_storage_info = &accounts[0];

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

    execute(program_id, holder_or_storage_info, accounts, &mut account_storage, step_count, None, alt_gas_used)
}

pub fn execute<'a>(
    program_id: &'a Pubkey,
    holder_or_storage_info: &'a AccountInfo<'a>,
    accounts: Accounts<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    step_count: u64,
    gas_multiplier: Option<U256>,
    alt_cost: u64,
) -> ProgramResult {
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

            let caller = recover_caller_address(&trx)?;
            let mut storage = State::new(program_id, holder_or_storage_info, &accounts, caller, &trx)?;

            if let Some(gas_multiplier) = gas_multiplier {
                storage.gas_limit = storage.gas_limit.saturating_mul(gas_multiplier);
            }

            do_begin(step_count, accounts, storage, account_storage, trx, caller, alt_cost)
        }
        State::TAG => {
            let storage = State::restore(program_id, holder_or_storage_info, &accounts.operator, accounts.remaining_accounts)?;

            solana_program::log::sol_log_data(&[b"HASH", &storage.transaction_hash]);

            do_continue(step_count, accounts, storage, account_storage)
        }
        FinalizedState::TAG => {
            Err!(EvmLoaderError::StorageAccountFinalized.into(); "Transaction already finalized")
        }
        _ => {
            Err!(ProgramError::InvalidAccountData; "Account {} - expected Holder or State", holder_or_storage_info.key)
        }
    }
}