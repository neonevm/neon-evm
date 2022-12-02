use log::{debug, info};

use evm_loader::{H160, U256, ExitReason};
use evm_loader::{executor::{Machine, LAMPORTS_PER_SIGNATURE}, config::{EVM_STEPS_MIN, PAYMENT_TO_TREASURE}};

use crate::{
    account_storage::{
        EmulatorAccountStorage, NeonAccount, SolanaAccount,
    },
    Config,
    NeonCliResult,
    syscall_stubs::Stubs,
    account_storage::make_solana_program_address,
};

use solana_sdk::pubkey::Pubkey;
use evm_loader::account_storage::AccountStorage;
use crate::errors;
use super::{get_program_ether, get_ether_account_nonce};

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn execute(
    config: &Config, 
    contract_id: Option<H160>, 
    caller_id: H160, 
    data: Option<Vec<u8>>,
    value: Option<U256>,
    token_mint: &Pubkey,
    chain_id: u64,
    max_steps_to_execute: u64,
) -> NeonCliResult {
    debug!("command_emulate(config={:?}, contract_id={:?}, caller_id={:?}, data={:?}, value={:?})",
        config,
        contract_id,
        caller_id,
        &hex::encode(data.clone().unwrap_or_default()),
        value);

    let syscall_stubs = Stubs::new(config)?;
    solana_sdk::program_stubs::set_syscall_stubs(syscall_stubs);

    let storage = EmulatorAccountStorage::new(config, *token_mint, chain_id);

    let program_id = if let Some(program_id) = contract_id {
        debug!("program_id to call: {}", program_id);
        program_id
    } else {
        let (solana_address, _nonce) = make_solana_program_address(&caller_id, &config.evm_loader);
        let trx_count = get_ether_account_nonce(config, &solana_address)?;
        let trx_count= trx_count.0;
        let program_id = get_program_ether(&caller_id, trx_count);
        debug!("program_id to deploy: {}", program_id);
        program_id
    };

    let (exit_reason, result, actions, steps_executed) = {
        let gas_limit = U256::from(999_999_999_999_u64);
        let mut executor = Machine::new(caller_id, &storage)?;
        debug!("Executor initialized");

        let (result, exit_reason) = match &contract_id {
            Some(_) =>  {
                debug!("call_begin(caller_id={:?}, program_id={:?}, data={:?}, value={:?})",
                    caller_id,
                    program_id,
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);

                executor.call_begin(caller_id,
                    program_id,
                    data.unwrap_or_default(),
                    value.unwrap_or_default(),
                    gas_limit, U256::zero())?;
                match executor.execute_n_steps(max_steps_to_execute){
                    Ok(()) => {
                        info!("too many steps");
                        return Err(errors::NeonCliError::TooManySteps)
                    },
                    Err(result) => result
                }
            },
            None => {
                debug!("create_begin(caller_id={:?}, data={:?}, value={:?})",
                    caller_id,
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.create_begin(
                    caller_id,
                    data.unwrap_or_default(),
                    value.unwrap_or_default(),
                    gas_limit,
                    U256::zero(),
                )?;
                match executor.execute_n_steps(max_steps_to_execute){
                    Ok(()) => {
                        info!("too many steps");
                        return Err(errors::NeonCliError::TooManySteps)
                    },
                    Err(result) => result
                }
            }
        };
        let steps_executed = executor.get_steps_executed();
        debug!("Execute done, exit_reason={:?}, result={:?}", exit_reason, result);
        debug!("{} steps executed", steps_executed);

        let actions = executor.into_state_actions();
        (exit_reason, result, actions, steps_executed)
    };

    let accounts_operations = storage.calc_accounts_operations(&actions);

    let max_iterations = (steps_executed + (EVM_STEPS_MIN - 1)) / EVM_STEPS_MIN;
    let steps_gas = max_iterations * (LAMPORTS_PER_SIGNATURE + PAYMENT_TO_TREASURE);
    let begin_end_gas = 2 * LAMPORTS_PER_SIGNATURE;
    let actions_gas = storage.apply_actions(actions);
    let accounts_gas = storage.apply_accounts_operations(accounts_operations);
    debug!("Gas - steps: {steps_gas}, actions: {actions_gas}, accounts: {accounts_gas}");

    debug!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => "succeed".to_string(),
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
        ExitReason::StepLimitReached => unreachable!(),
    };

    info!("{}", status);
    info!("{}", hex::encode(&result));

    let accounts: Vec<NeonAccount> = storage.accounts
        .borrow()
        .values()
        .cloned()
        .collect();

    let solana_accounts: Vec<SolanaAccount> = storage.solana_accounts
        .borrow()
        .values()
        .cloned()
        .collect();

    let js = serde_json::json!({
        "accounts": accounts,
        "solana_accounts": solana_accounts,
        "token_accounts": [],
        "result": hex::encode(result),
        "exit_status": status,
        "exit_reason": exit_reason,
        "steps_executed": steps_executed,
        "used_gas": steps_gas + begin_end_gas + actions_gas + accounts_gas
    });

    println!("{}", js);

    Ok(())
}
