use std::convert::TryFrom;
use log::{debug, info};

use evm::{H160, U256, ExitReason,};

use evm_loader::{
    executor_state::{
        ExecutorState,
        ExecutorSubstate,
    },
    executor::Machine,
};

use crate::{
    account_storage::{
        EmulatorAccountStorage,
        AccountJSON,
        SolanaAccountJSON,
        TokenAccountJSON,
    },
    Config,
    NeonCliResult,
};

#[allow(clippy::too_many_lines)]
pub fn execute(
    config: &Config, 
    contract_id: Option<H160>, 
    caller_id: H160, 
    data: Option<Vec<u8>>,
    value: Option<U256>
) -> NeonCliResult {
    debug!("command_emulate(config={:?}, contract_id={:?}, caller_id={:?}, data={:?}, value={:?})",
        config,
        contract_id,
        caller_id,
        &hex::encode(data.clone().unwrap_or_default()),
        value);

    let storage = EmulatorAccountStorage::new(config);
    
    let program_id = if let Some(program_id) = contract_id {
        debug!("program_id to call: {}", program_id);
        program_id
    } else {
        let (solana_address, _nonce) = crate::make_solana_program_address(&caller_id, &config.evm_loader);
        let trx_count = crate::get_ether_account_nonce(config, &solana_address)?;
        let trx_count= trx_count.0;
        let program_id = crate::get_program_ether(&caller_id, trx_count);
        debug!("program_id to deploy: {}", program_id);
        program_id
    };

    let (exit_reason, result, applies_logs, used_gas, steps_executed) = {
        // u64::MAX is too large, remix gives this error:
        // Gas estimation errored with the following message (see below).
        // Number can only safely store up to 53 bits
        let gas_limit = 50_000_000;
        let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, &storage));
        let executor_state = ExecutorState::new(executor_substate, &storage);
        let mut executor = Machine::new(caller_id, executor_state);
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
                                    gas_limit)?;
                executor.execute()
            },
            None => {
                debug!("create_begin(caller_id={:?}, data={:?}, value={:?})",
                    caller_id,
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.create_begin(caller_id,
                                      data.unwrap_or_default(),
                                      value.unwrap_or_default(),
                                      gas_limit)?;
                executor.execute()
            }
        };
        debug!("Execute done, exit_reason={:?}, result={:?}", exit_reason, result);
        debug!("{} steps executed", executor.get_steps_executed());

        let steps_executed = executor.get_steps_executed();
        let executor_state = executor.into_state();
        let used_gas: u64 = executor_state.gasometer().used_gas() + 1; // "+ 1" because of https://github.com/neonlabsorg/neon-evm/issues/144
        let refunded_gas: i64 = executor_state.gasometer().refunded_gas();
        let needed_gas: u64 = used_gas + (if refunded_gas > 0 { u64::try_from(refunded_gas).unwrap_or(0) } else { 0 });
        debug!("used_gas={:?} refunded_gas={:?}", used_gas, refunded_gas);
        if exit_reason.is_succeed() {
            debug!("Succeed execution");
            let apply = executor_state.deconstruct();
            (exit_reason, result, Some(apply), needed_gas, steps_executed)
        } else {
            (exit_reason, result, None, needed_gas, steps_executed)
        }
    };

    debug!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies,
                _logs,
                transfers,
                spl_transfers,
                spl_approves,
                withdrawals,
                erc20_approves) = applies_logs.unwrap();

            storage.apply(applies)?;
            storage.apply_transfers(transfers);
            storage.apply_spl_approves(spl_approves);
            storage.apply_spl_transfers(spl_transfers);
            storage.apply_erc20_approves(erc20_approves);
            storage.apply_withdrawals(withdrawals);

            debug!("Applies done");
            "succeed".to_string()
        }
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
        ExitReason::StepLimitReached => unreachable!(),
    };

    info!("{}", &status);
    info!("{}", &hex::encode(&result));

    if !exit_reason.is_succeed() {
        debug!("Not succeed execution");
    }

    let accounts: Vec<AccountJSON> = storage.get_used_accounts();

    let solana_accounts: Vec<SolanaAccountJSON> = storage.solana_accounts
        .borrow()
        .values()
        .cloned()
        .map(SolanaAccountJSON::from)
        .collect();

    let token_accounts: Vec<TokenAccountJSON> = storage.token_accounts
        .borrow()
        .values()
        .cloned()
        .map(TokenAccountJSON::from)
        .collect();

    let js = serde_json::json!({
        "accounts": accounts,
        "solana_accounts": solana_accounts,
        "token_accounts": token_accounts,
        "result": &hex::encode(&result),
        "exit_status": status,
        "exit_reason": exit_reason,
        "used_gas": used_gas,
        "steps_executed": steps_executed,
    }).to_string();

    println!("{}", js);

    Ok(())
}

