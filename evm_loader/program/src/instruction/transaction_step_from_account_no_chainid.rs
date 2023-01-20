use crate::account::{Operator, program, EthereumAccount, Treasury};
use crate::config::GAS_LIMIT_MULTIPLIER_NO_CHAINID;
use crate::account_storage::ProgramAccountStorage;
use crate::error::Result;
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
};
use crate::instruction::transaction::Accounts;
use ethnum::U256;


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> Result<()> {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Account Without ChainId");

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

    let gas_multiplier = U256::from(GAS_LIMIT_MULTIPLIER_NO_CHAINID);
    super::transaction_step_from_account::execute(
        program_id, holder_or_storage_info, accounts, &mut account_storage, step_count, Some(gas_multiplier)
    )
}
