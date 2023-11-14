use arrayref::array_ref;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::account::{program, AccountsDB, BalanceAccount, Operator};
use crate::config::DEFAULT_CHAIN_ID;
use crate::error::Result;
use crate::types::Address;

pub fn process<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Create Balance Account");

    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[0]) }?;
    let system = program::System::from_account(&accounts[1])?;

    let accounts_db = AccountsDB::new(&accounts[2..], operator, None, Some(system), None);

    let address = array_ref![instruction, 0, 20];
    let address = Address::from(*address);

    let chain_id = array_ref![instruction, 20, 8];
    let chain_id = u64::from_le_bytes(*chain_id);

    // TODO: validate chain_id?

    solana_program::msg!("Address: {}, ChainID: {}", address, chain_id);

    let mut excessive_lamports = 0;
    if chain_id == DEFAULT_CHAIN_ID {
        // we don't have enough accounts to update non Neon chains
        excessive_lamports += crate::account::legacy::update_legacy_accounts(&accounts_db)?;
    };

    BalanceAccount::create(address, chain_id, &accounts_db, None)?;

    **accounts_db.operator().try_borrow_mut_lamports()? += excessive_lamports;

    Ok(())
}
