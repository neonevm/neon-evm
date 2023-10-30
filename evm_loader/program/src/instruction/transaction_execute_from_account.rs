use crate::account::{program, EthereumAccount, Holder, Operator, Treasury};
use crate::account_storage::ProgramAccountStorage;
use crate::error::Result;
use crate::gasometer::Gasometer;
use crate::instruction::transaction_execute::Accounts;
use crate::types::Transaction;
use arrayref::array_ref;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

/// Execute Ethereum transaction in a single Solana transaction
pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Execute Transaction from Account");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);

    let holder = Holder::from_account(program_id, &accounts[0])?;

    let accounts = Accounts {
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[1])? },
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[2])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[3])?,
        system_program: program::System::from_account(&accounts[4])?,
        neon_program: program::Neon::from_account(program_id, &accounts[5])?,
        remaining_accounts: &accounts[6..],
        all_accounts: accounts,
    };

    holder.validate_owner(&accounts.operator)?;
    let mut trx = Transaction::from_rlp(&holder.transaction())?;
    holder.validate_transaction(&trx)?;

    let caller_address = trx.recover_caller_address()?;

    solana_program::log::sol_log_data(&[b"HASH", &trx.hash()]);

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        &accounts.operator,
        Some(&accounts.system_program),
        accounts.remaining_accounts,
    )?;

    let mut gasometer = Gasometer::new(None, &accounts.operator)?;
    gasometer.record_solana_transaction_cost();
    gasometer.record_address_lookup_table(accounts.all_accounts);
    gasometer.record_write_to_holder(&trx);

    super::transaction_execute::validate(&accounts, &account_storage, &trx, &caller_address)?;
    super::transaction_execute::execute(
        accounts,
        &mut account_storage,
        gasometer,
        &mut trx,
        caller_address,
    )
}
