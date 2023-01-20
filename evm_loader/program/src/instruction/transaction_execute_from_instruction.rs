use crate::account::{Operator, program, EthereumAccount, Treasury};
use crate::account_storage::{AccountsReadiness, ProgramAccountStorage};
use crate::evm::Machine;
use crate::executor::ExecutorState;
use crate::gasometer::Gasometer;
use crate::instruction::transaction::log_return_value;
use crate::types::{Address, Transaction};
use crate::error::{Error, Result};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
};


struct Accounts<'a> {
    operator: Operator<'a>,
    treasury: Treasury<'a>,
    operator_ether_account: EthereumAccount<'a>,
    system_program: program::System<'a>,
    neon_program: program::Neon<'a>,
    remaining_accounts: &'a [AccountInfo<'a>],
    all_accounts: &'a [AccountInfo<'a>],
}

/// Execute Ethereum transaction in a single Solana transaction
pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> Result<()> {
    solana_program::msg!("Instruction: Execute Transaction from Instruction");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let messsage = &instruction[4..];

    let accounts = Accounts {
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0])? },
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[1])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[2])?,
        system_program: program::System::from_account(&accounts[3])?,
        neon_program: program::Neon::from_account(program_id, &accounts[4])?,
        remaining_accounts: &accounts[5..],
        all_accounts: accounts,
    };

    let trx = Transaction::from_rlp(messsage)?;
    let caller_address = trx.recover_caller_address()?;

    solana_program::log::sol_log_data(&[b"HASH", &trx.hash]);

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        &accounts.operator,
        Some(&accounts.system_program),
        accounts.remaining_accounts,
    )?;


    validate(&accounts, &account_storage, &trx, &caller_address)?;
    execute(accounts, &mut account_storage, trx, caller_address)
}

fn validate(
    _accounts: &Accounts,
    account_storage: &ProgramAccountStorage,
    _trx: &Transaction,
    _caller_address: &Address,
) -> Result<()> {
    account_storage.check_for_blocked_accounts()?;

    Ok(())
}

fn execute<'a>(
    accounts: Accounts<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    trx: Transaction,
    caller_address: Address,
) -> Result<()> {
    let mut gasometer = Gasometer::new(None, &accounts.operator)?;
    gasometer.record_solana_transaction_cost();
    gasometer.record_address_lookup_table(accounts.all_accounts);

    accounts.system_program.transfer(&accounts.operator, &accounts.treasury, crate::config::PAYMENT_TO_TREASURE)?;

    let gas_limit = trx.gas_limit;
    let gas_price = trx.gas_price;

    let (exit_reason, apply_state) = {
        let mut backend = ExecutorState::new(account_storage);

        let mut evm = Machine::new(trx, caller_address, &mut backend)?;
        let (result, _) = evm.execute(u64::MAX, &mut backend)?;

        let actions = backend.into_actions();

        (result, actions)
    };

    let accounts_readiness = account_storage.apply_state_change(
        &accounts.neon_program,
        &accounts.system_program,
        &accounts.operator,
        apply_state,
    )?;

    assert_eq!(
        accounts_readiness,
        AccountsReadiness::Ready,
        "Deployment of contract which needs more than 10kb of account space needs several \
            transactions for reallocation and cannot be performed in a single instruction. \
            That's why you have to use iterative transaction for the deployment.",
    );

    gasometer.record_operator_expenses(&accounts.operator);
    let used_gas = gasometer.used_gas();
    if used_gas > gas_limit {
        return Err(Error::OutOfGas(gas_limit, used_gas));
    }

    solana_program::log::sol_log_data(&[b"IX_GAS", &used_gas.as_u64().to_le_bytes()]);

    let gas_cost = used_gas.saturating_mul(gas_price);
    account_storage.transfer_gas_payment(caller_address, accounts.operator_ether_account, gas_cost)?;

    log_return_value(&exit_reason, used_gas);
    
    Ok(())
}
