use crate::account::{program, EthereumAccount, Operator, Treasury};
use crate::account_storage::{AccountsReadiness, ProgramAccountStorage};
use crate::config::CHAIN_ID;
use crate::error::{Error, Result};
use crate::evm::Machine;
use crate::executor::ExecutorState;
use crate::gasometer::Gasometer;
use crate::instruction::transaction_step::log_return_value;
use crate::types::{Address, Transaction};
use ethnum::U256;
use solana_program::account_info::AccountInfo;

pub struct Accounts<'a> {
    pub operator: Operator<'a>,
    pub treasury: Treasury<'a>,
    pub operator_ether_account: EthereumAccount<'a>,
    pub system_program: program::System<'a>,
    pub neon_program: program::Neon<'a>,
    pub remaining_accounts: &'a [AccountInfo<'a>],
    pub all_accounts: &'a [AccountInfo<'a>],
}

pub fn validate(
    _accounts: &Accounts,
    account_storage: &ProgramAccountStorage,
    trx: &Transaction,
    _caller_address: &Address,
) -> Result<()> {
    if trx.chain_id != Some(CHAIN_ID.into()) {
        return Err(Error::InvalidChainId(trx.chain_id.unwrap_or(U256::ZERO)));
    }

    account_storage.check_for_blocked_accounts()?;

    Ok(())
}

pub fn execute<'a>(
    accounts: Accounts<'a>,
    account_storage: &mut ProgramAccountStorage<'a>,
    mut gasometer: Gasometer,
    trx: Transaction,
    caller_address: Address,
) -> Result<()> {
    accounts.system_program.transfer(
        &accounts.operator,
        &accounts.treasury,
        crate::config::PAYMENT_TO_TREASURE,
    )?;

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

    solana_program::log::sol_log_data(&[b"GAS", &used_gas.to_le_bytes(), &used_gas.to_le_bytes()]);

    let gas_cost = used_gas.saturating_mul(gas_price);
    account_storage.transfer_gas_payment(
        caller_address,
        accounts.operator_ether_account,
        gas_cost,
    )?;

    log_return_value(&exit_reason);

    Ok(())
}
