use crate::account::{Operator, program, EthereumAccount, Treasury, Storage, Holder};
use crate::transaction::{ UnsignedTransaction, verify_tx_signature};
use crate::account_storage::ProgramAccountStorage;
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use crate::instruction::transaction::{Accounts, is_new_transaction, do_begin, do_continue};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Account");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from_le_bytes(*array_ref![instruction, 4, 8]);

    let holder = Holder::from_account_unchecked(program_id, &accounts[0])?;
    let (unsigned_msg, signature) = holder.transaction_and_signature();
    let caller = verify_tx_signature(&signature, &unsigned_msg)?;


    let storage_info = &accounts[1];

    let accounts = Accounts {
        operator: Operator::from_account(&accounts[2])?,
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[3])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[4])?,
        system_program: program::System::from_account(&accounts[5])?,
        neon_program: program::Neon::from_account(program_id, &accounts[6])?,
        remaining_accounts: &accounts[7..]
    };

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        accounts.remaining_accounts,
        crate::config::token_mint::id())?;


    if is_new_transaction(program_id, storage_info, &signature, &caller)? {
        let trx = UnsignedTransaction::from_rlp(&unsigned_msg)?;
        let storage = Storage::new(program_id, storage_info, &accounts, caller, &trx, &signature)?;

        do_begin(step_count, accounts, storage, &mut account_storage, trx, caller)
    } else {
        let storage = Storage::restore(program_id, storage_info, &accounts.operator, accounts.remaining_accounts)?;

        do_continue(step_count, accounts, storage, &mut account_storage)
    }
}
