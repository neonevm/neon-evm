use crate::account::{program, AccountsDB, BalanceAccount, Holder, Operator, Treasury};
use crate::error::Result;
use crate::gasometer::Gasometer;
use crate::types::Transaction;
use arrayref::array_ref;
use ethnum::U256;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

/// Execute Ethereum transaction in a single Solana transaction
pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Execute Transaction from Account");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);

    let holder = Holder::from_account(program_id, accounts[0].clone())?;

    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[1])? };
    let treasury = Treasury::from_account(program_id, treasury_index, &accounts[2])?;
    let operator_balance = BalanceAccount::from_account(program_id, accounts[3].clone())?;
    let system = program::System::from_account(&accounts[4])?;

    holder.validate_owner(&operator)?;
    let trx = Transaction::from_rlp(&holder.transaction())?;
    holder.validate_transaction(&trx)?;

    let origin = trx.recover_caller_address()?;

    solana_program::log::sol_log_data(&[b"HASH", &trx.hash()]);
    solana_program::log::sol_log_data(&[b"MINER", operator_balance.address().as_bytes()]);

    let accounts_db = AccountsDB::new(
        &accounts[5..],
        operator,
        Some(operator_balance),
        Some(system),
        Some(treasury),
    );

    let mut gasometer = Gasometer::new(U256::ZERO, accounts_db.operator())?;
    gasometer.record_solana_transaction_cost();
    gasometer.record_address_lookup_table(accounts);
    gasometer.record_write_to_holder(&trx);

    super::transaction_execute::validate(program_id, &accounts_db)?;
    super::transaction_execute::execute(accounts_db, gasometer, trx, origin)
}
