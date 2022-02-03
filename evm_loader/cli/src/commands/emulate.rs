use std::convert::TryFrom;
use log::{debug, info};

use solana_sdk::{
    pubkey::Pubkey,
};

use evm::{H160, U256, ExitReason,};

use evm_loader::{
    executor_state::{
        ExecutorState,
        ExecutorSubstate,
    },
    executor::Machine,
    solana_backend::AccountStorage,
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
pub fn execute(config: &Config, contract_id: Option<H160>, caller_id: H160, data: Option<Vec<u8>>,
                   value: Option<U256>, token_mint: &Pubkey) -> NeonCliResult {
    debug!("command_emulate(config={:?}, contract_id={:?}, caller_id={:?}, data={:?}, value={:?})",
        config,
        contract_id,
        caller_id,
        &hex::encode(data.clone().unwrap_or_default()),
        value);

    let storage = match &contract_id {
        Some(program_id) =>  {
            debug!("program_id to call: {:?}", *program_id);
            EmulatorAccountStorage::new(config, *program_id, caller_id, *token_mint)
        },
        None => {
            let (solana_address, _nonce) = crate::make_solana_program_address(&caller_id, &config.evm_loader);
            let trx_count = crate::get_ether_account_nonce(config, &solana_address, token_mint)?;
            let trx_count= trx_count.0;
            let program_id = crate::get_program_ether(&caller_id, trx_count);
            debug!("program_id to deploy: {:?}", program_id);
            EmulatorAccountStorage::new(config, program_id, caller_id, *token_mint)
        }
    };

    let (exit_reason, result, applies_logs,  steps_executed) = {
        let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, &storage));
        let executor_state = ExecutorState::new(executor_substate, &storage);
        let mut executor = Machine::new(executor_state);
        debug!("Executor initialized");

        let (result, exit_reason) = match &contract_id {
            Some(_) =>  {
                debug!("call_begin(storage.origin()={:?}, storage.contract()={:?}, data={:?}, value={:?})",
                    storage.origin(),
                    storage.contract(),
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.call_begin(storage.origin(),
                                    storage.contract(),
                                    data.unwrap_or_default(),
                                    value.unwrap_or_default())?;
                executor.execute()
            },
            None => {
                debug!("create_begin(storage.origin()={:?}, data={:?}, value={:?})",
                    storage.origin(),
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.create_begin(storage.origin(),
                                      data.unwrap_or_default(),
                                      value.unwrap_or_default())?;
                executor.execute()
            }
        };
        debug!("Execute done, exit_reason={:?}, result={:?}", exit_reason, result);
        debug!("{} steps executed", executor.get_steps_executed());

        let steps_executed = executor.get_steps_executed();
        let executor_state = executor.into_state();
        if exit_reason.is_succeed() {
            debug!("Succeed execution");
            let apply = executor_state.deconstruct();
            (exit_reason, result, Some(apply), steps_executed)
        } else {
            (exit_reason, result, None, steps_executed)
        }
    };

    debug!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, _logs, transfers, spl_transfers, spl_approves, erc20_approves) = applies_logs.unwrap();

            storage.apply(applies)?;
            storage.apply_transfers(transfers, token_mint);
            storage.apply_spl_approves(spl_approves);
            storage.apply_spl_transfers(spl_transfers);
            storage.apply_erc20_approves(erc20_approves);

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
        "steps_executed": steps_executed,
    }).to_string();

    println!("{}", js);

    Ok(())
}

