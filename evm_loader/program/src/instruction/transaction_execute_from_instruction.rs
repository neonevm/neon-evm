use crate::account::{program, AccountsDB, BalanceAccount, Operator, Treasury};
use crate::debug::log_data;
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
    log_msg!("Instruction: Execute Transaction from Instruction");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let messsage = &instruction[4..];

    let operator = unsafe { Operator::from_account_not_whitelisted(&accounts[0])? };
    let treasury = Treasury::from_account(program_id, treasury_index, &accounts[1])?;
    let operator_balance = BalanceAccount::from_account(program_id, accounts[2].clone())?;
    let system = program::System::from_account(&accounts[3])?;

    let trx = Transaction::from_rlp(messsage)?;
    let origin = trx.recover_caller_address()?;

    log_data(&[b"HASH", &trx.hash()]);
    log_data(&[b"MINER", operator_balance.address().as_bytes()]);

    let accounts_db = AccountsDB::new(
        &accounts[4..],
        operator,
        Some(operator_balance),
        Some(system),
        Some(treasury),
    );

    let mut gasometer = Gasometer::new(U256::ZERO, accounts_db.operator())?;
    gasometer.record_solana_transaction_cost();
    gasometer.record_address_lookup_table(accounts);

    super::transaction_execute::execute(accounts_db, gasometer, trx, origin)
}
