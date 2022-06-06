use crate::account::{Operator, program, EthereumAccount, Treasury, State};
use crate::account_storage::ProgramAccountStorage;
use crate::instruction::transaction::Accounts;
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use crate::config::chain_id;



pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Continue Transaction");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from_le_bytes(*array_ref![instruction, 4, 8]);


    let storage_info = &accounts[0];

    let accounts = Accounts {
        operator: Operator::from_account(&accounts[1])?,
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[2])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[3])?,
        system_program: program::System::from_account(&accounts[4])?,
        neon_program: program::Neon::from_account(program_id, &accounts[5])?,
        remaining_accounts: &accounts[6..]
    };


    let storage = State::restore(program_id, storage_info, &accounts.operator, accounts.remaining_accounts)?;
    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        accounts.remaining_accounts,
        crate::config::token_mint::id(),
        chain_id().as_u64(),
    )?;

    super::transaction::do_continue(step_count, accounts, storage, &mut account_storage)
}
